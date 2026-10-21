#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use hub_core::{Db, Game, LibraryQuery};
use hub_providers::{steam::Steam, LibraryProvider};
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    db: Mutex<Db>,
    providers: Vec<Arc<dyn LibraryProvider>>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

/// Plays the game if any store has it installed, otherwise starts its install.
#[tauri::command]
fn launch_game(state: State<AppState>, id: i64) -> CmdResult<()> {
    let game = state
        .db
        .lock()
        .unwrap()
        .game(id)
        .map_err(err)?
        .ok_or("That game is no longer in your library")?;
    let entry = game
        .entries
        .iter()
        .find(|e| e.installed)
        .or(game.entries.first())
        .ok_or("This game has no store to launch it from")?;
    let provider = state
        .providers
        .iter()
        .find(|p| p.id() == entry.store)
        .ok_or_else(|| format!("{} isn't supported yet", entry.store.as_str()))?;
    let action = if entry.installed {
        provider.launch(&entry.store_game_id)
    } else {
        provider
            .install(&entry.store_game_id)
            .ok_or("This store can't install games from Hearth yet")?
    };
    hub_launch::run(&action).map_err(err)
}

/// Re-reads every store in the background and tells the UI when anything changed.
fn sync_all(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        for provider in state.providers.clone() {
            let games = match provider.installed().await {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("sync {}: {e}", provider.id().as_str());
                    continue;
                }
            };
            let mut db = state.db.lock().unwrap();
            if let Err(e) = db.sync_store(provider.id(), &games, now()) {
                eprintln!("sync {}: {e}", provider.id().as_str());
                continue;
            }
            for g in &games {
                let art = provider.artwork(&g.store_game_id);
                let _ = db.set_art_if_missing(
                    provider.id(),
                    &g.store_game_id,
                    art.cover.as_deref(),
                    art.hero.as_deref(),
                    art.logo.as_deref(),
                );
            }
        }
        let _ = app.emit("library-changed", ());
    });
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("library.db"))?;
            app.manage(AppState {
                db: Mutex::new(db),
                providers: vec![Arc::new(Steam::detect())],
            });

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                // Mica needs Windows 11; on Windows 10 the CSS background shows instead.
                let _ = window_vibrancy::apply_mica(&window, None);
            }

            sync_all(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            get_game,
            set_hidden,
            set_favorite,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hearth");
}
