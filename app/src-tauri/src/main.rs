#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod details;
mod folders;
mod tracking;
mod tray;

use std::sync::{Arc, Mutex};

use hub_core::{Db, Game, LibraryQuery};
use hub_providers::epic::{self, Epic};
use hub_providers::steam::{self, Steam, SteamAuth};
use hub_providers::{merge_entries, secrets, LibraryProvider};
use serde::Serialize;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub(crate) struct AppState {
    pub(crate) db: Mutex<Db>,
    epic: Arc<Epic>,
    steam: Arc<Steam>,
    pub(crate) metadata: hub_providers::metadata::Metadata,
    pub(crate) folders: Arc<folders::FolderProvider>,
    providers: Vec<Arc<dyn LibraryProvider>>,
    syncing: Mutex<bool>,
}

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Locks a mutex even if an earlier panic poisoned it: one failed task must
/// never leave the whole library unreadable.
pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
fn list_games(state: State<AppState>, query: LibraryQuery) -> CmdResult<Vec<Game>> {
    lock(&state.db).list_games(&query).map_err(err)
}

#[tauri::command]
fn get_game(state: State<AppState>, id: i64) -> CmdResult<Option<Game>> {
    lock(&state.db).game(id).map_err(err)
}

#[tauri::command]
fn set_hidden(state: State<AppState>, id: i64, hidden: bool) -> CmdResult<()> {
    lock(&state.db).set_game_hidden(id, hidden).map_err(err)
}

#[tauri::command]
fn set_favorite(state: State<AppState>, id: i64, favorite: bool) -> CmdResult<()> {
    lock(&state.db).set_favorite(id, favorite).map_err(err)
}

/// Plays the game if any store has it installed, otherwise starts its install.
#[tauri::command]
fn launch_game(state: State<AppState>, id: i64) -> CmdResult<()> {
    let game = lock(&state.db)
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
    /// The Steam account Hearth reads, if any.
    steam: Option<String>,
    /// "session", "api_key", or null when Hearth can only see installed games.
    steam_auth: Option<&'static str>,
}

#[tauri::command]
fn accounts(state: State<AppState>) -> Accounts {
    Accounts {
        epic: state.epic.account_name(),
        steam: state.steam.account().map(|a| a.name),
        steam_auth: state.steam.auth().map(|a| match a {
            SteamAuth::Session { .. } => "session",
            SteamAuth::ApiKey { .. } => "api_key",
        }),
    }
}

fn save_steam_auth(app: &AppHandle, auth: Option<SteamAuth>) {
    let json = auth.as_ref().and_then(|a| serde_json::to_string(a).ok());
    secrets::save("steam-auth", json.as_deref());
    app.state::<AppState>().steam.set_auth(auth);
    let _ = app.emit("accounts-changed", ());
}

/// Advanced alternative to signing in. Validated before it's kept, so a typo
/// never silently empties the library.
#[tauri::command]
async fn steam_set_api_key(app: AppHandle, key: String) -> CmdResult<usize> {
    let key = key.trim().to_string();
    if key.len() != 32 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("A Steam Web API key is 32 letters and numbers. Copy it from steamcommunity.com/dev/apikey.".into());
    }
    let auth = SteamAuth::ApiKey { key };
    let steam = app.state::<AppState>().steam.clone();
    let count = steam.check_auth(&auth).await.map_err(|e| format!("{e:#}"))?;
    save_steam_auth(&app, Some(auth));
    sync_all(app);
    Ok(count)
}

#[tauri::command]
fn steam_sign_out(app: AppHandle) {
    save_steam_auth(&app, None);
}

/// Opens Steam's API key page in the default browser. Only this fixed URL.
#[tauri::command]
fn open_steam_key_page() -> CmdResult<()> {
    hub_launch::run(&hub_launch::LaunchAction::Uri { uri: "https://steamcommunity.com/dev/apikey".into() }).map_err(err)
}

#[tauri::command]
fn sync_now(app: AppHandle) {
    sync_all(app);
}

/// Opens a store's own sign-in page in a separate window that has no access
/// to the app. When the page at `capture` loads, its text is handed back by
/// navigating to a made-up host that is intercepted and never loaded.
/// A hidden window reuses the saved browser session without showing anything.
fn open_auth_window(
    app: &AppHandle,
    label: &'static str,
    title: &str,
    url: &str,
    capture: &'static str,
    visible: bool,
    on_body: impl Fn(AppHandle, String) + Send + Sync + 'static,
) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window(label) {
        if visible {
            let _ = w.show();
            let _ = w.set_focus();
        }
        return Ok(());
    }
    let handle = app.clone();
    WebviewWindowBuilder::new(app, label, WebviewUrl::External(url.parse().map_err(err)?))
        .title(title)
        .inner_size(900.0, 860.0)
        .min_inner_size(480.0, 600.0)
        .resizable(true)
        .center()
        .visible(visible)
        .on_navigation(move |url| {
            if url.host_str() == Some("hearth.invalid") {
                let body = url
                    .query_pairs()
                    .find(|(k, _)| k == "body")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                on_body(handle.clone(), body);
                return false;
            }
            true
        })
        .on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Finished && payload.url().as_str().starts_with(capture) {
                let _ = window.eval(
                    "location.replace('https://hearth.invalid/?body=' + encodeURIComponent(document.body.innerText))",
                );
            }
        })
        .build()
        .map_err(err)?;
    Ok(())
}

fn close_window(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.close();
    }
}

// Sign-in commands are async on purpose: on Windows, building a window inside
// a synchronous command deadlocks the main thread (the app freezes).

/// Sign in on Steam's real login page (password, Steam Guard or QR code).
#[tauri::command]
async fn steam_sign_in(app: AppHandle) -> CmdResult<()> {
    open_auth_window(&app, "steam-login", "Sign in to Steam", steam::LOGIN_URL, steam::TOKEN_URL, true, |app, body| {
        if let Some(token) = steam::token_from_body(&body) {
            close_window(&app, "steam-login");
            finish_steam_sign_in(app, token);
        }
        // No token yet means Steam bounced back to its login page; stay open.
    })
}

fn finish_steam_sign_in(app: AppHandle, token: String) {
    tauri::async_runtime::spawn(async move {
        let auth = SteamAuth::Session { token };
        let steam = app.state::<AppState>().steam.clone();
        match steam.check_auth(&auth).await {
            Ok(_) => {
                save_steam_auth(&app, Some(auth));
                sync_all(app);
            }
            Err(e) => {
                let _ = app.emit("sync-error", format!("{e:#}"));
            }
        }
    });
}

/// Session tokens last about a day. The sign-in window keeps Steam's cookies,
/// so a hidden window can usually fetch a fresh token without asking.
fn steam_refresh_silently(app: &AppHandle) {
    let _ = open_auth_window(app, "steam-refresh", "Steam", steam::TOKEN_URL, steam::TOKEN_URL, false, |app, body| {
        close_window(&app, "steam-refresh");
        match steam::token_from_body(&body) {
            Some(token) => {
                save_steam_auth(&app, Some(SteamAuth::Session { token }));
                sync_all(app);
            }
            None => {
                let _ = app.emit("sync-error", steam::SESSION_EXPIRED);
            }
        }
    });
    // If Steam redirects the hidden window to its login page instead, the
    // capture never fires: give up quietly after a while.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        if app.get_webview_window("steam-refresh").is_some() {
            close_window(&app, "steam-refresh");
            let _ = app.emit("sync-error", steam::SESSION_EXPIRED);
        }
    });
}

#[tauri::command]
async fn epic_sign_out(app: AppHandle) -> CmdResult<()> {
    let epic = app.state::<AppState>().epic.clone();
    epic.sign_out().await;
    let _ = app.emit("accounts-changed", ());
    Ok(())
}

/// Epic's own sign-in page in a Hearth window. Some providers (Google) refuse
/// embedded windows; for those, [`epic_open_browser`] is the way in.
#[tauri::command]
async fn epic_sign_in(app: AppHandle) -> CmdResult<()> {
    let url = epic::login_url();
    open_auth_window(
        &app,
        "epic-login",
        "Sign in to Epic Games",
        &url,
        "https://www.epicgames.com/id/api/redirect",
        true,
        |app, body| {
            if let Some(code) = epic::code_from_redirect_body(&body) {
                close_window(&app, "epic-login");
                finish_epic_sign_in(app, code);
            }
        },
    )
}

/// Opens Epic's sign-in in the default browser, where every sign-in option
/// works and the user is often already signed in. Epic then shows a one-time
/// code to paste back into Hearth.
#[tauri::command]
fn epic_open_browser() -> CmdResult<()> {
    hub_launch::run(&hub_launch::LaunchAction::Uri { uri: epic::login_url() }).map_err(err)
}

#[tauri::command]
async fn epic_submit_code(app: AppHandle, code: String) -> CmdResult<()> {
    let code = epic::clean_code(&code).map_err(err)?;
    let epic = app.state::<AppState>().epic.clone();
    epic.sign_in(&code).await.map_err(|e| format!("{e:#}"))?;
    let _ = app.emit("accounts-changed", ());
    sync_all(app);
    Ok(())
}

fn finish_epic_sign_in(app: AppHandle, code: String) {
    tauri::async_runtime::spawn(async move {
        let epic = app.state::<AppState>().epic.clone();
        match epic.sign_in(&code).await {
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

/// Games without artwork (folder games, Epic games while signed out) borrow
/// it from their Steam store page when the title matches exactly.
async fn fill_missing_art(app: &AppHandle) {
    let ids = lock(&app.state::<AppState>().db).games_missing_cover().unwrap_or_default();
    let mut changed = false;
    for id in ids {
        if let Ok(Some(d)) = details::game_details(app.clone(), id).await {
            let art = Steam::art_urls(&d.steam_app_id.to_string());
            if let (Some(c), Some(h), Some(l)) = (art.cover, art.hero, art.logo) {
                changed |= lock(&app.state::<AppState>().db).set_game_art_if_missing(id, &c, &h, &l).is_ok();
            }
        }
    }
    if changed {
        let _ = app.emit("library-changed", ());
    }
}

/// Re-reads every store in the background and tells the UI when anything changed.
pub(crate) fn sync_all(app: AppHandle) {
    {
        let state = app.state::<AppState>();
        let mut busy = lock(&state.syncing);
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
            if store == hub_core::StoreId::Manual {
                // Folders other stores own are theirs, even inside a game folder.
                let owned = lock(&state.db).store_install_dirs(hub_core::StoreId::Manual).unwrap_or_default();
                lock(&state.folders).set_skip(owned.into_iter().map(Into::into).collect());
            }
            let installed = provider.installed().await.unwrap_or_else(|e| {
                eprintln!("sync {}: {e}", store.as_str());
                Vec::new()
            });
            let owned = match provider.owned().await {
                Ok(g) => g,
                Err(e) if store == hub_core::StoreId::Steam && e.to_string() == steam::SESSION_EXPIRED => {
                    steam_refresh_silently(&app);
                    Vec::new()
                }
                Err(e) => {
                    let _ = app.emit("sync-error", format!("{e:#}"));
                    Vec::new()
                }
            };
            let games = merge_entries(installed, owned);

            let mut db = lock(&state.db);
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
        *lock(&state.syncing) = false;
        tracking::refresh_dirs(&app);
        let _ = app.emit("library-changed", ());
        fill_missing_art(&app).await;
        let _ = app.emit("library-changed", ());
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            // Re-check games that had no store match last time.
            let _ = db.clear_detail_misses();

            let session = secrets::load("epic").and_then(|s| serde_json::from_str(&s).ok());
            let epic = Arc::new(Epic::new(
                &dir,
                session,
                Box::new(|s| {
                    let json = s.and_then(|s| serde_json::to_string(s).ok());
                    secrets::save("epic", json.as_deref());
                }),
            ));

            // Older builds stored a bare API key; carry it over.
            let steam_auth = secrets::load("steam-auth")
                .and_then(|s| serde_json::from_str::<SteamAuth>(&s).ok())
                .or_else(|| secrets::load("steam-api-key").map(|key| SteamAuth::ApiKey { key }));
            let steam = Arc::new(Steam::detect(steam_auth));
            let folders = Arc::new(Mutex::new(hub_providers::folders::Folders::new(folders::load(&dir))));
            app.manage(AppState {
                db: Mutex::new(db),
                // Stores first, so the folder scan knows which folders they already own.
                providers: vec![steam.clone(), epic.clone(), folders.clone()],
                folders,
                steam,
                metadata: Default::default(),
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
            now_playing,
            steam_set_api_key,
            steam_sign_in,
            steam_sign_out,
            open_steam_key_page,
            epic_open_browser,
            epic_submit_code,
            details::game_details,
            details::install_info,
            details::open_install_folder,
            details::open_store_page,
            folders::game_folders,
            folders::add_game_folder,
            folders::remove_game_folder,
            details::search_store,
            details::set_store_match
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hearth");
}
