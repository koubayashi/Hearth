//! Ways to play: programs the user adds to a game besides its store's own
//! launch (a server replacement's start script, a mod loader, another exe).
//! Hearth only ever runs a file the user picked here.

use std::path::Path;

use hub_core::LaunchOption;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, AppState, CmdResult};

/// How to start a game when the user picks one explicitly.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Via {
    /// The store's own launch (or install).
    Store,
    /// One of the game's ways to play.
    Option { id: i64 },
}

fn changed(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
}

/// Starts a way to play.
pub fn run(option: &LaunchOption) -> CmdResult<()> {
    hub_launch::run_program(
        Path::new(&option.path),
        &option.args,
        option.working_dir.as_deref().map(Path::new),
    )
    .map_err(err)
}

#[derive(Deserialize)]
pub struct WayInput {
    id: Option<i64>,
    game_id: i64,
    name: String,
    path: String,
    #[serde(default)]
    args: String,
    working_dir: Option<String>,
}

#[tauri::command]
pub fn save_launch_option(app: AppHandle, option: WayInput) -> CmdResult<i64> {
    if option.name.trim().is_empty() {
        return Err("Give it a name, like \u{201c}Offline server\u{201d}.".into());
    }
    if !Path::new(option.path.trim()).is_file() {
        return Err("That program can't be found. Choose it again with Browse.".into());
    }
    if let Some(dir) = option.working_dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        if !Path::new(dir).is_dir() {
            return Err("That start folder can't be found.".into());
        }
    }
    let id = lock(&app.state::<AppState>().db)
        .save_launch_option(
            option.id,
            option.game_id,
            &option.name,
            &option.path,
            &option.args,
            option.working_dir.as_deref(),
        )
        .map_err(err)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn delete_launch_option(app: AppHandle, id: i64) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).delete_launch_option(id).map_err(err)?;
    changed(&app);
    Ok(())
}

/// What the Play button does: a way to play, or None for the store.
#[tauri::command]
pub fn set_default_launch(app: AppHandle, game_id: i64, option: Option<i64>) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).set_default_launch(game_id, option).map_err(err)?;
    changed(&app);
    Ok(())
}
