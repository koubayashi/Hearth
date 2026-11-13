//! Ways to play: programs the user adds to a game besides its store's own
//! launch (a server replacement's start script, a mod loader, another exe).
//! Hearth only ever runs a file the user picked here.

use std::path::{Component, Path, PathBuf};

use hub_core::{LaunchOption, StoreEntry, StoreId};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, AppState, CmdResult};

/// How to start a game when the user picks one explicitly.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Via {
    /// The store's own launch (or install).
    Store,
    /// One of the game's ways to play.
    Option { id: i64 },
    /// One version of a combined game (its store entry): played if
    /// installed, else installed from its store.
    Entry { id: i64 },
}

fn changed(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
}

/// The store Play goes through with two versions installed: the one played
/// last; on a tie (neither played), the first. The interface's Play button
/// picks the same way (`launchEntry` in ways.ts).
pub fn installed_to_play(entries: &[StoreEntry]) -> Option<&StoreEntry> {
    entries
        .iter()
        .filter(|e| e.installed && !e.offline)
        .min_by_key(|e| std::cmp::Reverse(e.last_played.unwrap_or(0)))
}

/// Starts a way to play.
pub fn run(option: &LaunchOption) -> CmdResult<()> {
    hub_launch::run_program(
        Path::new(&option.path),
        &option.args,
        option.working_dir.as_deref().map(Path::new),
    )
    .map_err(err)
}

#[derive(Deserialize)]
pub struct WayInput {
    id: Option<i64>,
    game_id: i64,
    name: String,
    path: String,
    #[serde(default)]
    args: String,
    working_dir: Option<String>,
}

#[tauri::command]
pub fn save_launch_option(app: AppHandle, option: WayInput) -> CmdResult<i64> {
    if option.name.trim().is_empty() {
        return Err("Give it a name, like \u{201c}Offline server\u{201d}.".into());
    }
    if !Path::new(option.path.trim()).is_file() {
        return Err("That program can't be found. Choose it again with Browse.".into());
    }
    // Windows opens a shortcut itself, with the arguments saved in it.
    if !option.args.trim().is_empty() && !hub_launch::takes_arguments(Path::new(option.path.trim())) {
        return Err("Only a program (.exe) or script (.bat, .cmd) takes arguments. For a shortcut, add them in the shortcut's Properties, or leave Arguments empty.".into());
    }
    if let Some(dir) = option.working_dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        if !Path::new(dir).is_dir() {
            return Err("That start folder can't be found.".into());
        }
    }
    let id = lock(&app.state::<AppState>().db)
        .save_launch_option(
            option.id,
            option.game_id,
            &option.name,
            &option.path,
            &option.args,
            option.working_dir.as_deref(),
        )
        .map_err(err)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn delete_launch_option(app: AppHandle, id: i64) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).delete_launch_option(id).map_err(err)?;
    changed(&app);
    Ok(())
}

/// The program a folder game's Play starts when the user picked one: only
/// ever a file inside the game's own install folder.
pub fn folder_exe(install_dir: &str, launch_exe: &str) -> Option<PathBuf> {
    let rel = Path::new(launch_exe);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let path = Path::new(install_dir).join(rel);
    path.is_file().then_some(path)
}

/// `path` relative to `dir` when it lies inside it, ignoring case as Windows does.
fn inside(dir: &Path, path: &Path) -> Option<String> {
    let same = |a: Component, b: Component| a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase();
    let mut rest = path.components();
    for d in dir.components() {
        if !rest.next().is_some_and(|c| same(c, d)) {
            return None;
        }
    }
    let rel = rest.as_path();
    (rel.components().count() > 0).then(|| rel.to_string_lossy().into_owned())
}

/// Picks the program a folder game's Play starts, from inside its install
/// folder; None goes back to the one the folder scan chose.
#[tauri::command]
pub fn set_folder_exe(app: AppHandle, game_id: i64, path: Option<String>) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let db = lock(&state.db);
    let game = db.game(game_id).map_err(err)?.ok_or("That game is no longer in your library")?;
    let entry = game
        .entries
        .iter()
        .find(|e| e.store == StoreId::Manual && e.installed)
        .ok_or("Only games found in a game folder can have their program changed")?;
    let dir = entry.install_dir.as_deref().ok_or("This game's folder can't be found")?;
    let rel = match path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        None => None,
        Some(p) => {
            let rel = inside(Path::new(dir), Path::new(p)).ok_or("Choose a program inside the game's own folder")?;
            if folder_exe(dir, &rel).is_none() {
                return Err("That program can't be found. Choose it again.".into());
            }
            // The scan's own choice needs no override.
            let scans = inside(Path::new(dir), Path::new(&entry.store_game_id));
            (scans.map(|s| s.to_lowercase()) != Some(rel.to_lowercase())).then_some(rel)
        }
    };
    db.set_launch_exe(entry.id, rel.as_deref()).map_err(err)?;
    drop(db);
    changed(&app);
    Ok(())
}

/// What the Play button does: a way to play, or None for the store.
#[tauri::command]
pub fn set_default_launch(app: AppHandle, game_id: i64, option: Option<i64>) -> CmdResult<()> {
    lock(&app.state::<AppState>().db).set_default_launch(game_id, option).map_err(err)?;
    changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, store: StoreId, installed: bool, last_played: Option<i64>) -> StoreEntry {
        StoreEntry {
            id,
            game_id: 1,
            store,
            store_game_id: format!("game-{id}"),
            installed,
            install_dir: None,
            hidden: false,
            store_playtime_secs: None,
            store_baseline_secs: None,
            launch_exe: None,
            offline: false,
            title: None,
            last_played,
        }
    }

    #[test]
    fn play_picks_the_installed_version_played_last_else_the_first() {
        let pick = |entries: &[StoreEntry]| installed_to_play(entries).map(|e| e.id);
        let never = [entry(1, StoreId::Steam, true, None), entry(2, StoreId::Gog, true, None)];
        assert_eq!(pick(&never), Some(1));
        let tied = [entry(1, StoreId::Steam, true, Some(500)), entry(2, StoreId::Gog, true, Some(500))];
        assert_eq!(pick(&tied), Some(1));
        let later = [entry(1, StoreId::Steam, true, Some(100)), entry(2, StoreId::Gog, true, Some(900)), entry(3, StoreId::Epic, false, Some(2000))];
        assert_eq!(pick(&later), Some(2));
        let mut offline = entry(2, StoreId::Gog, true, Some(900));
        offline.offline = true;
        assert_eq!(pick(&[entry(1, StoreId::Steam, true, None), offline]), Some(1));
        assert_eq!(pick(&[entry(1, StoreId::Steam, false, None)]), None);
    }

    #[test]
    fn a_picked_program_stays_inside_the_game_folder() {
        let dir = Path::new(r"D:\Games\COD\Call of Duty - Modern Warfare Remastered");
        let sp = Path::new(r"d:\games\cod\Call of Duty - Modern Warfare Remastered\h1_sp64_ship.exe");
        assert_eq!(inside(dir, sp).as_deref(), Some("h1_sp64_ship.exe"));
        assert_eq!(inside(dir, &dir.join(r"bin\x.exe")).as_deref(), Some(r"bin\x.exe"));
        // A sibling folder with a shared prefix, the folder itself, elsewhere.
        assert_eq!(inside(dir, Path::new(r"D:\Games\COD\Call of Duty - Modern Warfare Remastered 2\a.exe")), None);
        assert_eq!(inside(dir, dir), None);
        assert_eq!(inside(dir, Path::new(r"C:\Windows\System32\cmd.exe")), None);
        // A stored path can't climb out of the folder.
        let tmp = std::env::temp_dir();
        assert_eq!(folder_exe(&tmp.join("game").to_string_lossy(), r"..\..\Windows\System32\cmd.exe"), None);
        assert_eq!(folder_exe(&tmp.to_string_lossy(), r"C:\Windows\System32\cmd.exe"), None);
    }
}
