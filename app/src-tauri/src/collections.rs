//! Statuses (playing, backlog, completed, abandoned) and the user's own
//! collections.

use hub_core::{Collection, GameStatus};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{err, lock, AppState, CmdResult};

fn changed(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
}

#[tauri::command]
pub fn set_status(app: AppHandle, id: i64, status: Option<GameStatus>) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).set_status(id, status).map_err(err)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn list_collections(state: State<AppState>) -> CmdResult<Vec<Collection>> {
    lock(&state.db).collections().map_err(err)
}

#[tauri::command]
pub fn create_collection(app: AppHandle, name: String) -> CmdResult<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the collection a name".into());
    }
    let id = lock(&app.state::<AppState>().db).create_collection(name).map_err(err)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn rename_collection(app: AppHandle, id: i64, name: String) -> CmdResult<()> {
    if name.trim().is_empty() {
        return Err("Give the collection a name".into());
    }
    lock(&app.state::<AppState>().db).rename_collection(id, &name).map_err(err)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_collection(app: AppHandle, id: i64) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).delete_collection(id).map_err(err)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_in_collection(app: AppHandle, collection: i64, game: i64, member: bool) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).set_in_collection(collection, game, member).map_err(err)?;
    changed(&app);
    Ok(())
}
