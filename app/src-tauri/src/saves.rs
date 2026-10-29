//! Dates the last session of every game from its save files, and reads the
//! game's own play timer where its saves keep one, after each sync. Save
//! locations come from PCGamingWiki, looked up once a month per game; the
//! folders themselves are checked on every sync (local and quick).

use std::path::Path;

use hub_providers::saves::{newest_file, played_secs, resolve, Places};
use tauri::{AppHandle, Emitter, Manager};

use crate::{lock, now, AppState};

const RECHECK_SECS: i64 = 30 * 86_400;
const PER_SYNC: i64 = 80;

pub async fn refresh(app: &AppHandle) {
    let state = app.state::<AppState>();

    // 1. Look up save locations for games we haven't asked about lately.
    let targets = lock(&state.db).save_lookup_targets(now() - RECHECK_SECS, PER_SYNC).unwrap_or_default();
    for (id, title, steam_app) in targets {
        match state.saves.locations(&title, steam_app.and_then(|a| u64::try_from(a).ok())).await {
            // An empty list is remembered too, so unknown games aren't re-asked every sync.
            Ok(paths) => {
                let _ = lock(&state.db).set_save_paths(id, &paths, now());
            }
            // Offline or the wiki is down: try again next sync.
            Err(_) => break,
        }
        // Up to three wiki requests a game; faster than this got throttled.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // 2. Date each game by its newest save file, and read its play timer.
    let Some(places) = Places::current() else { return };
    let locations = lock(&state.db).save_locations().unwrap_or_default();
    let found = tauri::async_runtime::spawn_blocking(move || {
        locations
            .into_iter()
            .filter_map(|(id, paths, install)| {
                let install = install.as_deref().map(Path::new);
                let resolved: Vec<String> = paths.iter().filter_map(|p| resolve(p, &places, install)).collect();
                let newest = resolved.iter().filter_map(|p| newest_file(p)).max()?;
                let played = resolved.iter().filter_map(|p| played_secs(p)).max();
                Some((id, newest, played))
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();

    let db = lock(&state.db);
    let changed = found.into_iter().fold(false, |acc, (id, at, played)| {
        let seen = db.set_save_seen(id, at).unwrap_or(false);
        let timed = played.is_some_and(|s| db.set_save_played(id, s).unwrap_or(false));
        seen || timed || acc
    });
    drop(db);
    if changed {
        let _ = app.emit("library-changed", ());
    }
}
