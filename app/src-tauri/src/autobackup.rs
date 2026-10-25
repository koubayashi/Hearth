//! Automatic backups: after the library changes, once things have been quiet
//! for a couple of minutes, a backup goes to a folder the user picked (often a
//! OneDrive or Dropbox folder, so it also lands in the cloud). At most hourly,
//! the newest few kept. Runs in the background, window open or not.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{err, lock, now, AppState, CmdResult};

/// Wait this long after the last change, so a burst of edits is one backup.
const QUIET_SECS: i64 = 120;
/// Never more often than this (play sessions write every minute).
const MIN_GAP_SECS: i64 = 3600;
/// Automatic backups kept in the folder; older ones are removed.
const KEEP: usize = 10;
const PREFIX: &str = "Hearth auto-backup ";
const EXT: &str = ".hearthbackup";

#[derive(Default, Clone, Serialize, Deserialize)]
struct Config {
    folder: Option<String>,
    last_at: Option<i64>,
}

/// What Settings shows: where backups go, when the last one was made, and
/// why the last attempt failed, if it did.
#[derive(Default, Clone, Serialize)]
pub struct Status {
    folder: Option<String>,
    last_at: Option<i64>,
    error: Option<String>,
}

#[derive(Default)]
pub struct AutoBackup {
    status: Mutex<Status>,
    /// Set when something changed since the last backup.
    pending: Mutex<bool>,
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("auto-backup.json"))
}

fn load(app: &AppHandle) -> Config {
    config_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        // Tolerate a byte-order mark, in case the file was edited by hand.
        .and_then(|t| serde_json::from_str(t.trim_start_matches('﻿')).ok())
        .unwrap_or_default()
}

fn store(app: &AppHandle, c: &Config) {
    if let (Some(p), Ok(text)) = (config_path(app), serde_json::to_string_pretty(c)) {
        let _ = std::fs::write(p, text);
    }
}

/// The appearance settings the window last reported, for backups made while
/// it is closed (Hearth in the tray).
fn prefs_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("prefs.json"))
}

#[tauri::command]
pub fn save_prefs_snapshot(app: AppHandle, prefs: String) {
    if let Some(p) = prefs_path(&app) {
        let _ = std::fs::write(p, prefs);
    }
}

#[tauri::command]
pub fn auto_backup_status(app: AppHandle) -> Status {
    let c = load(&app);
    let mut s = lock(&app.state::<AutoBackup>().status).clone();
    s.folder = c.folder;
    s.last_at = c.last_at;
    s
}

/// Turns automatic backups on (with a folder) or off (None). Turning them on
/// makes the first one straight away.
#[tauri::command]
pub async fn auto_backup_set(app: AppHandle, folder: Option<String>) -> CmdResult<Status> {
    if let Some(f) = &folder {
        if !Path::new(f).is_dir() {
            return Err("That folder can't be found.".into());
        }
    }
    let mut c = load(&app);
    c.folder = folder.clone();
    store(&app, &c);
    lock(&app.state::<AutoBackup>().status).error = None;
    if folder.is_some() {
        run(&app)?;
    }
    Ok(auto_backup_status(app))
}

/// Local date and time for file names, "2026-10-25 15-30-05", so they sort.
#[cfg(windows)]
fn stamp() -> String {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut t = SystemTime::default();
    // SAFETY: fills the struct it is given and nothing else.
    unsafe { GetLocalTime(&mut t) };
    format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second)
}

#[cfg(not(windows))]
fn stamp() -> String {
    now().to_string()
}

/// Makes one automatic backup now and removes the oldest beyond KEEP.
fn run(app: &AppHandle) -> CmdResult<()> {
    let mut c = load(app);
    let Some(folder) = c.folder.clone() else { return Ok(()) };
    let auto = app.state::<AutoBackup>();
    let result = (|| -> CmdResult<()> {
        let dir = PathBuf::from(&folder);
        if !dir.is_dir() {
            return Err(format!("{folder} isn't reachable right now. Hearth tries again later."));
        }
        let file = dir.join(format!("{PREFIX}{}{EXT}", stamp()));
        let prefs = prefs_path(app).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_else(|| "{}".into());
        let folders = serde_json::to_string(&crate::folders::current(app)).map_err(err)?;
        let created = now().to_string();
        lock(&app.state::<AppState>().db)
            .backup_to(
                &file,
                &[
                    ("format", "1"),
                    ("created_at", &created),
                    ("app_version", env!("CARGO_PKG_VERSION")),
                    ("game_folders", &folders),
                    ("prefs", &prefs),
                ],
            )
            .map_err(err)?;
        prune(&dir);
        Ok(())
    })();
    match &result {
        Ok(()) => {
            c.last_at = Some(now());
            store(app, &c);
            lock(&auto.status).error = None;
            *lock(&auto.pending) = false;
            crate::tracking::log(app, "auto-backup: saved");
        }
        Err(e) => {
            lock(&auto.status).error = Some(e.clone());
            crate::tracking::log(app, &format!("auto-backup: {e}"));
        }
    }
    let _ = app.emit("auto-backup", ());
    result
}

/// Keeps the newest KEEP automatic backups (names sort by date). Backups the
/// user saved by hand are never touched.
fn prune(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut ours: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(EXT))
        })
        .collect();
    ours.sort();
    let extra = ours.len().saturating_sub(KEEP);
    for old in ours.into_iter().take(extra) {
        let _ = std::fs::remove_file(old);
    }
}

/// Watches the library for changes and backs up once it has been quiet.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let changes = || lock(&app.state::<AppState>().db).changes();
        let mut seen = changes();
        let mut changed_at = now();
        loop {
            std::thread::sleep(Duration::from_secs(30));
            let auto = app.state::<AutoBackup>();
            let current = changes();
            if current != seen {
                seen = current;
                changed_at = now();
                *lock(&auto.pending) = true;
            }
            let c = load(&app);
            if c.folder.is_none() || !*lock(&auto.pending) {
                continue;
            }
            let quiet = now() - changed_at >= QUIET_SECS;
            let due = c.last_at.is_none_or(|t| now() - t >= MIN_GAP_SECS);
            // A failed attempt (folder unplugged) retries on the next pass.
            if quiet && due {
                let _ = run(&app);
                // The backup itself doesn't count as a change.
                seen = changes();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_the_newest_and_never_hand_made_backups() {
        let dir = std::env::temp_dir().join(format!("hearth-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..13 {
            std::fs::write(dir.join(format!("{PREFIX}2026-10-{:02} 10-00-00{EXT}", i + 1)), "x").unwrap();
        }
        std::fs::write(dir.join("Hearth backup 2026-10-01.hearthbackup"), "mine").unwrap();
        prune(&dir);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left.len(), KEEP + 1);
        assert!(left.contains(&"Hearth backup 2026-10-01.hearthbackup".to_string()));
        assert!(!left.iter().any(|n| n.contains("2026-10-03 ")), "the oldest three automatic ones go");
        assert!(left.iter().any(|n| n.contains("2026-10-13 ")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
