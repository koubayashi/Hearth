//! Runs the playtime tracker on its own thread for as long as Hearth is open,
//! including while it sits in the tray with no window.
//!
//! Running sessions are saved every minute, so if Hearth is killed, crashes or
//! the PC shuts down mid-game, the time up to the last save is recorded the
//! next time Hearth starts. Every start and end is written to `tracker.log`.

use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;

use hub_core::SessionSource;
use hub_tracker::{Event, ProcessScanner, Tracker, MIN_SESSION_SECS};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{lock, now, AppState};

const POLL: Duration = Duration::from_secs(5);
/// Polls between checkpoints of running sessions (12 × 5s = a minute).
const CHECKPOINT_EVERY: u32 = 12;

#[derive(Default)]
pub struct Tracking {
    pub tracker: Mutex<Tracker>,
}

#[derive(Clone, Serialize)]
pub struct NowPlaying {
    pub game_id: i64,
    pub title: String,
    pub started_at: i64,
}

/// Appends a line to `tracker.log` in the app data folder.
pub fn log(app: &AppHandle, msg: &str) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    let path = dir.join("tracker.log");
    // Keep the log small: start over once it passes 1 MB.
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {msg}", now());
    }
}

fn title(app: &AppHandle, id: i64) -> String {
    lock(&app.state::<AppState>().db)
        .game(id)
        .ok()
        .flatten()
        .map(|g| g.title)
        .unwrap_or_else(|| format!("game {id}"))
}

/// Point the tracker at the current install folders (after every sync).
pub fn refresh_dirs(app: &AppHandle) {
    let state = app.state::<AppState>();
    let dirs = lock(&state.db).install_dirs().unwrap_or_default();
    log(app, &format!("watching {} install folders", dirs.len()));
    lock(&app.state::<Tracking>().tracker).set_install_dirs(dirs);
}

/// Sessions that were still running when Hearth last stopped become real
/// sessions, ending at their last checkpoint.
pub fn recover(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut db = lock(&state.db);
    let Ok(leftover) = db.take_active() else { return };
    for (game, started, seen) in leftover {
        if seen - started >= MIN_SESSION_SECS
            && db.record_session(game, started, seen, SessionSource::Tracker).is_ok()
        {
            drop(db);
            log(app, &format!("recovered {}s of {} from before Hearth stopped", seen - started, title(app, game)));
            db = lock(&state.db);
        }
    }
}

pub fn now_playing(app: &AppHandle) -> Vec<NowPlaying> {
    let running = lock(&app.state::<Tracking>().tracker).running();
    let state = app.state::<AppState>();
    let db = lock(&state.db);
    running
        .into_iter()
        .filter_map(|(id, started_at)| {
            let title = db.game(id).ok()??.title;
            Some(NowPlaying { game_id: id, title, started_at })
        })
        .collect()
}

fn checkpoint(app: &AppHandle) {
    let active = lock(&app.state::<Tracking>().tracker).active();
    let _ = lock(&app.state::<AppState>().db).checkpoint_active(&active);
}

pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("playtime-tracker".into())
        .spawn(move || {
            let mut scanner = ProcessScanner::new();
            let mut polls: u32 = 0;
            loop {
                let exes = scanner.exes();
                let events = lock(&app.state::<Tracking>().tracker).observe(exes.iter().map(|p| p.as_path()), now());
                let had_events = !events.is_empty();
                if had_events {
                    handle(&app, events);
                }
                polls = polls.wrapping_add(1);
                if had_events || polls % CHECKPOINT_EVERY == 0 {
                    checkpoint(&app);
                }
                std::thread::sleep(POLL);
            }
        })
        .expect("start tracker thread");
}

/// Close open sessions so quitting mid-game still counts the time played.
pub fn flush(app: &AppHandle) {
    let events = lock(&app.state::<Tracking>().tracker).flush();
    handle(app, events);
    checkpoint(app);
}

fn handle(app: &AppHandle, events: Vec<Event>) {
    let mut changed = false;
    for e in events {
        match e {
            Event::Started { game_id, .. } => log(app, &format!("started {}", title(app, game_id))),
            Event::Ended(s) => {
                let ok = lock(&app.state::<AppState>().db)
                    .record_session(s.game_id, s.started_at, s.ended_at, SessionSource::Tracker)
                    .is_ok();
                log(
                    app,
                    &format!("ended {} after {}s{}", title(app, s.game_id), s.ended_at - s.started_at, if ok { "" } else { " (not saved)" }),
                );
                changed |= ok;
            }
        }
    }
    let playing = now_playing(app);
    crate::tray::set_playing(app, &playing);
    let _ = app.emit("now-playing", &playing);
    if changed {
        let _ = app.emit("library-changed", ());
    }
}
