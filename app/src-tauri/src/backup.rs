//! Backup and restore: the whole library in one file the user keeps wherever
//! they like (an external drive, or a OneDrive / Google Drive / Dropbox folder
//! that syncs it). Holds games, Hearth's own playtime, hidden games, statuses,
//! collections, artwork choices, game folders and appearance settings; never
//! sign-ins, which stay in Windows Credential Manager.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hub_core::{Db, LaunchOption};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, folders, lock, now, sync_all, AppState, CmdResult};

/// Bumped if the backup's own layout ever changes (the library inside it is
/// versioned and upgraded by its migrations).
const FORMAT: &str = "1";

#[tauri::command]
pub fn backup_export(app: AppHandle, path: String, prefs: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let folders = serde_json::to_string(&folders::current(&app)).map_err(err)?;
    let created = now().to_string();
    lock(&state.db)
        .backup_to(
            Path::new(&path),
            &[
                ("format", FORMAT),
                ("created_at", &created),
                ("app_version", env!("CARGO_PKG_VERSION")),
                ("game_folders", &folders),
                ("prefs", &prefs),
            ],
        )
        .map_err(err)?;
    crate::tracking::log(&app, &format!("backup: saved to {path}"));
    Ok(())
}

#[derive(Serialize)]
pub struct Restored {
    /// Appearance settings from the backup, for the window to apply.
    prefs: Option<String>,
    /// When the backup was made (unix seconds).
    created_at: Option<i64>,
    /// Ways to play the backup holds, set aside until the user confirms them.
    ways: Vec<WayToConfirm>,
    /// Game folders the backup adds, set aside until the user confirms them.
    folders: Vec<String>,
}

/// Copies of the library kept from before a restore; older ones are removed.
const KEEP_BEFORE_RESTORE: usize = 3;

const LIBRARY_FILES: [&str; 3] = ["library.db", "library.db-wal", "library.db-shm"];

/// Moves the library set aside during a restore (`library.db.old`) back in
/// place. Also run at startup: Hearth closed mid-restore with no library
/// file gets its own back instead of starting empty. False if there was
/// nothing to put back.
pub fn put_back_aside(dir: &std::path::Path) -> bool {
    if dir.join("library.db").exists() || !dir.join("library.db.old").exists() {
        return false;
    }
    LIBRARY_FILES.iter().fold(true, |ok, name| {
        let old = dir.join(format!("{name}.old"));
        ok && (!old.exists() || std::fs::rename(&old, dir.join(name)).is_ok())
    })
}

#[tauri::command]
pub fn backup_restore(app: AppHandle, path: String) -> CmdResult<Restored> {
    let source = PathBuf::from(&path);
    let meta = Db::read_backup(&source).map_err(err)?;
    let dir = app.path().app_data_dir().map_err(err)?;
    let live = dir.join("library.db");
    let state = app.state::<AppState>();

    // Copied beside the library first, so a failed copy leaves it untouched,
    // and before the copy of what's here now: restoring that copy can't
    // overwrite itself.
    let incoming = dir.join("library.db.incoming");
    std::fs::copy(&source, &incoming).map_err(err)?;
    // What's here now is kept, in case the backup turns out to be the wrong
    // one. Each under its own name, so restoring one never replaces another.
    let safety = dir.join(format!("before-restore {}.hearthbackup", crate::autobackup::stamp()));

    // A game being timed is saved to the library being replaced first: its
    // id may belong to another game, or none, in the restored one.
    crate::tracking::around_restore(&app, || -> CmdResult<()> {
        let mut db = lock(&state.db);
        // After the tracker has saved to it, so the copy has that time too.
        if let Err(e) = db.backup_to(&safety, &[("format", FORMAT), ("created_at", &now().to_string())]) {
            let _ = std::fs::remove_file(&incoming);
            return Err(err(e));
        }
        // Close the library file before replacing it. It's moved aside, not
        // deleted, until the restored one opens: any failure puts it back.
        *db = Db::open_in_memory().map_err(err)?;
        for name in LIBRARY_FILES {
            let _ = std::fs::remove_file(dir.join(format!("{name}.old")));
            let _ = std::fs::rename(dir.join(name), dir.join(format!("{name}.old")));
        }
        let opened = std::fs::rename(&incoming, &live).map_err(err).and_then(|_| Db::open(&live).map_err(err));
        match opened.and_then(|restored| {
            let ways = restored.quarantine_restored().map_err(err)?;
            Ok((restored, ways))
        }) {
            Ok((restored, ways)) => {
                let _ = restored.forget_backup_meta();
                *db = restored;
                *lock(&app.state::<PendingWays>().ways) = ways;
                for name in LIBRARY_FILES {
                    let _ = std::fs::remove_file(dir.join(format!("{name}.old")));
                }
                Ok(())
            }
            Err(e) => {
                // Put the library back as it was: the files moved aside, or
                // failing that the copy made above.
                let _ = std::fs::remove_file(&incoming);
                for name in LIBRARY_FILES {
                    let _ = std::fs::remove_file(dir.join(name));
                }
                if !put_back_aside(&dir) {
                    std::fs::copy(&safety, &live).map_err(err)?;
                }
                *db = Db::open(&live).map_err(err)?;
                let _ = db.forget_backup_meta();
                Err(format!("Couldn't restore that backup ({e}). Your library is unchanged."))
            }
        }
    })?;
    prune_before_restore(&dir);

    // Game folders are scanned for programs to time and play: like ways to
    // play, the backup's are added only once the user confirms them.
    let have = folders::current(&app);
    let new_folders: Vec<String> = meta
        .get("game_folders")
        .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|f| !have.iter().any(|h| h.eq_ignore_ascii_case(f)))
        .collect();
    *lock(&app.state::<PendingWays>().folders) = new_folders.clone();
    crate::tracking::log(&app, &format!("backup: restored {path}"));
    let _ = app.emit("library-changed", ());
    sync_all(app.clone());
    let ways = lock(&app.state::<PendingWays>().ways)
        .iter()
        .map(|w| WayToConfirm { name: w.name.clone(), path: w.path.clone(), args: w.args.clone() })
        .collect();
    Ok(Restored {
        prefs: meta.get("prefs").cloned(),
        created_at: meta.get("created_at").and_then(|t| t.parse().ok()),
        ways,
        folders: new_folders,
    })
}

/// Keeps the newest few copies made before a restore (names sort by date).
fn prune_before_restore(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut copies: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("before-restore ") && n.ends_with(".hearthbackup")))
        .collect();
    copies.sort();
    let extra = copies.len().saturating_sub(KEEP_BEFORE_RESTORE);
    for old in copies.into_iter().take(extra) {
        let _ = std::fs::remove_file(old);
    }
}

/// Ways to play and game folders from a restored backup, held back until
/// the user confirms them: each can run a program, and the backup file may
/// not be theirs. Lost (so dropped) if Hearth closes first.
#[derive(Default)]
pub struct PendingWays {
    ways: Mutex<Vec<LaunchOption>>,
    folders: Mutex<Vec<String>>,
}

#[derive(Serialize)]
pub struct WayToConfirm {
    name: String,
    path: String,
    args: String,
}

/// The user's answer about a restored backup's ways to play and, when
/// given, its game folders.
#[tauri::command]
pub fn restore_ways(app: AppHandle, keep: bool, folders: Option<bool>) -> CmdResult<()> {
    let pending = app.state::<PendingWays>();
    let ways = std::mem::take(&mut *lock(&pending.ways));
    if keep && !ways.is_empty() {
        lock(&app.state::<AppState>().db).put_back_launch_options(&ways).map_err(err)?;
        let _ = app.emit("library-changed", ());
    }
    crate::tracking::log(&app, &format!("backup: {} {} restored ways to play", if keep { "kept" } else { "dropped" }, ways.len()));
    if let Some(add) = folders {
        let extra = std::mem::take(&mut *lock(&pending.folders));
        if add && !extra.is_empty() {
            let mut list = folders::current(&app);
            list.extend(extra.iter().cloned());
            folders::replace(&app, list)?;
        }
        crate::tracking::log(&app, &format!("backup: {} {} restored game folders", if add { "added" } else { "left out" }, extra.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_left_aside_by_an_interrupted_restore_comes_back() {
        let dir = std::env::temp_dir().join(format!("hearth-aside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("library.db.old"), "mine").unwrap();
        std::fs::write(dir.join("library.db-wal.old"), "wal").unwrap();
        assert!(put_back_aside(&dir));
        assert_eq!(std::fs::read_to_string(dir.join("library.db")).unwrap(), "mine");
        assert!(dir.join("library.db-wal").exists() && !dir.join("library.db.old").exists());
        // A library in place is never replaced.
        std::fs::write(dir.join("library.db.old"), "older").unwrap();
        assert!(!put_back_aside(&dir));
        assert_eq!(std::fs::read_to_string(dir.join("library.db")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
