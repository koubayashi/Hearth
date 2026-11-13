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

/// Whether idle time is kept separate by default, and each game's own choice.
type IdleSettings = (bool, Vec<(i64, IdleMode)>);
/// A paused game: id, seconds counted before the pause, reason, and the idle
/// time not yet recorded (since, last seen).
type PausedGame = (i64, i64, PauseReason, Option<(i64, i64)>);

#[derive(Default)]
pub struct Tracking {
    pub tracker: Mutex<Tracker>,
    /// The idle-time settings (Settings', then each game's own), read once
    /// and again only after they change: see `idle_settings_changed`.
    idle: Mutex<Option<IdleSettings>>,
    /// What paused.json holds, so it's written only when that changes.
    paused_json: Mutex<String>,
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
    lock(&app.state::<Tracking>().tracker).set_install_entries(dirs);
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
    let paused = load_paused(app);
    let tracking = app.state::<Tracking>();
    let mut tracker = lock(&tracking.tracker);
    for &(game, before, reason, _) in &paused {
        tracker.restore_paused(game, before, reason, now);
    }
    // Saved first, with the idle time starting now, so another crash can't
    // record the idle time below a second time. If it can't be saved, the
    // old list still holds that idle time: it's left for next time.
    let saved = save_paused(app, &tracker);
    drop(tracker);
    if !saved {
        return;
    }
    // An away pause's idle time up to its last checkpoint (a quit already
    // recorded it, so none is left here).
    let db = lock(&state.db);
    for (game, _, reason, span) in paused {
        if let Some((since, seen)) = span.filter(|(since, seen)| reason == PauseReason::Away && seen - since >= MIN_SESSION_SECS) {
            let _ = db.record_idle(game, since, seen);
        }
    }
}

/// Paused games, kept in paused.json so a pause survives Hearth restarting.
fn paused_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("paused.json"))
}

/// Called with the tracker locked, so an older list can never be written
/// over a newer one. Away pauses keep their idle time not yet recorded
/// (`since` to `seen`), so a crash loses at most a minute of it. Returns
/// whether paused.json holds the list now.
fn save_paused(app: &AppHandle, tracker: &Tracker) -> bool {
    let idle = tracker.idle_so_far();
    let list: Vec<serde_json::Value> = tracker
        .paused_list()
        .into_iter()
        .map(|(game, before, reason)| {
            let mut v = serde_json::json!({ "game": game, "before": before, "away": reason == PauseReason::Away });
            if let Some(&(_, since, seen)) = idle.iter().find(|i| i.0 == game) {
                v["since"] = since.into();
                v["seen"] = seen.into();
            }
            v
        })
        .collect();
    let text = serde_json::Value::Array(list).to_string();
    let tracking = app.state::<Tracking>();
    let mut last = lock(&tracking.paused_json);
    if *last == text {
        return true;
    }
    let Some(p) = paused_path(app) else { return false };
    // Written beside it and moved over it: a crash mid-write can't leave
    // half a list, which would drop every pause.
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &p)).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    *last = text;
    true
}

fn load_paused(app: &AppHandle) -> Vec<PausedGame> {
    let Some(text) = paused_path(app).and_then(|p| std::fs::read_to_string(p).ok()) else { return Vec::new() };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(&text) else { return Vec::new() };
    items
        .iter()
        .filter_map(|v| {
            let reason = if v.get("away")?.as_bool()? { PauseReason::Away } else { PauseReason::Manual };
            let span = v.get("since").and_then(|s| s.as_i64()).zip(v.get("seen").and_then(|s| s.as_i64()));
            Some((v.get("game")?.as_i64()?, v.get("before")?.as_i64()?, reason, span))
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

/// The idle-time setting, or a game's own choice, changed: read them again.
pub fn idle_settings_changed(app: &AppHandle) {
    *lock(&app.state::<Tracking>().idle) = None;
}

/// One look at who's at the PC, for games being timed or paused.
/// `now` is the time of the process look just before, so a game counts as
/// running at exactly the moment presence is judged.
fn check_presence(app: &AppHandle, now: i64, scanner: &ProcessScanner) {
    let tracking = app.state::<Tracking>();
    let busy = !lock(&tracking.tracker).sittings().is_empty();
    crate::presence::watch(busy);
    if !busy {
        return;
    }
    let (global, own) = lock(&tracking.idle)
        .get_or_insert_with(|| (idle_separate(app), lock(&app.state::<AppState>().db).idle_modes().unwrap_or_default()))
        .clone();
    let separate = |id: i64| own.iter().find(|(g, _)| *g == id).map_or(global, |(_, m)| *m == IdleMode::Separate);
    let front_exes = crate::presence::front_exes(&scanner.parents());
    let windows = crate::presence::windows();
    let last_input = crate::presence::last_input(now);
    step(app, false, |tracker| {
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
    });
}

/// Pause from the tray or a game's panel: counting stops now, and starts
/// again once you're back in the game (or press Resume).
pub fn pause(app: &AppHandle, game_id: i64) {
    step(app, true, |t| t.pause(game_id, now(), PauseReason::Manual));
}

pub fn resume(app: &AppHandle, game_id: i64) {
    step(app, true, |t| t.resume(game_id, now()));
}

/// Makes one change to the tracker and saves its outcome before the tracker
/// is unlocked, so nothing can slip in between (see `save`). With no
/// events, saves only for a `checkpoint`.
fn step(app: &AppHandle, checkpoint: bool, f: impl FnOnce(&mut Tracker) -> Vec<Event>) {
    let tracking = app.state::<Tracking>();
    let mut tracker = lock(&tracking.tracker);
    let events = f(&mut tracker);
    if events.is_empty() && !checkpoint {
        return;
    }
    let saved = save(app, &tracker, &events);
    drop(tracker);
    if !events.is_empty() {
        let changed = log_events(app, &events, saved);
        notify(app, changed);
    }
}

/// Saves what the tracker just did, with it locked. Paused games go first:
/// a crash right after loses a little idle time, never counts it twice.
/// Then the ended sessions, idle time and running sessions in one database
/// transaction, so an ended session can't stay in the checkpoint (it would
/// be recovered, and counted twice, after a crash).
fn save(app: &AppHandle, tracker: &Tracker, events: &[Event]) -> bool {
    save_paused(app, tracker);
    // Each with the version of the game it ran from, for separating
    // combined versions later.
    let spans = |idle: bool| -> Vec<hub_core::db::Span> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Ended(s) if !idle => Some(s),
                Event::Idle(s) if idle => Some(s),
                _ => None,
            })
            .map(|s| (s.game_id, tracker.entry_of(s.game_id), s.started_at, s.ended_at))
            .collect()
    };
    lock(&app.state::<AppState>().db).save_tracking(&spans(false), &spans(true), &tracker.active()).is_ok()
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
                polls = polls.wrapping_add(1);
                step(&app, polls.is_multiple_of(CHECKPOINT_EVERY), |t| t.observe(exes.iter().map(|p| p.as_path()), at));
                check_presence(&app, at, &scanner);
                std::thread::sleep(POLL);
            }
        })
        .expect("start tracker thread");
}

/// The user stopped timing a game from the tray: its session ends now.
pub fn stop(app: &AppHandle, game_id: i64) {
    log(app, &format!("stopped timing {} by hand", title(app, game_id)));
    step(app, true, |t| t.stop(game_id));
}

/// Close open sessions so quitting mid-game still counts the time played.
pub fn flush(app: &AppHandle) {
    step(app, true, |t| t.flush());
}

/// Restoring a backup swaps the library under the tracker, and game ids
/// differ between libraries. What was timed so far is saved to the library
/// being replaced (before `swap` keeps a copy of it), then the tracker starts over
/// with the restored library's games: one still running starts a new
/// session, and a pause is dropped. The tracker stays locked throughout, so
/// nothing is timed against the wrong library in between.
pub fn around_restore<T>(app: &AppHandle, swap: impl FnOnce() -> T) -> T {
    let tracking = app.state::<Tracking>();
    let mut tracker = lock(&tracking.tracker);
    let events = tracker.flush();
    let saved = save(app, &tracker, &events);
    log_events(app, &events, saved);
    *tracker = Tracker::new();
    let out = swap();
    tracker.set_install_entries(lock(&app.state::<AppState>().db).install_dirs().unwrap_or_default());
    save(app, &tracker, &[]);
    drop(tracker);
    idle_settings_changed(app);
    notify(app, false);
    out
}

/// Combining `other` into `keep` (`combine` does it in the library): with
/// the tracker locked throughout, so nothing is saved against `other` once
/// it's gone, and a game being timed carries on under `keep`.
pub fn around_combine<T>(app: &AppHandle, keep: i64, other: i64, combine: impl FnOnce() -> crate::CmdResult<T>) -> crate::CmdResult<T> {
    let tracking = app.state::<Tracking>();
    let mut tracker = lock(&tracking.tracker);
    let out = combine()?;
    tracker.renumber(other, keep);
    tracker.set_install_entries(lock(&app.state::<AppState>().db).install_dirs().unwrap_or_default());
    Ok(out)
}

/// Writes the events to tracker.log; true if play or idle time was saved.
fn log_events(app: &AppHandle, events: &[Event], saved: bool) -> bool {
    let note = if saved { "" } else { " (not saved)" };
    let mut changed = false;
    for e in events {
        match e {
            Event::Started { game_id, .. } => log(app, &format!("started {}", title(app, *game_id))),
            Event::Ended(s) => {
                log(app, &format!("ended {} after {}s{note}", title(app, s.game_id), s.ended_at - s.started_at));
                changed |= saved;
            }
            Event::Paused { game_id, reason } => {
                let why = if *reason == PauseReason::Away { "away" } else { "by hand" };
                log(app, &format!("paused {} ({why})", title(app, *game_id)));
            }
            Event::Resumed { game_id, .. } => log(app, &format!("resumed {}", title(app, *game_id))),
            Event::Closed { game_id } => log(app, &format!("{} closed while paused", title(app, *game_id))),
            Event::Idle(s) => {
                log(app, &format!("{} idle for {}s{note}", title(app, s.game_id), s.ended_at - s.started_at));
                changed |= saved;
            }
        }
    }
    changed
}

/// Tells the tray and the window what's being timed now.
fn notify(app: &AppHandle, library_changed: bool) {
    let playing = now_playing(app);
    crate::tray::set_playing(app, &playing);
    let _ = app.emit("now-playing", &playing);
    if library_changed {
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
