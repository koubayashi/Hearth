#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artwork;
mod auth;
mod autobackup;
mod backup;
mod collections;
mod details;
mod drives;
mod errors;
mod folders;
mod gog_account;
mod jumplist;
mod saves;
mod smart;
mod snap_layouts;
mod presence;
mod rescan;
mod storage;
mod uninstall;
mod versions;
mod tracking;
mod tray;
mod webview_lock;
mod updates;
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
    pub(crate) gog: Arc<hub_providers::gog_web::GogWeb>,
    steam: Arc<Steam>,
    pub(crate) metadata: hub_providers::metadata::Metadata,
    pub(crate) folders: Arc<folders::FolderProvider>,
    pub(crate) sgdb: Arc<hub_providers::sgdb::SteamGridDb>,
    pub(crate) saves: hub_providers::saves::Saves,
    pub(crate) wikidata: hub_providers::wikidata::Wikidata,
    providers: Vec<Arc<dyn LibraryProvider>>,
    syncing: Mutex<SyncState>,
}

/// Whether a library sync is running, and whether another was asked for
/// meanwhile (a renewed sign-in mid-sync), so that one runs after it.
#[derive(Default)]
struct SyncState {
    busy: bool,
    again: bool,
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

// Off the UI thread, as `game_counts`: both read the whole library.
#[tauri::command(async)]
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

#[tauri::command(async)]
fn game_counts(state: State<AppState>) -> CmdResult<GameCounts> {
    let (library, hidden) = lock(&state.db).game_counts().map_err(err)?;
    Ok(GameCounts { library, hidden })
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
    play(&state, id, via)
}

pub(crate) fn play(state: &AppState, id: i64, via: Option<ways::Via>) -> CmdResult<()> {
    let game = lock(&state.db)
        .game(id)
        .map_err(err)?
        .ok_or("That game is no longer in your library")?;
    let version = match via {
        Some(ways::Via::Entry { id }) => Some(
            game.entries
                .iter()
                .find(|e| e.id == id)
                .ok_or("That version is no longer part of this game")?,
        ),
        _ => None,
    };
    let chosen = match via {
        Some(ways::Via::Store) | Some(ways::Via::Entry { .. }) => None,
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
    // A version picked by hand that isn't on the missing drive (another
    // store's, or one to install) doesn't need that drive.
    let elsewhere = version.is_some_and(|e| !e.offline);
    if let Some(drive) = game.offline_drive.as_ref().filter(|_| !elsewhere) {
        return Err(format!("{} is on drive {drive}, which isn't connected. Connect it to play.", game.title));
    }
    let entry = version
        .or_else(|| ways::installed_to_play(&game.entries))
        // Nothing installed: install from a store, not from a deleted folder.
        .or_else(|| game.entries.iter().find(|e| e.store != hub_core::StoreId::Manual))
        .or(game.entries.first())
        .ok_or("This game has no store to launch it from")?;
    let provider = state
        .providers
        .iter()
        .find(|p| p.id() == entry.store)
        .ok_or_else(|| format!("{} isn't supported yet", entry.store.as_str()))?;
    let picked = entry.installed && !entry.offline && entry.store == hub_core::StoreId::Manual;
    if let Some((dir, exe)) = entry.install_dir.as_deref().zip(entry.launch_exe.as_deref()).filter(|_| picked) {
        let path = ways::folder_exe(dir, exe)
            .ok_or_else(|| format!("{exe} isn't in {}'s folder any more. Choose its program again.", game.title))?;
        return hub_launch::run_program(&path, "", None).map_err(err);
    }
    let action = if entry.installed && !entry.offline {
        provider.launch(&entry.store_game_id)
    } else if entry.store == hub_core::StoreId::Manual {
        return Err(format!(
            "{}'s folder is gone. Put it back in a game folder Hearth watches to play it.",
            game.title
        ));
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
fn delete_play_session(app: AppHandle, state: State<AppState>, id: i64) -> CmdResult<()> {
    lock(&state.db).delete_session(id).map_err(err)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn now_playing(app: AppHandle) -> Vec<tracking::NowPlaying> {
    tracking::now_playing(&app)
}

/// Pause from the tray or a game's panel; it resumes by itself once you're
/// back in the game.
#[tauri::command]
async fn pause_timing(app: AppHandle, id: i64) {
    tracking::pause(&app, id);
}

#[tauri::command]
async fn resume_timing(app: AppHandle, id: i64) {
    tracking::resume(&app, id);
}

/// Hearth has it wrong: stop timing this game until it closes.
#[tauri::command]
async fn stop_timing(app: AppHandle, id: i64) {
    tracking::stop(&app, id);
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
    /// The GOG username when signed in to GOG.
    gog: Option<String>,
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
        gog: state.gog.signed_in().then(|| state.gog.username().unwrap_or_else(|| "your GOG account".into())),
    }
}

fn save_steam_auth(app: &AppHandle, auth: Option<SteamAuth>) {
    let json = auth.as_ref().and_then(|a| serde_json::to_string(a).ok());
    if let Err(e) = secrets::save("steam-auth", json.as_deref()) {
        tracking::log(app, &format!("steam: couldn't save the sign-in ({e})"));
    }
    // The bare key older builds kept is carried over at startup: once the
    // sign-in is saved here (or removed), it must not come back.
    let _ = secrets::save("steam-api-key", None);
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
    let count = steam.check_auth(&auth).await.map_err(|e| crate::tracking::redact(&format!("{e:#}")))?;
    save_steam_auth(&app, Some(auth));
    // A key needs no website session: drop any left from signing in.
    auth::forget(&app, auth::Store::Steam);
    sync_all(app);
    Ok(count)
}

#[tauri::command]
fn steam_sign_out(app: AppHandle) {
    save_steam_auth(&app, None);
    auth::forget(&app, auth::Store::Steam);
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
/// to the app, in that store's own browser profile ([`auth`]). When the page
/// at `capture` loads, its text is handed back by navigating to a made-up
/// host that is intercepted and never loaded, under a secret path only
/// Hearth's script knows. A hidden window reuses the saved browser session
/// without showing anything.
#[allow(clippy::too_many_arguments)]
fn open_auth_window(
    app: &AppHandle,
    label: &'static str,
    store: auth::Store,
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
    let profile = auth::profile(app, store).ok_or("No folder for the sign-in window")?;
    let nonce = auth::nonce();
    let answer_path = format!("/{nonce}");
    let script = format!(
        "location.replace('https://hearth.invalid/{nonce}?body=' + encodeURIComponent(document.body.innerText))"
    );
    WebviewWindowBuilder::new(app, label, WebviewUrl::External(url.parse().map_err(err)?))
        .data_directory(profile)
        .title(title)
        .inner_size(900.0, 860.0)
        .min_inner_size(480.0, 600.0)
        .resizable(true)
        .center()
        .visible(visible)
        // "Sign in with Google / Steam / Xbox…" opens the provider in a popup
        // that reports back to the page that opened it. Tauri blocks popups by
        // default, which made those buttons do nothing. Allow them as normal
        // browser popups, but only on the store's own sign-in pages; any
        // other link opens in the real browser, as in `on_navigation`.
        .on_new_window(move |url, _features| {
            if auth::allowed(store, &url) {
                return tauri::webview::NewWindowResponse::Allow;
            }
            if url.scheme() == "https" {
                let _ = hub_launch::run(&hub_launch::LaunchAction::Uri { uri: url.to_string() });
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .on_navigation(move |url| {
            if url.host_str() == Some("hearth.invalid") {
                // Any page can navigate here; only the one carrying the
                // secret is the store's answer.
                if url.path() == answer_path {
                    let body = url
                        .query_pairs()
                        .find(|(k, _)| k == "body")
                        .map(|(_, v)| v.into_owned())
                        .unwrap_or_default();
                    on_body(handle.clone(), body);
                }
                return false;
            }
            // Microsoft puts the code in the address it redirects to, and
            // that page has nothing to read: hand over the address itself.
            if url.as_str().starts_with(capture) && url.query_pairs().any(|(k, _)| k == "code") {
                on_body(handle.clone(), url.to_string());
                return false;
            }
            if auth::allowed(store, url) {
                return true;
            }
            // Help pages and other links leave the sign-in window for the
            // real browser, where the address shows.
            if url.scheme() == "https" {
                let _ = hub_launch::run(&hub_launch::LaunchAction::Uri { uri: url.to_string() });
            }
            false
        })
        .on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Finished && payload.url().as_str().starts_with(capture) {
                let _ = window.eval(&script);
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
    open_auth_window(&app, "steam-login", auth::Store::Steam, "Sign in to Steam", steam::LOGIN_URL, steam::TOKEN_URL, true, |app, body| {
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
                let _ = app.emit("sync-error", crate::tracking::redact(&format!("{e:#}")));
            }
        }
    });
}

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Counts each silent sign-in, so a give-up timer only closes the hidden
/// window of its own attempt, never a newer one.
static STEAM_TRIES: AtomicU64 = AtomicU64::new(0);
static EPIC_TRIES: AtomicU64 = AtomicU64::new(0);
/// When Epic last signed in silently (unix seconds).
static EPIC_SILENT_AT: AtomicI64 = AtomicI64::new(0);

/// Session tokens last about a day. The sign-in window keeps Steam's cookies,
/// so a hidden window can usually fetch a fresh token without asking.
fn steam_refresh_silently(app: &AppHandle) {
    let attempt = STEAM_TRIES.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = open_auth_window(app, "steam-refresh", auth::Store::Steam, "Steam", steam::TOKEN_URL, steam::TOKEN_URL, false, |app, body| {
        close_window(&app, "steam-refresh");
        match steam::token_from_body(&body) {
            // Checked before it's kept: a token Steam then refuses would
            // otherwise sync, expire and open this window again, forever.
            Some(token) => finish_steam_sign_in(app, token),
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
        // A newer attempt owns the window now, and its own timer.
        if STEAM_TRIES.load(Ordering::SeqCst) == attempt && app.get_webview_window("steam-refresh").is_some() {
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
    // Signed in silently minutes ago and already refused again: Epic is
    // rejecting fresh sign-ins, so stop instead of looping sync, expire,
    // sign in. A later sync tries again.
    if app.get_webview_window("epic-refresh").is_none() && now() - EPIC_SILENT_AT.load(Ordering::SeqCst) < 10 * 60 {
        tracking::log(app, "epic: refused again right after signing in silently; not retrying yet");
        let _ = app.emit("sync-error", epic::SESSION_EXPIRED);
        return;
    }
    let attempt = EPIC_TRIES.fetch_add(1, Ordering::SeqCst) + 1;
    let url = epic::login_url();
    let _ = open_auth_window(
        app,
        "epic-refresh",
        auth::Store::Epic,
        "Epic Games",
        &url,
        "https://www.epicgames.com/id/api/redirect",
        false,
        |app, body| {
            close_window(&app, "epic-refresh");
            match epic::code_from_redirect_body(&body) {
                Some(code) => {
                    tracking::log(&app, "epic: signed in again silently");
                    EPIC_SILENT_AT.store(now(), Ordering::SeqCst);
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
        // A newer attempt owns the window now, and its own timer.
        if EPIC_TRIES.load(Ordering::SeqCst) == attempt && app.get_webview_window("epic-refresh").is_some() {
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
                Err(e) if hub_providers::is_session_expired(&e, epic::SESSION_EXPIRED) => epic_sign_in_silently(&app),
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
    auth::forget(app, auth::Store::Epic);
    let _ = app.emit("accounts-changed", ());
    let _ = app.emit("sync-error", epic::SESSION_EXPIRED);
}

#[tauri::command]
async fn epic_sign_out(app: AppHandle) -> CmdResult<()> {
    let epic = app.state::<AppState>().epic.clone();
    epic.sign_out().await;
    auth::forget(&app, auth::Store::Epic);
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
        auth::Store::Epic,
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
    epic.sign_in(&code).await.map_err(|e| crate::tracking::redact(&format!("{e:#}")))?;
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
                let _ = app.emit("sync-error", crate::tracking::redact(&e.to_string()));
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
        // Saved details cost nothing; asking Steam waits a second after, as
        // the prefetch does, or a big library of folder games hits its limit.
        let asking = details::stale(app, id);
        let fetched = details::fetch(app, id).await;
        if asking {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        let Ok((details, described)) = fetched else { continue };
        changed |= described;
        if let Some(d) = details {
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

/// Background work that runs one at a time: asked for again while it runs
/// (a second sync, "Reset artwork" clicked twice), it runs once more after,
/// rather than a second copy beside it.
struct OneAtATime(Mutex<(bool, bool)>);

impl OneAtATime {
    const fn new() -> Self {
        Self(Mutex::new((false, false)))
    }

    async fn run<F: std::future::Future<Output = ()>>(&'static self, mut work: impl FnMut() -> F) {
        {
            let mut s = lock(&self.0);
            if s.0 {
                s.1 = true;
                return;
            }
            s.0 = true;
        }
        /// Frees it if the work panics, which would otherwise stop it for
        /// good until a restart.
        struct Running(&'static OneAtATime);
        impl Drop for Running {
            fn drop(&mut self) {
                if std::thread::panicking() {
                    *lock(&self.0 .0) = (false, false);
                }
            }
        }
        let _running = Running(self);
        loop {
            work().await;
            let mut s = lock(&self.0);
            if !std::mem::take(&mut s.1) {
                s.0 = false;
                return;
            }
        }
    }
}

/// Store artwork first (so every game has something), then SteamGridDB's.
pub(crate) async fn refresh_art(app: &AppHandle) {
    static ART: OneAtATime = OneAtATime::new();
    ART.run(|| async {
        fill_missing_art(app).await;
        refresh_steam_art(app).await;
        artwork::apply_automatically(app).await;
    })
    .await;
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
    let _ = db.in_transaction(|db| {
        for (game, app_id) in targets {
            let Some(a) = u64::try_from(app_id).ok().and_then(|id| art.get(&id)) else { continue };
            if let Some(cover) = &a.cover {
                let hero = a.hero.as_deref().unwrap_or(cover);
                changed |= db.set_game_art(game, cover, hero, a.logo.as_deref().unwrap_or("")).unwrap_or(false);
            }
        }
        Ok(())
    });
    drop(db);
    if changed {
        let _ = app.emit("library-changed", ());
    }
}

/// Re-reads every store in the background and tells the UI when anything changed.
pub(crate) fn sync_all(app: AppHandle) {
    {
        let state = app.state::<AppState>();
        let mut sync = lock(&state.syncing);
        if sync.busy {
            sync.again = true;
            return;
        }
        sync.busy = true;
    }
    /// Clears `busy` however the sync ends, even by a panic, which would
    /// otherwise leave the library never syncing again until a restart.
    struct Busy(AppHandle);
    impl Drop for Busy {
        fn drop(&mut self) {
            lock(&self.0.state::<AppState>().syncing).busy = false;
            let _ = self.0.emit("sync-finished", ());
        }
    }
    tauri::async_runtime::spawn(async move {
        let busy = Busy(app.clone());
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
                Err(e) if store == hub_core::StoreId::Steam && hub_providers::is_session_expired(&e, steam::SESSION_EXPIRED) => {
                    steam_refresh_silently(&app);
                    Vec::new()
                }
                Err(e) if store == hub_core::StoreId::Epic && hub_providers::is_session_expired(&e, epic::SESSION_EXPIRED) => {
                    epic_sign_in_silently(&app);
                    Vec::new()
                }
                Err(e) if store == hub_core::StoreId::Xbox && hub_providers::is_session_expired(&e, hub_providers::xbox_live::SESSION_EXPIRED) => {
                    xbox_account::give_up(&app);
                    Vec::new()
                }
                Err(e) if store == hub_core::StoreId::Gog && hub_providers::is_session_expired(&e, hub_providers::gog_web::SESSION_EXPIRED) => {
                    gog_account::give_up(&app);
                    Vec::new()
                }
                Err(e) => {
                    let _ = app.emit("sync-error", crate::tracking::redact(&format!("{e:#}")));
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
            // One transaction: one disk flush for the whole store, not two per game.
            let _ = db.in_transaction(|db| {
                for g in &games {
                    let art = provider.artwork(&g.store_game_id);
                    let _ = db.set_art_if_missing(
                        store,
                        &g.store_game_id,
                        art.cover.as_deref(),
                        art.hero.as_deref(),
                        art.logo.as_deref(),
                    );
                    let described = provider.description(&g.store_game_id);
                    if let Some(d) = described.and_then(|d| hub_providers::metadata::useful_description(&g.title, &d)) {
                        let _ = db.set_description_if_missing(store, &g.store_game_id, &d);
                    }
                }
                Ok(())
            });
        }
        // Cleared here by hand (with `again`); the guard must not clear it
        // again later, when a new sync may have set it.
        let again = std::mem::take(&mut *lock(&state.syncing)).again;
        std::mem::forget(busy);
        if again {
            // The follow-up sync does the artwork and the rest; doing them
            // here too would run two of each side by side.
            sync_all(app.clone());
            return;
        }
        tracking::refresh_dirs(&app);
        let _ = app.emit("library-changed", ());
        // The stores are read; art and details below are background extras.
        let _ = app.emit("sync-finished", ());
        // Each of these says so itself when it changed something. A sync
        // finishing while they run (a renewed sign-in) has them run again
        // after, not a second set beside them.
        static EXTRAS: OneAtATime = OneAtATime::new();
        EXTRAS
            .run(|| async {
                refresh_art(&app).await;
                saves::refresh(&app).await;
                details::prefetch(&app).await;
                // After details: they give games from other stores a Steam id to match by.
                smart::refresh(&app).await;
            })
            .await;
    });
}

fn main() {
    jumplist::set_app_id();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if jumplist::handle_args(app, &args) {
                tray::show_main(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(tracking::Tracking::default())
        .manage(autobackup::AutoBackup::default())
        .manage(updates::Updates::default())
        .manage(backup::PendingWays::default())
        .manage(jumplist::Pending::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            errors::install_panic_hook(dir.clone());
            backup::put_back_aside(&dir);
            let db = Db::open(&dir.join("library.db"))?;
            // Re-check games that had no store match after an update (the
            // matcher may have improved); otherwise a miss waits its week.
            let version = app.package_info().version.to_string();
            let marker = dir.join("misses-cleared");
            if std::fs::read_to_string(&marker).ok().as_deref() != Some(version.as_str())
                && db.clear_detail_misses().is_ok()
            {
                let _ = std::fs::write(&marker, &version);
            }
            // Xbox art saved before it was asked for at a sensible size.
            let _ = db.rewrite_art(hub_providers::xbox_live::sized);
            // Store descriptions saved before they were checked for content.
            let _ = db.rewrite_descriptions(hub_providers::metadata::useful_description);

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
            let gog = gog_account::load(app.handle(), &dir);
            app.manage(AppState {
                db: Mutex::new(db),
                // Stores first, so the folder scan knows which folders they already own.
                providers: vec![
                    steam.clone(),
                    epic.clone(),
                    Arc::new(hub_providers::gog::Gog::detect(gog.clone())),
                    Arc::new(hub_providers::xbox::Xbox::new(xbox.clone())),
                    folders.clone(),
                ],
                xbox,
                gog,
                folders,
                saves: Default::default(),
                wikidata: Default::default(),
                sgdb: Arc::new(hub_providers::sgdb::SteamGridDb::new(secrets::load("sgdb-key"))),
                steam,
                metadata: Default::default(),
                epic,
                syncing: Default::default(),
            });

            // Website sessions only for stores still signed in through them.
            {
                let state = app.state::<AppState>();
                auth::forget_unused(app.handle(), |store| match store {
                    auth::Store::Steam => matches!(state.steam.auth(), Some(SteamAuth::Session { .. })),
                    auth::Store::Epic => state.epic.account_name().is_some(),
                    auth::Store::Xbox => state.xbox.signed_in(),
                    auth::Store::Gog => state.gog.signed_in(),
                });
            }

            if let Some(window) = app.get_webview_window("main") {
                tray::style_main(&window);
            }

            tray::install(app.handle())?;
            tracking::refresh_dirs(app.handle());
            // Start with Windows is on by default so the tracker always runs.
            // Done once; after that the Settings switch is the user's choice.
            let marker = dir.join("autostart-set");
            {
                use tauri_plugin_autostart::ManagerExt;
                if !marker.exists() {
                    if app.autolaunch().enable().is_ok() {
                        let _ = std::fs::write(&marker, "");
                    }
                } else if !cfg!(debug_assertions) && app.autolaunch().is_enabled().unwrap_or(false) {
                    // Point the entry at this exe: it may name a dev build, or
                    // an install that has since moved.
                    let _ = app.autolaunch().enable();
                }
            }

            keep_epic_alive(app.handle().clone());
            xbox_account::keep_alive(app.handle().clone());
            gog_account::keep_alive(app.handle().clone());
            drives::watch(app.handle().clone());
            autobackup::watch(app.handle().clone());
            updates::watch(app.handle().clone());
            tracking::recover(app.handle());
            tracking::start(app.handle().clone());
            presence::start();
            sync_all(app.handle().clone());

            // Started with Windows: stay in the tray, without even loading the
            // interface, until the user opens Hearth.
            // Started from the jump list with a game: it starts, Hearth stays
            // in the tray.
            // After an update the installer starts Hearth again with the old
            // command line: a jump-list game would start again by itself.
            let args: Vec<String> = if updates::restarted_for_update(app.handle()) {
                Vec::new()
            } else {
                std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect()
            };
            if args.iter().any(|a| a == "--minimized") || !jumplist::handle_args(app.handle(), &args) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.destroy();
                }
            } else {
                tray::show_main(app.handle());
            }
            jumplist::watch(app.handle());
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
            ways::set_folder_exe,
            snap_layouts::place_maximize_button,
            play_sessions,
            delete_play_session,
            backup::backup_export,
            backup::backup_restore,
            backup::restore_ways,
            autobackup::auto_backup_status,
            updates::update_status,
            updates::release_notes,
            updates::check_updates,
            updates::seen_update,
            updates::install_update,
            autobackup::auto_backup_set,
            autobackup::save_prefs_snapshot,
            accounts,
            sync_now,
            epic_sign_in,
            epic_sign_out,
            xbox_account::xbox_sign_in,
            xbox_account::xbox_sign_out,
            gog_account::gog_sign_in,
            gog_account::gog_sign_out,
            now_playing,
            pause_timing,
            resume_timing,
            stop_timing,
            tray::tray_menu_fit,
            tray::tray_menu_hide,
            tray::open_hearth,
            tray::quit_hearth,
            jumplist::take_jump_request,
            jumplist::taskbar_light,
            steam_set_api_key,
            steam_sign_in,
            steam_sign_out,
            open_steam_key_page,
            epic_open_browser,
            epic_submit_code,
            details::game_details,
            details::install_info,
            details::saved_game_info,
            details::open_install_folder,
            storage::storage_drives,
            uninstall::uninstall_plan,
            uninstall::uninstall_game,
            versions::combine_games,
            versions::separate_version,
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
            artwork::store_artwork,
            smart::smart_index,
            smart::set_game_link,
            smart::create_series,
            smart::set_entity_parent,
            artwork::reset_artwork,
            collections::set_status,
            collections::set_idle_mode,
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
