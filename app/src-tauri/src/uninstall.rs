//! Uninstalling a game, always through whoever installed it, so the store
//! doesn't go on thinking the game is there (and offer to "repair" it).
//! Steam and GOG start their own uninstall; Epic and the Xbox app can't be
//! asked to, so their place to do it is opened with directions; a folder game
//! is moved to the Recycle Bin. Whichever way, Hearth rescans once the install
//! folder is gone.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hub_core::StoreId;
use hub_launch::LaunchAction;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, AppState, CmdResult};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Uninstall {
    /// The store's own uninstall started (it asks to confirm).
    Started { by: String },
    /// That app was opened; the uninstall is finished there.
    Elsewhere { app: String, steps: String },
    /// The folder is in the Recycle Bin.
    Recycled,
}

/// Where a game would be uninstalled from, for the confirmation before it.
#[derive(Serialize)]
pub struct UninstallPlan {
    /// "steam", "gog", "epic", "xbox", "manual".
    pub store: String,
    pub path: String,
}

fn installed_entry(app: &AppHandle, id: i64, version: Option<i64>) -> CmdResult<(hub_core::Game, hub_core::StoreEntry)> {
    let state = app.state::<AppState>();
    let game = lock(&state.db)
        .game(id)
        .map_err(err)?
        .ok_or("That game is no longer in your library")?;
    if let Some(drive) = &game.offline_drive {
        return Err(format!("{} is on drive {drive}, which isn't connected. Connect it to uninstall.", game.title));
    }
    let entry = game
        .entries
        .iter()
        .find(|e| e.installed && !e.offline && e.install_dir.is_some() && version.is_none_or(|v| v == e.id))
        .cloned()
        .ok_or_else(|| format!("{} isn't installed", game.title))?;
    Ok((game, entry))
}

#[tauri::command]
pub fn uninstall_plan(app: AppHandle, id: i64, entry: Option<i64>) -> CmdResult<UninstallPlan> {
    let (_, e) = installed_entry(&app, id, entry)?;
    Ok(UninstallPlan { store: e.store.as_str().into(), path: e.install_dir.unwrap_or_default() })
}

#[tauri::command]
pub fn uninstall_game(app: AppHandle, id: i64, entry: Option<i64>) -> CmdResult<Uninstall> {
    let (game, entry) = installed_entry(&app, id, entry)?;
    let dir = PathBuf::from(entry.install_dir.clone().unwrap_or_default());
    let provider = || {
        app.state::<AppState>()
            .providers
            .iter()
            .find(|p| p.id() == entry.store)
            .and_then(|p| p.uninstall(&entry.store_game_id))
    };
    let outcome = match entry.store {
        StoreId::Manual => {
            check_recyclable(&app, &dir)?;
            recycle(&dir)?;
            Uninstall::Recycled
        }
        StoreId::Steam | StoreId::Gog => match provider() {
            Some(action) => {
                hub_launch::run(&action).map_err(err)?;
                Uninstall::Started { by: if entry.store == StoreId::Steam { "Steam" } else { "GOG" }.into() }
            }
            // A GOG game without its uninstaller on disk: Galaxy can still remove it.
            None => {
                let uri = format!("goggalaxy://openGameView/{}", entry.store_game_id);
                hub_launch::run(&LaunchAction::Uri { uri }).map_err(err)?;
                Uninstall::Elsewhere {
                    app: "GOG Galaxy".into(),
                    steps: format!("On {}'s page, open the settings menu, then Manage installation › Uninstall.", game.title),
                }
            }
        },
        StoreId::Epic => {
            hub_launch::run(&LaunchAction::Uri { uri: "com.epicgames.launcher://store".into() }).map_err(err)?;
            Uninstall::Elsewhere {
                app: "Epic Games Launcher".into(),
                steps: format!("In Library, click ⋯ on {}, then Uninstall.", game.title),
            }
        }
        StoreId::Xbox => {
            // Windows' Installed apps removes Store and Game Pass games cleanly.
            hub_launch::run(&LaunchAction::Uri { uri: "ms-settings:appsfeatures".into() }).map_err(err)?;
            Uninstall::Elsewhere {
                app: "Installed apps".into(),
                steps: format!("Search for {}, click ⋯ beside it, then Uninstall.", game.title),
            }
        }
        StoreId::Ea | StoreId::Ubisoft => return Err(format!("Uninstall {} from its own app for now", game.title)),
    };
    crate::tracking::log(&app, &format!("uninstall: {} via {}", game.title, entry.store.as_str()));
    if matches!(outcome, Uninstall::Recycled) {
        crate::sync_all(app.clone());
    } else {
        rescan_when_gone(app, dir);
    }
    Ok(outcome)
}

/// Waits (up to half an hour) for the store to remove the folder, then
/// rescans so the game shows as not installed and the space as free.
fn rescan_when_gone(app: AppHandle, dir: PathBuf) {
    std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(30 * 60) {
            std::thread::sleep(Duration::from_secs(3));
            if !dir.exists() {
                crate::sync_all(app.clone());
                let _ = app.emit("library-changed", ());
                return;
            }
        }
    });
}

/// A folder game's folder, never a drive or one of the watched game folders
/// themselves (the folder holding every game), nor a folder holding one, or
/// holding another store's game (a launcher's folder found as a game).
fn check_recyclable(app: &AppHandle, dir: &Path) -> CmdResult<()> {
    let watched = crate::folders::current(app);
    let others = lock(&app.state::<AppState>().db).store_install_dirs(StoreId::Manual).map_err(err)?;
    if !single_game(dir, watched.iter().chain(&others)) {
        return Err(format!("Hearth won't remove {}: it isn't a single game's folder", dir.display()));
    }
    if !dir.is_dir() {
        return Err(format!("{} isn't there any more", dir.display()));
    }
    Ok(())
}

/// `dir` isn't a drive and none of the folders to `keep` is it or inside it.
fn single_game<'a>(dir: &Path, mut keep: impl Iterator<Item = &'a String>) -> bool {
    let is_root = dir.parent().is_none() || dir.components().count() <= 2;
    !is_root && !keep.any(|k| inside(dir, Path::new(k)))
}

/// Whether `path` is `dir` or lies inside it, ignoring case as Windows does.
fn inside(dir: &Path, path: &Path) -> bool {
    let name = |c: std::path::Component| c.as_os_str().to_string_lossy().to_lowercase();
    let mut rest = path.components();
    dir.components().all(|d| rest.next().is_some_and(|c| name(c) == name(d)))
}

#[cfg(windows)]
fn recycle(dir: &Path) -> CmdResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_WANTNUKEWARNING, FO_DELETE, SHFILEOPSTRUCTW,
    };
    // A list of paths, each NUL-terminated, ending in an extra NUL.
    let from: Vec<u16> = dir.as_os_str().encode_wide().chain([0, 0]).collect();
    let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
    op.wFunc = FO_DELETE;
    op.pFrom = from.as_ptr();
    // To the Recycle Bin. Hearth already asked; Windows still warns if the
    // folder is too big for the bin and would be deleted for good.
    op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING) as u16;
    // SAFETY: `op` is fully initialised and `from` outlives the call.
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 {
        return Err(format!("Windows couldn't move the folder to the Recycle Bin (code {code})"));
    }
    if op.fAnyOperationsAborted != 0 {
        return Err("Cancelled; nothing was removed".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn recycle(_dir: &Path) -> CmdResult<()> {
    Err("Only on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_single_games_folder_is_recycled() {
        let keep = [r"D:\Games".to_string(), r"F:\Media\Games".to_string(), r"D:\Launchers\Galaxy\Games\Skyfall".to_string(), r"E:\Steam\steamapps\common\Driftwood".to_string()];
        let ok = |d: &str| single_game(Path::new(d), keep.iter());
        assert!(ok(r"D:\Games\Lantern Keep"));
        assert!(ok(r"D:\Games\Warhammer 40,000"));
        // A sibling whose name starts the same isn't inside.
        assert!(ok(r"D:\Launchers\Galaxy\Games\Skyfall 2"));
        // Drives, a watched folder, and folders holding another store's game.
        assert!(!ok(r"D:\"));
        assert!(!ok(""));
        assert!(!ok(r"d:\games\"));
        assert!(!ok(r"D:\Launchers\Galaxy"));
        assert!(!ok(r"d:\launchers\galaxy\games\skyfall"));
        assert!(!ok(r"E:\Steam"));
        // A folder holding a watched folder.
        assert!(!ok(r"F:\Media"));
    }
}
