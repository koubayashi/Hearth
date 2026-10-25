//! Backup and restore: the whole library in one file the user keeps wherever
//! they like (an external drive, or a OneDrive / Google Drive / Dropbox folder
//! that syncs it). Holds games, Hearth's own playtime, hidden games, statuses,
//! collections, artwork choices, game folders and appearance settings; never
//! sign-ins, which stay in Windows Credential Manager.

use std::path::{Path, PathBuf};

use hub_core::Db;
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
}

#[tauri::command]
pub fn backup_restore(app: AppHandle, path: String) -> CmdResult<Restored> {
    let source = PathBuf::from(&path);
    let meta = Db::read_backup(&source).map_err(err)?;
    let dir = app.path().app_data_dir().map_err(err)?;
    let live = dir.join("library.db");
    let state = app.state::<AppState>();

    // What's here now is kept, in case the backup turns out to be the wrong one.
    let safety = dir.join("before-restore.hearthbackup");
    lock(&state.db).backup_to(&safety, &[("format", FORMAT), ("created_at", &now().to_string())]).map_err(err)?;
    // Copied beside the library first, so a failed copy leaves it untouched.
    let incoming = dir.join("library.db.incoming");
    std::fs::copy(&source, &incoming).map_err(err)?;

    {
        let mut db = lock(&state.db);
        // Close the library file before replacing it.
        *db = Db::open_in_memory().map_err(err)?;
        for name in ["library.db", "library.db-wal", "library.db-shm"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
        let opened = std::fs::rename(&incoming, &live).map_err(err).and_then(|_| Db::open(&live).map_err(err));
        match opened {
            Ok(restored) => {
                let _ = restored.forget_backup_meta();
                *db = restored;
            }
            Err(e) => {
                // Put the library back as it was.
                std::fs::copy(&safety, &live).map_err(err)?;
                *db = Db::open(&live).map_err(err)?;
                let _ = db.forget_backup_meta();
                return Err(format!("Couldn't restore that backup ({e}). Your library is unchanged."));
            }
        }
    }

    if let Some(list) = meta.get("game_folders").and_then(|j| serde_json::from_str::<Vec<String>>(j).ok()) {
        folders::replace(&app, list)?;
    }
    crate::tracking::log(&app, &format!("backup: restored {path}"));
    let _ = app.emit("library-changed", ());
    sync_all(app.clone());
    Ok(Restored {
        prefs: meta.get("prefs").cloned(),
        created_at: meta.get("created_at").and_then(|t| t.parse().ok()),
    })
}
