//! Series, developers and publishers for the "browse by" shelves: looked up
//! on Wikidata after each sync, a batch at a time, and fixed by the user.

use hub_core::SmartIndex;
use hub_providers::wikidata::{pick, Item};
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, now, AppState, CmdResult};

/// Ask about each game, and each company or series' parents, at most monthly.
const RECHECK_SECS: i64 = 30 * 86_400;
/// Games per sync, so a big library fills in over a few starts.
const PER_SYNC: i64 = 200;
/// Steam ids per query; titles are slower to match, so fewer of them.
const STEAM_BATCH: usize = 50;
const TITLE_BATCH: usize = 20;
/// Titles searched per round (each search waits a moment: Wikidata rate-limits it).
const SEARCH_BATCH: usize = 10;
/// Rounds of parent lookups per sync: studio → publisher → its owner.
const PARENT_ROUNDS: usize = 4;

pub async fn refresh(app: &AppHandle) {
    let state = app.state::<AppState>();
    let targets = lock(&state.db).wikidata_targets(now() - RECHECK_SECS, PER_SYNC).unwrap_or_default();
    let parents_due = !lock(&state.db).entities_without_parents(now() - RECHECK_SECS, 1).unwrap_or_default().is_empty();
    if targets.is_empty() && !parents_due {
        return;
    }
    // The shelves say "looking up" meanwhile, rather than showing nothing.
    let _ = app.emit("smart-status", true);
    lookup(app, targets).await;
    let _ = app.emit("smart-status", false);
}

async fn lookup(app: &AppHandle, targets: Vec<(i64, String, Option<i64>)>) {
    let state = app.state::<AppState>();

    // Three ways to find a game, surest first: its Steam id, its exact
    // title, then a search. Each passes on what it couldn't find.
    let (steam, mut titled): (Vec<_>, Vec<_>) = targets.into_iter().partition(|(_, _, app)| app.is_some());
    for batch in steam.chunks(STEAM_BATCH) {
        let apps: Vec<u64> = batch.iter().filter_map(|(_, _, a)| a.and_then(|a| u64::try_from(a).ok())).collect();
        // Offline, or Wikidata is busy: try again next sync.
        let Ok(found) = state.wikidata.by_steam(&apps).await else { return };
        for (id, title, app_id) in batch {
            match app_id.and_then(|a| u64::try_from(a).ok()).and_then(|a| found.get(&a)) {
                Some(items) if !items.is_empty() => save(app, *id, title, Some(items)),
                _ => titled.push((*id, title.clone(), None)),
            }
        }
        // Each batch shows at once, so a big library fills in as it goes.
        let _ = app.emit("smart-changed", ());
    }
    let mut unnamed = Vec::new();
    for batch in titled.chunks(TITLE_BATCH) {
        let titles: Vec<String> = batch.iter().map(|(_, t, _)| t.clone()).collect();
        let Ok(found) = state.wikidata.by_title(&titles).await else { return };
        for (id, title, _) in batch {
            match found.get(title) {
                Some(items) if !items.is_empty() => save(app, *id, title, Some(items)),
                _ => unnamed.push((*id, title.clone())),
            }
        }
        let _ = app.emit("smart-changed", ());
    }
    // Searching is the slowest step and the one Wikidata rate-limits: when it
    // can't go on, the rest wait for the next sync, and the parents below
    // are still looked up (they used to be skipped, leaving studios like
    // Rockstar Leeds outside Rockstar Games).
    for batch in unnamed.chunks(SEARCH_BATCH) {
        let titles: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();
        let Ok((found, unsearched)) = state.wikidata.by_search(&titles).await else { break };
        // Not found even by search: saved as checked, asked again next month.
        // Not searched at all: left unchecked, so next sync tries again.
        for (id, title) in batch.iter().filter(|(_, t)| !unsearched.contains(t)) {
            save(app, *id, title, found.get(title).map(Vec::as_slice));
        }
        let _ = app.emit("smart-changed", ());
        if !unsearched.is_empty() {
            break;
        }
    }

    for _ in 0..PARENT_ROUNDS {
        let ids = lock(&state.db).entities_without_parents(now() - RECHECK_SECS, 150).unwrap_or_default();
        if ids.is_empty() {
            break;
        }
        let Ok(parents) = state.wikidata.parents(&ids).await else { break };
        let rows: Vec<(&str, &str, &str)> = parents.iter().map(|p| (p.child.as_str(), p.id.as_str(), p.label.as_str())).collect();
        let _ = lock(&state.db).set_parents(&ids, &rows, now());
        let _ = app.emit("smart-changed", ());
    }
}

/// Saves the best match for one game, or that there was none.
fn save(app: &AppHandle, id: i64, title: &str, items: Option<&[Item]>) {
    let item = items.and_then(|i| pick(i, title));
    let links: Vec<(&str, &str, &str)> = item
        .map(|i| i.links.iter().map(|l| (l.role.as_str(), l.id.as_str(), l.label.as_str())).collect())
        .unwrap_or_default();
    let _ = lock(&app.state::<AppState>().db).set_wikidata(id, item.map(|i| i.id.as_str()), &links, now());
}

#[tauri::command]
pub fn smart_index(app: AppHandle) -> CmdResult<SmartIndex> {
    lock(&app.state::<AppState>().db).smart_index().map_err(err)
}

/// Puts a game in a series (or credits a company), or takes it out.
#[tauri::command]
pub fn set_game_link(app: AppHandle, game_id: i64, role: String, entity: String, on: bool) -> CmdResult<()> {
    if !matches!(role.as_str(), "series" | "developer" | "publisher") {
        return Err("Unknown kind of link".into());
    }
    lock(&app.state::<AppState>().db).set_game_link(game_id, &role, &entity, on).map_err(err)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// Puts a company or series inside another, or takes it out.
#[tauri::command]
pub fn set_entity_parent(app: AppHandle, child: String, parent: String, on: bool) -> CmdResult<()> {
    if child == parent {
        return Err("Something can't be part of itself".into());
    }
    lock(&app.state::<AppState>().db).set_entity_parent(&child, &parent, on).map_err(err)?;
    let _ = app.emit("smart-changed", ());
    Ok(())
}

/// A new series of the user's own, with this game in it.
#[tauri::command]
pub fn create_series(app: AppHandle, game_id: i64, name: String) -> CmdResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the series a name".into());
    }
    let state = app.state::<AppState>();
    let db = lock(&state.db);
    let id = db.create_entity(name).map_err(err)?;
    db.set_game_link(game_id, "series", &id, true).map_err(err)?;
    drop(db);
    let _ = app.emit("library-changed", ());
    Ok(id)
}
