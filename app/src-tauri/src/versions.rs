//! Versions of one game: GTA V and GTA V Enhanced, a game and its remaster,
//! shown as one library entry. Combining is always the user's choice (Hearth
//! only suggests it) and can be undone.

use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, AppState, CmdResult};

fn changed(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
}

/// Makes `other` a version of `keep`.
#[tauri::command]
pub fn combine_games(app: AppHandle, keep: i64, other: i64) -> CmdResult<()> {
    let state = app.state::<AppState>();
    lock(&state.db).combine_games(keep, other).map_err(err)?;
    crate::tracking::log(&app, &format!("versions: combined game {other} into {keep}"));
    changed(&app);
    Ok(())
}

/// Takes one version (and the same version on other stores) back out into a
/// game of its own. Returns the new game's id.
#[tauri::command]
pub fn separate_version(app: AppHandle, game_id: i64, entry_id: i64) -> CmdResult<i64> {
    let state = app.state::<AppState>();
    let id = lock(&state.db).separate_version(game_id, entry_id).map_err(err)?;
    crate::tracking::log(&app, &format!("versions: separated entry {entry_id} of game {game_id}"));
    changed(&app);
    // The separated game starts without art; fetch it now rather than at
    // the next sync, so it doesn't sit there blank.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::refresh_art(&handle).await;
        let _ = handle.emit("library-changed", ());
    });
    Ok(id)
}
