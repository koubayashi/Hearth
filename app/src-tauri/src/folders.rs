//! The user's game folders: where they live on disk and the commands that
//! change them. Each folder is scanned by `hub_providers::folders`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hub_providers::folders::Folders;
use tauri::{AppHandle, Manager};

use crate::{err, lock, sync_all, AppState, CmdResult};

pub type FolderProvider = Mutex<Folders>;

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("game-folders.json"))
}

pub fn load(app_data: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(app_data.join("game-folders.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn save(app: &AppHandle, folders: &[String]) -> CmdResult<()> {
    let path = config_path(app).ok_or("No app data folder")?;
    std::fs::write(path, serde_json::to_string_pretty(folders).map_err(err)?).map_err(err)
}

fn current(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let dirs = lock(&state.folders).roots().to_vec();
    dirs.into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

fn set(app: &AppHandle, folders: Vec<String>) -> CmdResult<Vec<String>> {
    save(app, &folders)?;
    lock(&app.state::<AppState>().folders).set_roots(folders.iter().map(PathBuf::from).collect());
    sync_all(app.clone());
    Ok(folders)
}

#[tauri::command]
pub fn game_folders(app: AppHandle) -> Vec<String> {
    current(&app)
}

#[tauri::command]
pub fn add_game_folder(app: AppHandle, path: String) -> CmdResult<Vec<String>> {
    let path = path.trim().trim_end_matches(['\\', '/']).to_string();
    if !Path::new(&path).is_dir() {
        return Err(format!("{path} isn't a folder on this PC"));
    }
    let mut folders = current(&app);
    if folders.iter().any(|f| f.eq_ignore_ascii_case(&path)) {
        return Ok(folders);
    }
    folders.push(path);
    set(&app, folders)
}

#[tauri::command]
pub fn remove_game_folder(app: AppHandle, path: String) -> CmdResult<Vec<String>> {
    let folders = current(&app).into_iter().filter(|f| !f.eq_ignore_ascii_case(&path)).collect();
    set(&app, folders)
}
