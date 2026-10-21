#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tracking;
mod tray;

use std::sync::{Arc, Mutex};

use hub_core::{Db, Game, LibraryQuery};
use hub_providers::epic::{self, Epic};
use hub_providers::{merge_entries, secrets, steam::Steam, LibraryProvider};
use serde::Serialize;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub(crate) struct AppState {
    pub(crate) db: Mutex<Db>,
    epic: Arc<Epic>,
    providers: Vec<Arc<dyn LibraryProvider>>,
    syncing: Mutex<bool>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub(crate) fn now() -> i64 {
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

#[tauri::command]
fn now_playing(app: AppHandle) -> Vec<tracking::NowPlaying> {
    tracking::now_playing(&app)
}

#[derive(Serialize)]
struct Accounts {
    epic: Option<String>,
}

#[tauri::command]
fn accounts(state: State<AppState>) -> Accounts {
    Accounts { epic: state.epic.account_name() }
}

#[tauri::command]
fn sync_now(app: AppHandle) {
    sync_all(app);
}

#[tauri::command]
async fn epic_sign_out(app: AppHandle) -> CmdResult<()> {
    let epic = app.state::<AppState>().epic.clone();
    epic.sign_out().await;
    let _ = app.emit("accounts-changed", ());
    Ok(())
}

/// Opens Epic's own sign-in page in a separate window with no access to the
/// app. When Epic shows the one-time code, it's caught here and exchanged.
#[tauri::command]
fn epic_sign_in(app: AppHandle) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window("epic-login") {
        let _ = w.set_focus();
        return Ok(());
    }
    let url = epic::login_url().parse().map_err(err)?;
    let on_nav = app.clone();
    WebviewWindowBuilder::new(&app, "epic-login", WebviewUrl::External(url))
        .title("Sign in to Epic Games")
        .inner_size(520.0, 760.0)
        .center()
        .on_navigation(move |url| {
            // The redirect page hands its JSON to this made-up host; never load it.
            if url.host_str() == Some("hearth.invalid") {
                let body = url
                    .query_pairs()
                    .find(|(k, _)| k == "body")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                if let Some(code) = epic::code_from_redirect_body(&body) {
                    finish_epic_sign_in(on_nav.clone(), code);
                }
                return false;
            }
            true
        })
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished
                && payload.url().as_str().starts_with("https://www.epicgames.com/id/api/redirect")
            {
                let _ = window.eval(
                    "location.replace('https://hearth.invalid/?body=' + encodeURIComponent(document.body.innerText))",
                );
            }
        })
        .build()
        .map_err(err)?;
    Ok(())
}

fn finish_epic_sign_in(app: AppHandle, code: String) {
    tauri::async_runtime::spawn(async move {
        let epic = app.state::<AppState>().epic.clone();
        let result = epic.sign_in(&code).await;
        if let Some(w) = app.get_webview_window("epic-login") {
            let _ = w.close();
        }
        match result {
            Ok(()) => {
                let _ = app.emit("accounts-changed", ());
                sync_all(app);
            }
            Err(e) => {
                let _ = app.emit("sync-error", e.to_string());
            }
        }
    });
}

/// Re-reads every store in the background and tells the UI when anything changed.
fn sync_all(app: AppHandle) {
    {
        let state = app.state::<AppState>();
        let mut busy = state.syncing.lock().unwrap();
        if *busy {
            return;
        }
        *busy = true;
    }
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _ = app.emit("sync-started", ());
        for provider in state.providers.clone() {
            let store = provider.id();
            let installed = provider.installed().await.unwrap_or_else(|e| {
                eprintln!("sync {}: {e}", store.as_str());
                Vec::new()
            });
            let owned = match provider.owned().await {
                Ok(g) => g,
                Err(e) => {
                    let _ = app.emit("sync-error", format!("{e:#}"));
                    Vec::new()
                }
            };
            let games = merge_entries(installed, owned);

            let mut db = state.db.lock().unwrap();
            let _ = db.remove_entries(store, provider.excluded_ids());
            if let Err(e) = db.sync_store(store, &games, now()) {
                eprintln!("sync {}: {e}", store.as_str());
                continue;
            }
            for g in &games {
                let art = provider.artwork(&g.store_game_id);
                let _ = db.set_art_if_missing(
                    store,
                    &g.store_game_id,
                    art.cover.as_deref(),
                    art.hero.as_deref(),
                    art.logo.as_deref(),
                );
            }
        }
        *state.syncing.lock().unwrap() = false;
        tracking::refresh_dirs(&app);
        let _ = app.emit("library-changed", ());
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(tracking::Tracking::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("library.db"))?;

            let session = secrets::load("epic").and_then(|s| serde_json::from_str(&s).ok());
            let epic = Arc::new(Epic::new(
                &dir,
                session,
                Box::new(|s| {
                    let json = s.and_then(|s| serde_json::to_string(s).ok());
                    secrets::save("epic", json.as_deref());
                }),
            ));

            app.manage(AppState {
                db: Mutex::new(db),
                providers: vec![Arc::new(Steam::detect()), epic.clone()],
                epic,
                syncing: Mutex::new(false),
            });

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                // Mica needs Windows 11; on Windows 10 the CSS background shows instead.
                let _ = window_vibrancy::apply_mica(&window, None);
            }

            tray::install(app.handle())?;
            tracking::refresh_dirs(app.handle());
            tracking::start(app.handle().clone());
            sync_all(app.handle().clone());

            // Started with Windows: stay in the tray until the user opens Hearth.
            if !std::env::args().any(|a| a == "--minimized") {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Hearth in the tray so playtime keeps counting.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            get_game,
            set_hidden,
            set_favorite,
            launch_game,
            accounts,
            sync_now,
            epic_sign_in,
            epic_sign_out,
            now_playing
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hearth");
}
