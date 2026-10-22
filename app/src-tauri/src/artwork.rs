//! SteamGridDB artwork: the API key, picking art automatically after a sync,
//! and the per-game artwork picker.

use hub_providers::sgdb::{self, ArtKind, ArtOption, GameRef};
use hub_providers::secrets;
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, now, AppState, CmdResult};

/// Ask SteamGridDB about each game at most monthly, and this many per sync,
/// so a big library is covered over a few starts without hammering the API.
const RECHECK_SECS: i64 = 30 * 86_400;
const PER_SYNC: i64 = 60;

#[tauri::command]
pub async fn sgdb_set_key(app: AppHandle, key: String) -> CmdResult<()> {
    let key = key.trim().to_string();
    if key.len() < 16 || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("That doesn't look like a SteamGridDB API key. Copy it from your SteamGridDB preferences.".into());
    }
    let state = app.state::<AppState>();
    state.sgdb.check_key(&key).await.map_err(|e| format!("{e:#}"))?;
    secrets::save("sgdb-key", Some(&key));
    state.sgdb.set_key(Some(key));
    let _ = app.emit("accounts-changed", ());
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { apply_automatically(&handle).await });
    Ok(())
}

#[tauri::command]
pub fn sgdb_clear_key(app: AppHandle) {
    secrets::save("sgdb-key", None);
    app.state::<AppState>().sgdb.set_key(None);
    let _ = app.emit("accounts-changed", ());
}

#[tauri::command]
pub fn open_sgdb_key_page() -> CmdResult<()> {
    hub_launch::run(&hub_launch::LaunchAction::Uri { uri: sgdb::KEY_PAGE.into() }).map_err(err)
}

/// Where to find a game on SteamGridDB: its Steam app when known, else by title.
async fn game_ref(app: &AppHandle, title: &str, steam_app: Option<i64>) -> anyhow::Result<Option<GameRef>> {
    if let Some(id) = steam_app.and_then(|a| u64::try_from(a).ok()) {
        return Ok(Some(GameRef::Steam(id)));
    }
    let found = app.state::<AppState>().sgdb.find_game(title).await?;
    Ok(found.map(GameRef::Sgdb))
}

fn steam_app_of(app: &AppHandle, id: i64) -> CmdResult<(String, Option<i64>)> {
    let state = app.state::<AppState>();
    let db = lock(&state.db);
    let g = db.game(id).map_err(err)?.ok_or("That game is no longer in your library")?;
    let steam = g
        .entries
        .iter()
        .find(|e| e.store == hub_core::StoreId::Steam)
        .and_then(|e| e.store_game_id.parse::<i64>().ok());
    let matched = db
        .details(id)
        .ok()
        .flatten()
        .and_then(|(json, _)| json)
        .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
        .and_then(|v| v.get("steam_app_id")?.as_i64());
    Ok((g.title, steam.or(matched)))
}

/// SteamGridDB pictures to choose from for one game.
#[tauri::command]
pub async fn artwork_options(app: AppHandle, id: i64, kind: ArtKind) -> CmdResult<Vec<ArtOption>> {
    let (title, steam_app) = steam_app_of(&app, id)?;
    let sgdb = &app.state::<AppState>().sgdb;
    let r = match steam_app.and_then(|a| u64::try_from(a).ok()) {
        Some(app_id) => Some(GameRef::Steam(app_id)),
        // The user sees the pictures, so the closest title is fine here.
        None => sgdb.closest_game(&title).await.map_err(|e| format!("{e:#}"))?.map(GameRef::Sgdb),
    };
    let Some(r) = r else {
        return Ok(Vec::new());
    };
    app.state::<AppState>().sgdb.images(r, kind).await.map_err(|e| format!("{e:#}"))
}

/// The user picked a picture: it's theirs and is never replaced automatically.
#[tauri::command]
pub fn set_artwork(app: AppHandle, id: i64, kind: ArtKind, url: String) -> CmdResult<()> {
    if !url.starts_with("https://") {
        return Err("Artwork must be an https address".into());
    }
    let kind = match kind {
        ArtKind::Cover => "cover",
        ArtKind::Hero => "hero",
        ArtKind::Logo => "logo",
    };
    lock(&app.state::<AppState>().db).set_art(id, kind, &url, "user").map_err(err)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// Back to the store's own artwork (and automatic SteamGridDB picks).
#[tauri::command]
pub async fn reset_artwork(app: AppHandle, id: i64) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).reset_art(id).map_err(err)?;
    crate::refresh_art(&app).await;
    Ok(())
}

/// After a sync, give games still on store artwork the community's favourite
/// SteamGridDB cover and background. Skipped without a key.
pub async fn apply_automatically(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.sgdb.has_key() {
        return;
    }
    let targets = lock(&state.db).sgdb_targets(now() - RECHECK_SECS, PER_SYNC).unwrap_or_default();
    let mut changed = 0;
    for (id, title, steam_app) in targets {
        let r = match game_ref(app, &title, steam_app).await {
            Ok(r) => r,
            // Bad key or rate limited: stop for now, try again next sync.
            Err(_) => break,
        };
        if let Some(r) = r {
            for kind in [ArtKind::Cover, ArtKind::Hero] {
                if let Ok(images) = state.sgdb.images(r, kind).await {
                    if let Some(best) = images.first() {
                        let k = if kind == ArtKind::Cover { "cover" } else { "hero" };
                        if lock(&state.db).set_art(id, k, &best.url, "sgdb").unwrap_or(false) {
                            changed += 1;
                        }
                    }
                }
            }
        }
        let _ = lock(&state.db).mark_sgdb_checked(id, now());
        if changed > 0 && changed % 20 == 0 {
            let _ = app.emit("library-changed", ());
        }
        // Be gentle with a free community API.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    if changed > 0 {
        let _ = app.emit("library-changed", ());
    }
}
