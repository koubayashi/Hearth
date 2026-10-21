//! Runs the playtime tracker on its own thread for as long as Hearth is open,
//! including while it sits in the tray with no window.

use std::sync::Mutex;
use std::time::Duration;

use hub_core::SessionSource;
use hub_tracker::{Event, ProcessScanner, Tracker};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{lock, now, AppState};

const POLL: Duration = Duration::from_secs(5);

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

/// Point the tracker at the current install folders (after every sync).
pub fn refresh_dirs(app: &AppHandle) {
    let state = app.state::<AppState>();
    let dirs = lock(&state.db).install_dirs().unwrap_or_default();
    lock(&app.state::<Tracking>().tracker).set_install_dirs(dirs);
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

pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("playtime-tracker".into())
        .spawn(move || {
            let mut scanner = ProcessScanner::new();
            loop {
                let exes = scanner.exes();
                let events = lock(&app.state::<Tracking>().tracker)
                    .observe(exes.iter().map(|p| p.as_path()), now());
                if !events.is_empty() {
                    handle(&app, events);
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
}

fn handle(app: &AppHandle, events: Vec<Event>) {
    let mut changed = false;
    for e in events {
        if let Event::Ended(s) = e {
            let state = app.state::<AppState>();
            let db = lock(&state.db);
            if db.record_session(s.game_id, s.started_at, s.ended_at, SessionSource::Tracker).is_ok() {
                changed = true;
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
