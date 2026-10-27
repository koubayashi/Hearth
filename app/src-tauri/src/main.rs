#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artwork;
mod autobackup;
mod backup;
mod collections;
mod details;
mod drives;
mod errors;
mod folders;
mod fullscreen_fix;
mod saves;
mod tracking;
mod tray;
mod ways;
mod xbox_account;

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
    pub(crate) xbox: Arc<hub_providers::xbox_live::XboxLive>,
    steam: Arc<Steam>,
    pub(crate) metadata: hub_providers::metadata::Metadata,
    pub(crate) folders: Arc<folders::FolderProvider>,
    pub(crate) sgdb: Arc<hub_providers::sgdb::SteamGridDb>,
    pub(crate) saves: hub_providers::saves::Saves,
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

/// How many games the library and Hidden hold, for the sidebar: the same
/// queries as their views, without sending every game across to count them.
#[derive(Serialize)]
struct GameCounts {
    library: usize,
    hidden: usize,
}

#[tauri::command]
fn game_counts(state: State<AppState>) -> CmdResult<GameCounts> {
    let db = lock(&state.db);
    let count = |hidden_only| {
        let q = LibraryQuery { hidden_only, ..Default::default() };
        db.list_games(&q).map(|g| g.len()).map_err(err)
    };
    Ok(GameCounts { library: count(false)?, hidden: count(true)? })
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

/// Plays the game: with the way to play the user picked (`via`), else its
/// default one, else through a store that has it installed, else starts its
/// install.
#[tauri::command]
fn launch_game(state: State<AppState>, id: i64, via: Option<ways::Via>) -> CmdResult<()> {
    let game = lock(&state.db)
        .game(id)
        .map_err(err)?
        .ok_or("That game is no longer in your library")?;
    let chosen = match via {
        Some(ways::Via::Store) => None,
        Some(ways::Via::Option { id }) => Some(
            game.launch_options
                .iter()
                .find(|o| o.id == id)
                .ok_or("That way to play was removed")?,
        ),
        None => game.launch_options.iter().find(|o| o.is_default),
    };
    if let Some(option) = chosen {
        return ways::run(option);
    }
    if let Some(drive) = &game.offline_drive {
        return Err(format!("{} is on drive {drive}, which isn't connected. Connect it to play.", game.title));
    }
    let entry = game
        .entries
        .iter()
        .find(|e| e.installed && !e.offline)
        .or(game.entries.first())
        .ok_or("This game has no store to launch it from")?;
    let provider = state
        .providers
        .iter()
        .find(|p| p.id() == entry.store)
        .ok_or_else(|| format!("{} isn't supported yet", entry.store.as_str()))?;
    let action = if entry.installed && !entry.offline {
        provider.launch(&entry.store_game_id)
    } else {
        provider
            .install(&entry.store_game_id)
            .ok_or("This store can't install games from Hearth yet")?
    };
    hub_launch::run(&action).map_err(err)
}

/// Recorded play sessions, for the Stats page.
#[tauri::command]
fn play_sessions(state: State<AppState>) -> CmdResult<Vec<hub_core::PlaySession>> {
    lock(&state.db).sessions().map_err(err)
}

#[tauri::command]
fn now_playing(app: AppHandle) -> Vec<tracking::NowPlaying> {
    tracking::now_playing(&app)
}

#[derive(Serialize)]
struct Accounts {
    epic: Option<String>,
    /// A SteamGridDB key is saved.
    sgdb: bool,
    /// The Steam account Hearth reads, if any.
    steam: Option<String>,
    /// "session", "api_key", or null when Hearth can only see installed games.
    steam_auth: Option<&'static str>,
    /// The Xbox gamertag when signed in to Xbox.
    xbox: Option<String>,
}

#[tauri::command]
fn accounts(state: State<AppState>) -> Accounts {
    Accounts {
        epic: state.epic.account_name(),
        sgdb: state.sgdb.has_key(),
        steam: state.steam.account().map(|a| a.name),
        steam_auth: state.steam.auth().map(|a| match a {
            SteamAuth::Session { .. } => "session",
            SteamAuth::ApiKey { .. } => "api_key",
        }),
        xbox: state.xbox.signed_in().then(|| state.xbox.gamertag().unwrap_or_else(|| "your Xbox account".into())),
    }
}

fn save_steam_auth(app: &AppHandle, auth: Option<SteamAuth>) {
    let json = auth.as_ref().and_then(|a| serde_json::to_string(a).ok());
    if let Err(e) = secrets::save("steam-auth", json.as_deref()) {
        tracking::log(app, &format!("steam: couldn't save the sign-in ({e})"));
    }
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
        // "Sign in with Google / Steam / Xbox…" opens the provider in a popup
        // that reports back to the page that opened it. Tauri blocks popups by
        // default, which made those buttons do nothing. Allow them as normal
        // browser popups, for secure pages only.
        .on_new_window(|url, _features| {
            if url.scheme() == "https" {
                tauri::webview::NewWindowResponse::Allow
            } else {
                tauri::webview::NewWindowResponse::Deny
            }
        })
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
            // Microsoft puts the code in the address it redirects to, and
            // that page has nothing to read: hand over the address itself.
            if url.as_str().starts_with(capture) && url.query_pairs().any(|(k, _)| k == "code") {
                on_body(handle.clone(), url.to_string());
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

/// Epic's refresh tokens last about 8 hours, so after a night off they're
/// gone. Epic's sign-in page remembers the user in its cookies, so a hidden
/// window usually gets a new code without asking; only if that fails is the
/// user asked to sign in again.
fn epic_sign_in_silently(app: &AppHandle) {
    let url = epic::login_url();
    let _ = open_auth_window(
        app,
        "epic-refresh",
        "Epic Games",
        &url,
        "https://www.epicgames.com/id/api/redirect",
        false,
        |app, body| {
            close_window(&app, "epic-refresh");
            match epic::code_from_redirect_body(&body) {
                Some(code) => {
                    tracking::log(&app, "epic: signed in again silently");
                    finish_epic_sign_in(app, code)
                }
                None => give_up_epic(&app, "Epic's page had no sign-in code"),
            }
        },
    );
    tracking::log(app, "epic: session expired, trying to sign in again in the background");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(90)).await;
        if app.get_webview_window("epic-refresh").is_some() {
            close_window(&app, "epic-refresh");
            give_up_epic(&app, "Epic didn't sign in by itself within 90s (it wants you to sign in)");
        }
    });
}

/// Renews Epic's tokens every 20 minutes while Hearth runs, so a sign-in
/// lasts as long as Hearth keeps running (which, from the tray, is all day).
fn keep_epic_alive(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let epic = app.state::<AppState>().epic.clone();
            match epic.keep_alive().await {
                Ok(Some(until)) => tracking::log(&app, &format!("epic: renewed sign-in (refresh token valid until {until})")),
                Ok(None) => {}
                Err(e) if e.to_string() == epic::SESSION_EXPIRED => epic_sign_in_silently(&app),
                // Offline: try again next round, keep the session.
                Err(e) => tracking::log(&app, &format!("epic: couldn't renew yet ({e})")),
            }
            tokio::time::sleep(std::time::Duration::from_secs(20 * 60)).await;
        }
    });
}

/// The saved refresh token was rejected and Epic didn't remember the browser:
/// a fresh sign-in is the only way back.
fn give_up_epic(app: &AppHandle, why: &str) {
    tracking::log(app, &format!("epic: signed out ({why})"));
    app.state::<AppState>().epic.forget();
    let _ = app.emit("accounts-changed", ());
    let _ = app.emit("sync-error", epic::SESSION_EXPIRED);
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

/// Store artwork first (so every game has something), then SteamGridDB's.
pub(crate) async fn refresh_art(app: &AppHandle) {
    fill_missing_art(app).await;
    refresh_steam_art(app).await;
    artwork::apply_automatically(app).await;
}

/// Puts the real Steam artwork addresses on every Steam-sourced cover.
async fn refresh_steam_art(app: &AppHandle) {
    let state = app.state::<AppState>();
    let targets = lock(&state.db).steam_art_targets().unwrap_or_default();
    if targets.is_empty() {
        return;
    }
    let mut ids: Vec<u64> = targets.iter().filter_map(|(_, a)| u64::try_from(*a).ok()).collect();
    ids.sort_unstable();
    ids.dedup();
    let Ok(art) = state.metadata.steam_art(&ids).await else { return };
    let db = lock(&state.db);
    let mut changed = false;
    for (game, app_id) in targets {
        let Some(a) = u64::try_from(app_id).ok().and_then(|id| art.get(&id)) else { continue };
        if let Some(cover) = &a.cover {
            let hero = a.hero.as_deref().unwrap_or(cover);
            changed |= db.set_game_art(game, cover, hero, a.logo.as_deref().unwrap_or("")).is_ok();
        }
    }
    drop(db);
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
                Err(e) if store == hub_core::StoreId::Epic && e.to_string() == epic::SESSION_EXPIRED => {
                    epic_sign_in_silently(&app);
                    Vec::new()
                }
                Err(e) if store == hub_core::StoreId::Xbox && e.to_string() == hub_providers::xbox_live::SESSION_EXPIRED => {
                    xbox_account::give_up(&app);
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
        refresh_art(&app).await;
        saves::refresh(&app).await;
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
        .manage(autobackup::AutoBackup::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            errors::install_panic_hook(dir.clone());
            let db = Db::open(&dir.join("library.db"))?;
            // Re-check games that had no store match last time.
            let _ = db.clear_detail_misses();
            // Xbox art saved before it was asked for at a sensible size.
            let _ = db.rewrite_art(hub_providers::xbox_live::sized);

            let session = secrets::load("epic").and_then(|s| serde_json::from_str(&s).ok());
            let handle = app.handle().clone();
            let epic = Arc::new(Epic::new(
                &dir,
                session,
                Box::new(move |s| {
                    let json = s.and_then(|s| serde_json::to_string(s).ok());
                    // A sign-in that can't be saved is lost on the next restart: say so.
                    if let Err(e) = secrets::save("epic", json.as_deref()) {
                        tracking::log(&handle, &format!("epic: couldn't save the sign-in ({e})"));
                    }
                }),
            ));

            // Older builds stored a bare API key; carry it over.
            let steam_auth = secrets::load("steam-auth")
                .and_then(|s| serde_json::from_str::<SteamAuth>(&s).ok())
                .or_else(|| secrets::load("steam-api-key").map(|key| SteamAuth::ApiKey { key }));
            let steam = Arc::new(Steam::detect(steam_auth));
            let folders = Arc::new(Mutex::new(hub_providers::folders::Folders::new(folders::load(&dir))));
            let xbox = xbox_account::load(app.handle());
            app.manage(AppState {
                db: Mutex::new(db),
                // Stores first, so the folder scan knows which folders they already own.
                providers: vec![
                    steam.clone(),
                    epic.clone(),
                    Arc::new(hub_providers::gog::Gog::detect()),
                    Arc::new(hub_providers::xbox::Xbox::new(xbox.clone())),
                    folders.clone(),
                ],
                xbox,
                folders,
                saves: Default::default(),
                sgdb: Arc::new(hub_providers::sgdb::SteamGridDb::new(secrets::load("sgdb-key"))),
                steam,
                metadata: Default::default(),
                epic,
                syncing: Mutex::new(false),
            });

            if let Some(window) = app.get_webview_window("main") {
                tray::style_main(&window);
            }

            tray::install(app.handle())?;
            tracking::refresh_dirs(app.handle());
            // Start with Windows is on by default so the tracker always runs.
            // Done once; after that the Settings switch is the user's choice.
            let marker = dir.join("autostart-set");
            if !marker.exists() {
                use tauri_plugin_autostart::ManagerExt;
                if app.autolaunch().enable().is_ok() {
                    let _ = std::fs::write(&marker, "");
                }
            }

            keep_epic_alive(app.handle().clone());
            xbox_account::keep_alive(app.handle().clone());
            drives::watch(app.handle().clone());
            autobackup::watch(app.handle().clone());
            tracking::recover(app.handle());
            tracking::start(app.handle().clone());
            sync_all(app.handle().clone());

            // Started with Windows: stay in the tray, without even loading the
            // interface, until the user opens Hearth.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.destroy();
                }
            } else {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            game_counts,
            get_game,
            set_hidden,
            set_favorite,
            launch_game,
            ways::save_launch_option,
            ways::delete_launch_option,
            ways::set_default_launch,
            play_sessions,
            backup::backup_export,
            backup::backup_restore,
            autobackup::auto_backup_status,
            autobackup::auto_backup_set,
            autobackup::save_prefs_snapshot,
            accounts,
            sync_now,
            epic_sign_in,
            epic_sign_out,
            xbox_account::xbox_sign_in,
            xbox_account::xbox_sign_out,
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
            details::set_store_match,
            artwork::sgdb_set_key,
            artwork::sgdb_clear_key,
            artwork::sgdb_copy_key,
            artwork::open_sgdb_key_page,
            artwork::artwork_options,
            artwork::set_artwork,
            artwork::reset_artwork,
            collections::set_status,
            collections::list_collections,
            collections::create_collection,
            collections::rename_collection,
            collections::delete_collection,
            collections::set_in_collection,
            errors::report_error,
            errors::open_error_log
        ])
        .build(tauri::generate_context!())
        .expect("error while building Hearth")
        .run(|app, event| match event {
            // Closing the window unloads the interface (most of Hearth's memory)
            // but keeps Hearth in the tray, so playtime keeps counting. Only
            // Quit, which passes an exit code, really exits.
            tauri::RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            // Windows shutdown, sign-out or Quit: save what is being played.
            tauri::RunEvent::Exit => tracking::flush(app),
            _ => {}
        });
}
