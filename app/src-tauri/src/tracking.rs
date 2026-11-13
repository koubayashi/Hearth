//! Runs the playtime tracker on its own thread for as long as Hearth is open,
//! including while it sits in the tray with no window.
//!
//! Running sessions are saved every minute, so if Hearth is killed, crashes or
//! the PC shuts down mid-game, the time up to the last save is recorded the
//! next time Hearth starts. Every start and end is written to `tracker.log`.

use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;

use hub_core::{IdleMode, SessionSource};
use hub_tracker::{Event, PauseReason, Presence, ProcessScanner, Tracker, MIN_SESSION_SECS};
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
    /// When the session being counted started; None while paused.
    pub started_at: Option<i64>,
    /// Seconds counted earlier in this sitting, before a pause.
    pub before_secs: i64,
    /// "manual" (the Pause button) or "away" (Hearth saw you step away).
    pub paused: Option<&'static str>,
}

/// Appends a line to `tracker.log` in the app data folder.
pub fn log(app: &AppHandle, msg: &str) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    let path = dir.join("tracker.log");
    // Keep the log small: start over once it passes 1 MB.
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    // One write per line, so lines from different threads never interleave.
    let line = format!("{} {}\n", now(), redact(msg));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Drops the query from every web address in a message. Some stores take
/// their token in the address, and an error that quotes it must not put it
/// in a plain-text log.
pub fn redact(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    let next = |s: &str| [s.find("https://"), s.find("http://")].into_iter().flatten().min();
    while let Some(at) = next(rest) {
        out.push_str(&rest[..at]);
        let url = &rest[at..];
        let end = url.find(|c: char| c.is_whitespace() || c == ')' || c == '"').unwrap_or(url.len());
        out.push_str(url[..end].split('?').next().unwrap_or(""));
        rest = &url[end..];
    }
    out.push_str(rest);
    out
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
    drop(db);
    // Games paused when Hearth stopped (a restart for an update, a quit, a
    // crash) stay paused: they resume once played again, or close.
    let now = now();
    let tracking = app.state::<Tracking>();
    let mut tracker = lock(&tracking.tracker);
    for (game, before, reason) in load_paused(app) {
        tracker.restore_paused(game, before, reason, now);
    }
}

/// Paused games, kept in paused.json so a pause survives Hearth restarting.
fn paused_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("paused.json"))
}

fn save_paused(app: &AppHandle) {
    let list: Vec<serde_json::Value> = lock(&app.state::<Tracking>().tracker)
        .paused_list()
        .into_iter()
        .map(|(game, before, reason)| serde_json::json!({ "game": game, "before": before, "away": reason == PauseReason::Away }))
        .collect();
    if let Some(p) = paused_path(app) {
        let _ = std::fs::write(p, serde_json::Value::Array(list).to_string());
    }
}

fn load_paused(app: &AppHandle) -> Vec<(i64, i64, PauseReason)> {
    let Some(text) = paused_path(app).and_then(|p| std::fs::read_to_string(p).ok()) else { return Vec::new() };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(&text) else { return Vec::new() };
    items
        .iter()
        .filter_map(|v| {
            let reason = if v.get("away")?.as_bool()? { PauseReason::Away } else { PauseReason::Manual };
            Some((v.get("game")?.as_i64()?, v.get("before")?.as_i64()?, reason))
        })
        .collect()
}

pub fn now_playing(app: &AppHandle) -> Vec<NowPlaying> {
    let sittings = lock(&app.state::<Tracking>().tracker).sittings();
    let state = app.state::<AppState>();
    let db = lock(&state.db);
    sittings
        .into_iter()
        .filter_map(|s| {
            let title = db.game(s.game_id).ok()??.title;
            let paused = s.paused.map(|r| match r {
                PauseReason::Manual => "manual",
                PauseReason::Away => "away",
            });
            Some(NowPlaying { game_id: s.game_id, title, started_at: s.started_at, before_secs: s.before, paused })
        })
        .collect()
}

/// "Idle time" in Settings: kept separate unless set to count as play. The
/// window saves its settings to prefs.json, which is read here since Hearth
/// may be running in the tray with no window.
fn idle_separate(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_data_dir() else { return true };
    let prefs = std::fs::read_to_string(dir.join("prefs.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let Some(v) = prefs else { return true };
    match v.get("idleTime").and_then(|m| m.as_str()) {
        Some(m) => m != "count",
        // Settings saved before "Idle time" had the "Don't count time away" switch.
        None => v.get("pauseWhenAway").and_then(|b| b.as_bool()).unwrap_or(true),
    }
}

/// One look at who's at the PC, for games being timed or paused.
/// `now` is the time of the process look just before, so a game counts as
/// running at exactly the moment presence is judged.
fn check_presence(app: &AppHandle, now: i64) -> Vec<Event> {
    let busy = !lock(&app.state::<Tracking>().tracker).sittings().is_empty();
    crate::presence::watch(busy);
    if !busy {
        return Vec::new();
    }
    let global = idle_separate(app);
    let own = lock(&app.state::<AppState>().db).idle_modes().unwrap_or_default();
    let separate = |id: i64| own.iter().find(|(g, _)| *g == id).map_or(global, |(_, m)| *m == IdleMode::Separate);
    let front_exes = crate::presence::front_exes();
    let windows = crate::presence::windows();
    let last_input = crate::presence::last_input(now);
    let tracking = app.state::<Tracking>();
    let mut tracker = lock(&tracking.tracker);
    // The window in front is a game's if its program, or the one that
    // started it, lives in the game's folder.
    let front = front_exes.iter().find_map(|e| tracker.game_for_exe(e));
    let games = |exes: &[std::path::PathBuf]| {
        let mut ids: Vec<i64> = exes.iter().filter_map(|e| tracker.game_for_exe(e)).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let (elsewhere, windowed) = (games(&windows.elsewhere), games(&windows.windowed));
    tracker.presence(Presence { now, last_input, front, elsewhere, windowed }, separate)
}

/// Pause from the tray or a game's panel: counting stops now, and starts
/// again once you're back in the game (or press Resume).
pub fn pause(app: &AppHandle, game_id: i64) {
    let events = lock(&app.state::<Tracking>().tracker).pause(game_id, now(), PauseReason::Manual);
    handle(app, events);
    checkpoint(app);
}

pub fn resume(app: &AppHandle, game_id: i64) {
    let events = lock(&app.state::<Tracking>().tracker).resume(game_id, now());
    handle(app, events);
    checkpoint(app);
}

/// Saves the running sessions. The tracker stays locked until they're
/// written, so a pause can't slip in between and leave an ended session in
/// the checkpoint (it would be recovered, and counted twice, after a crash).
fn checkpoint(app: &AppHandle) {
    let tracking = app.state::<Tracking>();
    let tracker = lock(&tracking.tracker);
    let _ = lock(&app.state::<AppState>().db).checkpoint_active(&tracker.active());
}

pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("playtime-tracker".into())
        .spawn(move || {
            let mut scanner = ProcessScanner::new();
            let mut polls: u32 = 0;
            loop {
                let exes = scanner.exes();
                let at = now();
                let mut events = lock(&app.state::<Tracking>().tracker).observe(exes.iter().map(|p| p.as_path()), at);
                events.extend(check_presence(&app, at));
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

/// The user stopped timing a game from the tray: its session ends now.
pub fn stop(app: &AppHandle, game_id: i64) {
    let events = lock(&app.state::<Tracking>().tracker).stop(game_id);
    log(app, &format!("stopped timing {} by hand", title(app, game_id)));
    handle(app, events);
    checkpoint(app);
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
            Event::Paused { game_id, reason } => {
                let why = if reason == PauseReason::Away { "away" } else { "by hand" };
                log(app, &format!("paused {} ({why})", title(app, game_id)));
            }
            Event::Resumed { game_id, .. } => log(app, &format!("resumed {}", title(app, game_id))),
            Event::Closed { game_id } => log(app, &format!("{} closed while paused", title(app, game_id))),
            Event::Idle(s) => {
                let ok = lock(&app.state::<AppState>().db).record_idle(s.game_id, s.started_at, s.ended_at).is_ok();
                log(app, &format!("{} idle for {}s{}", title(app, s.game_id), s.ended_at - s.started_at, if ok { "" } else { " (not saved)" }));
                changed |= ok;
            }
        }
    }
    save_paused(app);
    let playing = now_playing(app);
    crate::tray::set_playing(app, &playing);
    let _ = app.emit("now-playing", &playing);
    if changed {
        let _ = app.emit("library-changed", ());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn logs_never_keep_tokens_from_addresses() {
        let msg = "gog: couldn't renew yet (error sending request for url (https://auth.gog.com/token?client_id=1&refresh_token=SECRET))";
        let clean = super::redact(msg);
        assert!(!clean.contains("SECRET"));
        assert!(clean.contains("https://auth.gog.com/token)"));
    }
}
