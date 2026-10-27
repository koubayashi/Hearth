//! Extra information for the selected game: store-page details, install
//! folder size, and opening the folder or store page.

use std::path::Path;

use hub_core::{Game, StoreId};
use hub_launch::LaunchAction;
use hub_providers::metadata::GameDetails;
use tauri::{AppHandle, Manager};

use crate::{err, lock, now, AppState, CmdResult};

/// Found details are refreshed monthly; a miss is retried after a week.
const HIT_TTL: i64 = 30 * 86_400;
const MISS_TTL: i64 = 7 * 86_400;

fn game(app: &AppHandle, id: i64) -> CmdResult<Game> {
    lock(&app.state::<AppState>().db)
        .game(id)
        .map_err(err)?
        .ok_or_else(|| "That game is no longer in your library".to_string())
}

#[tauri::command]
pub async fn game_details(app: AppHandle, id: i64) -> CmdResult<Option<GameDetails>> {
    let state = app.state::<AppState>();
    if let Some((json, checked)) = lock(&state.db).details(id).map_err(err)? {
        // Details saved before the description was kept are fetched once more.
        let current = json.as_deref().is_none_or(|j| j.contains("\"summary\""));
        let fresh = current && now() - checked < if json.is_some() { HIT_TTL } else { MISS_TTL };
        if fresh {
            return Ok(json.and_then(|j| serde_json::from_str(&j).ok()));
        }
    }

    let g = game(&app, id)?;
    let steam_id = g
        .entries
        .iter()
        .find(|e| e.store == StoreId::Steam)
        .and_then(|e| e.store_game_id.parse::<u64>().ok());
    let app_id = match steam_id {
        Some(id) => Some(id),
        None => state.metadata.find_on_steam(&g.title).await.map_err(err)?,
    };
    let details = match app_id {
        Some(app_id) => state.metadata.steam_app(app_id).await.map_err(err)?,
        None => None,
    };
    let json = details.as_ref().and_then(|d| serde_json::to_string(d).ok());
    lock(&state.db).set_details(id, json.as_deref(), now()).map_err(err)?;
    Ok(details)
}

#[derive(serde::Serialize)]
pub struct StoreHit {
    pub steam_app_id: u64,
    pub name: String,
}

/// Steam store search, for picking a game's page by hand.
#[tauri::command]
pub async fn search_store(app: AppHandle, term: String) -> CmdResult<Vec<StoreHit>> {
    let hits = app.state::<AppState>().metadata.search_store(term.trim()).await.map_err(err)?;
    Ok(hits.into_iter().map(|(steam_app_id, name)| StoreHit { steam_app_id, name }).collect())
}

/// The user chose this game's store page: take its details and artwork.
#[tauri::command]
pub async fn set_store_match(app: AppHandle, id: i64, steam_app_id: u64) -> CmdResult<Option<GameDetails>> {
    let state = app.state::<AppState>();
    let details = state.metadata.steam_app(steam_app_id).await.map_err(err)?;
    let Some(d) = details else {
        return Err("That store page has no game details".into());
    };
    let json = serde_json::to_string(&d).map_err(err)?;
    let art = hub_providers::steam::Steam::art_urls(&steam_app_id.to_string());
    {
        let db = lock(&state.db);
        db.set_details(id, Some(&json), now()).map_err(err)?;
        if let (Some(c), Some(h), Some(l)) = (art.cover, art.hero, art.logo) {
            db.set_game_art(id, &c, &h, &l).map_err(err)?;
        }
    }
    let _ = tauri::Emitter::emit(&app, "library-changed", ());
    Ok(Some(d))
}

#[derive(serde::Serialize)]
pub struct InstallInfo {
    pub path: String,
    pub bytes: Option<i64>,
}

/// Where the game is installed and how big the folder is. The size is measured
/// once (off the UI thread) and cached; `refresh` measures it again.
#[tauri::command]
pub async fn install_info(app: AppHandle, id: i64, refresh: bool) -> CmdResult<Option<InstallInfo>> {
    let g = game(&app, id)?;
    let Some(path) = g.entries.iter().find(|e| e.installed).and_then(|e| e.install_dir.clone()) else {
        return Ok(None);
    };
    let state = app.state::<AppState>();
    let cached = lock(&state.db).install_size(id).map_err(err)?;
    let bytes = match cached {
        Some(b) if !refresh => Some(b),
        _ => {
            let p = path.clone();
            let measured = tauri::async_runtime::spawn_blocking(move || folder_size(Path::new(&p)))
                .await
                .map_err(err)?;
            if let Some(b) = measured {
                let _ = lock(&state.db).set_install_size(id, b);
            }
            measured
        }
    };
    Ok(Some(InstallInfo { path, bytes }))
}

fn folder_size(root: &Path) -> Option<i64> {
    if !root.is_dir() {
        return None;
    }
    let mut total: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            // Don't follow links out of the game folder.
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                total += meta.len();
            }
        }
    }
    i64::try_from(total).ok()
}

#[tauri::command]
pub fn open_install_folder(app: AppHandle, id: i64) -> CmdResult<()> {
    let g = game(&app, id)?;
    let dir = g
        .entries
        .iter()
        .find_map(|e| e.install_dir.clone())
        .ok_or("This game isn't installed")?;
    if !Path::new(&dir).is_dir() {
        return Err(format!("The folder {dir} doesn't exist anymore"));
    }
    hub_launch::run(&LaunchAction::Process { exe: "explorer.exe".into(), args: vec![dir], working_dir: None })
        .map_err(err)
}

/// The game's page in its store: Steam's client page when Hearth knows the
/// Steam app (directly or through matched details), otherwise the store's app.
#[tauri::command]
pub async fn open_store_page(app: AppHandle, id: i64) -> CmdResult<()> {
    let g = game(&app, id)?;
    let uri = if let Some(e) = g.entries.iter().find(|e| e.store == StoreId::Steam) {
        format!("steam://store/{}", e.store_game_id)
    } else if g.entries.iter().any(|e| e.store == StoreId::Epic) {
        "com.epicgames.launcher://store".to_string()
    } else {
        match game_details(app.clone(), id).await? {
            Some(d) => format!("https://store.steampowered.com/app/{}", d.steam_app_id),
            None => return Err("No store page is known for this game".into()),
        }
    };
    hub_launch::run(&LaunchAction::Uri { uri }).map_err(err)
}

