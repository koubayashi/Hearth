#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use hub_core::{Db, Game, LibraryQuery};
use tauri::{Manager, State};

struct AppState {
    db: Mutex<Db>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn list_games(state: State<AppState>, query: LibraryQuery) -> CmdResult<Vec<Game>> {
    state.db.lock().unwrap().list_games(&query).map_err(err)
}

#[tauri::command]
fn get_game(state: State<AppState>, id: i64) -> CmdResult<Option<Game>> {
    state.db.lock().unwrap().game(id).map_err(err)
}

#[tauri::command]
fn set_hidden(state: State<AppState>, id: i64, hidden: bool) -> CmdResult<()> {
    state.db.lock().unwrap().set_game_hidden(id, hidden).map_err(err)
}

#[tauri::command]
fn set_favorite(state: State<AppState>, id: i64, favorite: bool) -> CmdResult<()> {
    state.db.lock().unwrap().set_favorite(id, favorite).map_err(err)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("library.db"))?;
            app.manage(AppState { db: Mutex::new(db) });

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                // Mica needs Windows 11; on Windows 10 the CSS background shows instead.
                let _ = window_vibrancy::apply_mica(&window, None);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_games, get_game, set_hidden, set_favorite])
        .run(tauri::generate_context!())
        .expect("error while running Hearth");
}
