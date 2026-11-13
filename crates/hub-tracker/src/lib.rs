//! Playtime tracking for games started any way at all: from Hearth, from a
//! store launcher, or straight from their folder.
//!
//! A game counts as running while any process whose executable lives inside
//! its install folder is alive. A short grace period bridges launcher → game
//! hand-offs (and restarts) so they record as one session.

use std::collections::HashMap;
use std::path::Path;

mod poll;
pub use poll::ProcessScanner;

/// Seconds a game may have no process before its session is closed.
pub const GRACE_SECS: i64 = 15;
/// Sessions shorter than this are launch hiccups, not play.
pub const MIN_SESSION_SECS: i64 = 30;
/// Longer than this between two looks (polls are every 5s) means the PC was
/// asleep; the gap isn't counted.
pub const MAX_GAP_SECS: i64 = 120;

/// Helper processes that live in game folders but don't mean you're playing:
/// any exe whose name contains one of these...
const IGNORED_PARTS: &[&str] = &[
    "crashreport", "crashhandler", "crashpad", "bugsplat", "vcredist", "vc_redist", "dxsetup", "dxwebsetup",
    "dotnetfx", "ue4prereq", "ueprereq", "easyanticheat_setup",
];
/// ...or whose name starts with one of these as a whole word: "setup.exe",
/// "setup_x64.exe" and "unins000.exe", but not "Redistrict.exe".
const IGNORED_WORDS: &[&str] = &["unins", "uninstall", "installer", "setup", "redist"];

fn is_helper(file: &str) -> bool {
    IGNORED_PARTS.iter().any(|i| file.contains(i))
        || IGNORED_WORDS.iter().any(|w| file.strip_prefix(w).is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphabetic())))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub game_id: i64,
    pub started_at: i64,
    pub ended_at: i64,
}

/// No keyboard, mouse or controller input for this long while a game runs:
/// you've stepped away, and the time since your last input isn't counted.
pub const AWAY_IDLE_SECS: i64 = 600;
/// The game not in front for this long: you're doing something else, and
/// the time since you left it isn't counted. A quick look at a wiki stays.
pub const AWAY_BACKGROUND_SECS: i64 = 300;
/// The same, for a game still showing on another screen (Discord on one,
/// the game on the other): longer, as you may still be watching it. Typing
/// in the other window doesn't keep it counting.
pub const AWAY_ELSEWHERE_SECS: i64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Started { game_id: i64, at: i64 },
    Ended(Session),
    /// Counting stopped while the game runs on (the session up to then is
    /// an `Ended` before it).
    Paused { game_id: i64, reason: PauseReason },
    /// Counting again after a pause: a new session starts.
    Resumed { game_id: i64, at: i64 },
    /// A paused game closed, or stopped being timed: nothing to record.
    Closed { game_id: i64 },
    /// Time the game ran while you were idle (an away pause, not one you
    /// chose): kept apart from play, so it can be shown on its own.
    Idle(Session),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    /// Paused with the Pause button.
    Manual,
    /// Hearth saw you step away or switch to something else.
    Away,
}

/// What Windows says about the person at the PC, for away detection.
#[derive(Debug, Clone, Default)]
pub struct Presence {
    pub now: i64,
    /// Last keyboard, mouse or controller input (unix seconds).
    pub last_input: i64,
    /// The game whose window is in front, if any.
    pub front: Option<i64>,
    /// Games still showing on another screen than the window in front.
    pub elsewhere: Vec<i64>,
    /// Games that have a window of their own (shown or minimised). Only these
    /// can be "left behind": a game whose window belongs to another program
    /// (javaw.exe, an emulator) can't be told apart from not being in front.
    pub windowed: Vec<i64>,
}

/// One game's time since it was started: counting now, or paused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sitting {
    pub game_id: i64,
    /// When the session being counted started; None while paused.
    pub started_at: Option<i64>,
    /// Seconds counted earlier in this sitting, before a pause.
    pub before: i64,
    pub paused: Option<PauseReason>,
}

#[derive(Debug)]
struct Active {
    started_at: i64,
    last_seen: i64,
    before: i64,
    /// Last time its window was in front; None if it never has been (some
    /// games' windows belong to another process), so being behind can't be
    /// told apart from that and isn't treated as away.
    last_front: Option<i64>,
}

#[derive(Debug)]
struct Paused {
    /// Input after this (in the game) resumes it.
    resume_after: i64,
    /// Start of the idle time not yet recorded (away pauses only).
    since: i64,
    last_seen: i64,
    before: i64,
    reason: PauseReason,
    front_known: bool,
    /// When the game last became the window in front while paused: only
    /// input after this was input in the game.
    front_since: Option<i64>,
}

/// Input times are whole seconds, rounded either way: a press this close to
/// the pause may be the click on Pause itself.
const INPUT_MARGIN_SECS: i64 = 2;

#[derive(Debug, Default)]
pub struct Tracker {
    /// (normalized dir with trailing separator, game id), longest dir first.
    dirs: Vec<(String, i64)>,
    active: HashMap<i64, Active>,
    /// Games still running but not being counted.
    paused: HashMap<i64, Paused>,
    /// Games the user stopped timing by hand: not timed again until none of
    /// their processes is running (a helper left behind can't restart it).
    muted: Vec<i64>,
}

fn normalize(p: &str) -> String {
    let mut s = p.replace('/', "\\").to_lowercase();
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the known install folders (after every library sync).
    pub fn set_install_dirs(&mut self, dirs: impl IntoIterator<Item = (i64, String)>) {
        self.dirs = dirs
            .into_iter()
            .filter(|(_, d)| d.len() > 3) // never a whole drive like "D:\"
            .map(|(id, d)| (normalize(&d), id))
            .collect();
        // Nested folders: the most specific match wins.
        self.dirs.sort_by_key(|(d, _)| std::cmp::Reverse(d.len()));
    }

    /// Which game an executable belongs to, if any.
    pub fn game_for_exe(&self, exe: &Path) -> Option<i64> {
        let path = exe.to_string_lossy().replace('/', "\\").to_lowercase();
        let file = path.rsplit('\\').next().unwrap_or("");
        if !file.ends_with(".exe") || is_helper(file) {
            return None;
        }
        self.dirs.iter().find(|(d, _)| path.starts_with(d.as_str())).map(|(_, id)| *id)
    }

    /// Running sessions as (game id, started, last seen), for checkpoints.
    pub fn active(&self) -> Vec<(i64, i64, i64)> {
        self.active.iter().map(|(id, a)| (*id, a.started_at, a.last_seen)).collect()
    }

    /// Every game being timed or paused.
    pub fn sittings(&self) -> Vec<Sitting> {
        let counting = self.active.iter().map(|(id, a)| Sitting { game_id: *id, started_at: Some(a.started_at), before: a.before, paused: None });
        let paused = self.paused.iter().map(|(id, p)| Sitting { game_id: *id, started_at: None, before: p.before, paused: Some(p.reason) });
        let mut all: Vec<Sitting> = counting.chain(paused).collect();
        all.sort_by_key(|s| s.game_id);
        all
    }

    /// Stop counting a running game at `at` (clamped to its session); it
    /// stays paused until input reaches it after `at`, or `resume`.
    pub fn pause(&mut self, id: i64, at: i64, reason: PauseReason) -> Vec<Event> {
        let Some(a) = self.active.remove(&id) else { return Vec::new() };
        let end = at.clamp(a.started_at, a.last_seen);
        let counted = kept(end - a.started_at, a.before);
        let before = a.before + if counted { end - a.started_at } else { 0 };
        self.paused.insert(id, Paused { resume_after: at, since: end, last_seen: a.last_seen, before, reason, front_known: a.last_front.is_some(), front_since: None });
        let mut events = Vec::new();
        if counted {
            events.push(Event::Ended(Session { game_id: id, started_at: a.started_at, ended_at: end }));
        }
        events.push(Event::Paused { game_id: id, reason });
        events
    }

    /// Count a paused game again, from `now`.
    pub fn resume(&mut self, id: i64, now: i64) -> Vec<Event> {
        let Some(p) = self.paused.remove(&id) else { return Vec::new() };
        let last_front = p.front_known.then_some(now);
        self.active.insert(id, Active { started_at: now, last_seen: now, before: p.before, last_front });
        idle(id, &p, now).into_iter().chain([Event::Resumed { game_id: id, at: now }]).collect()
    }

    /// A pause kept across a restart of Hearth (an update, a quit): the game
    /// stays paused until it's played again, or closes.
    pub fn restore_paused(&mut self, id: i64, before: i64, reason: PauseReason, now: i64) {
        if self.active.contains_key(&id) {
            return;
        }
        self.paused.insert(id, Paused { resume_after: now, since: now, last_seen: now, before, reason, front_known: true, front_since: None });
    }

    /// Away detection: pauses games you've stepped away from or left behind
    /// (those `separate` says keep idle time apart), and resumes paused ones
    /// once you're back in them.
    pub fn presence(&mut self, p: Presence, separate: impl Fn(i64) -> bool) -> Vec<Event> {
        for (id, a) in self.active.iter_mut() {
            if p.front == Some(*id) {
                a.last_front = Some(p.now);
            }
        }
        let mut events = Vec::new();
        {
            let idle = (p.now - p.last_input >= AWAY_IDLE_SECS).then_some(p.last_input);
            let away: Vec<(i64, i64)> = self
                .active
                .iter()
                .filter(|(id, _)| separate(**id))
                .filter_map(|(id, a)| {
                    let limit = if p.elsewhere.contains(id) { AWAY_ELSEWHERE_SECS } else { AWAY_BACKGROUND_SECS };
                    let behind = a.last_front.filter(|f| p.now - f >= limit && p.windowed.contains(id));
                    idle.into_iter().chain(behind).min().map(|at| (*id, at))
                })
                .collect();
            for (id, at) in away {
                events.extend(self.pause(id, at, PauseReason::Away));
            }
        }
        for (id, q) in self.paused.iter_mut() {
            q.front_since = if p.front == Some(*id) { Some(q.front_since.unwrap_or(p.now)) } else { None };
        }
        // Back: input in the game after it paused, and after it became the
        // window in front (typing in Discord, then closing it, isn't playing).
        // A game whose window was never seen in front resumes on any input,
        // but only from away: a Pause you chose waits for the game itself.
        let back: Vec<i64> = self
            .paused
            .iter()
            .filter(|(id, q)| {
                let after_pause = p.last_input >= q.resume_after + INPUT_MARGIN_SECS;
                let in_game = q.front_since.is_some_and(|since| p.front == Some(**id) && p.last_input > since);
                // Hearth can't see this game's window at all: from away, any
                // input means you're back at the PC.
                // Only while it's running, though: a game between two processes
                // (a launcher restarting it) has no window for a moment either.
                let unseen = (!q.front_known || !p.windowed.contains(id)) && q.last_seen >= p.now;
                after_pause && (in_game || (unseen && q.reason == PauseReason::Away))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in back {
            events.extend(self.resume(id, p.now));
        }
        events
    }

    /// Feed one snapshot of running executables taken at `now` (unix seconds).
    pub fn observe<'a>(&mut self, exes: impl IntoIterator<Item = &'a Path>, now: i64) -> Vec<Event> {
        let mut seen: Vec<i64> = exes.into_iter().filter_map(|e| self.game_for_exe(e)).collect();
        seen.sort_unstable();
        seen.dedup();
        self.muted.retain(|id| seen.contains(id));
        seen.retain(|id| !self.muted.contains(id));

        let mut events = Vec::new();
        // A long silence means the PC slept (or Hearth stalled) with the game
        // open: that time wasn't play. The session ends when it was last seen,
        // and one still running starts a new session now.
        // The time already counted carries on into the new session, so
        // "Playing now" doesn't start again from zero after the PC wakes.
        let stale: Vec<i64> = self.active.iter().filter(|(_, a)| now - a.last_seen > MAX_GAP_SECS).map(|(id, _)| *id).collect();
        // The same for a game paused while idle: the sleep isn't idle time.
        for (id, p) in self.paused.iter_mut() {
            if now - p.last_seen > MAX_GAP_SECS {
                events.extend(idle(*id, p, p.last_seen));
                p.since = now;
            }
        }
        let mut carried: HashMap<i64, i64> = HashMap::new();
        for id in stale {
            let before = self.active.get(&id).map(|a| a.before).unwrap_or(0);
            match self.end(id) {
                Some(Event::Ended(s)) => {
                    carried.insert(id, before + s.ended_at - s.started_at);
                    events.push(Event::Ended(s));
                }
                _ => {
                    carried.insert(id, before);
                }
            }
        }
        for id in &seen {
            if let Some(p) = self.paused.get_mut(id) {
                p.last_seen = now;
                continue;
            }
            match self.active.get_mut(id) {
                Some(a) => a.last_seen = now,
                None => {
                    let before = carried.get(id).copied().unwrap_or(0);
                    self.active.insert(*id, Active { started_at: now, last_seen: now, before, last_front: None });
                    events.push(Event::Started { game_id: *id, at: now });
                }
            }
        }
        // A paused game that has closed: nothing more to count.
        let closed: Vec<i64> = self.paused.iter().filter(|(id, p)| !seen.contains(id) && now - p.last_seen >= GRACE_SECS).map(|(id, _)| *id).collect();
        for id in closed {
            if let Some(p) = self.paused.remove(&id) {
                events.extend(idle(id, &p, p.last_seen));
            }
            events.push(Event::Closed { game_id: id });
        }
        let gone: Vec<i64> = self
            .active
            .iter()
            .filter(|(id, a)| !seen.contains(id) && now - a.last_seen >= GRACE_SECS)
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            if let Some(e) = self.end(id) {
                events.push(e);
            }
        }
        events
    }

    /// The user says this game isn't being played: close its session now and
    /// leave it alone until its processes have all closed.
    pub fn stop(&mut self, id: i64) -> Vec<Event> {
        if let Some(p) = self.paused.remove(&id) {
            self.muted.push(id);
            return idle(id, &p, p.last_seen).into_iter().chain([Event::Closed { game_id: id }]).collect();
        }
        if !self.active.contains_key(&id) {
            return Vec::new();
        }
        self.muted.push(id);
        self.end(id).into_iter().collect()
    }

    /// Close every open session, e.g. when Hearth quits. Paused games stay
    /// paused (see `paused_list`, which Hearth saves to restore them); their
    /// idle time so far is recorded now.
    pub fn flush(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        for (id, p) in self.paused.iter_mut() {
            events.extend(idle(*id, p, p.last_seen));
            p.since = p.last_seen;
        }
        let ids: Vec<i64> = self.active.keys().copied().collect();
        events.extend(ids.into_iter().filter_map(|id| self.end(id)));
        events
    }

    /// Paused games as (game id, seconds counted before the pause, reason),
    /// to keep them paused across a restart of Hearth.
    pub fn paused_list(&self) -> Vec<(i64, i64, PauseReason)> {
        let mut v: Vec<_> = self.paused.iter().map(|(id, p)| (*id, p.before, p.reason)).collect();
        v.sort_by_key(|x| x.0);
        v
    }

    /// Idle time of away pauses not recorded yet, as (game id, start, last
    /// seen), saved with the checkpoint so a crash loses at most a minute.
    pub fn idle_so_far(&self) -> Vec<(i64, i64, i64)> {
        self.paused.iter().filter(|(_, p)| p.reason == PauseReason::Away).map(|(id, p)| (*id, p.since, p.last_seen)).collect()
    }

    fn end(&mut self, id: i64) -> Option<Event> {
        let a = self.active.remove(&id)?;
        // The session ends when the game was last seen, not when we noticed.
        let s = Session { game_id: id, started_at: a.started_at, ended_at: a.last_seen };
        kept(s.ended_at - s.started_at, a.before).then_some(Event::Ended(s))
    }
}

/// Whether a session of `secs` is play worth recording. A short one at the
/// start is a launch hiccup; after a pause or a sleep the sitting has already
/// counted time, so the few seconds since are play too.
fn kept(secs: i64, before: i64) -> bool {
    secs >= MIN_SESSION_SECS || (before > 0 && secs > 0)
}

/// The idle time of an away pause, from its start up to `until`. A pause you
/// chose is never idle time: you said you weren't playing. Like a session,
/// a few seconds of it is a launch hiccup, not time the game was open.
fn idle(game_id: i64, p: &Paused, until: i64) -> Option<Event> {
    (p.reason == PauseReason::Away && until - p.since >= MIN_SESSION_SECS).then_some(Event::Idle(Session { game_id, started_at: p.since, ended_at: until }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tracker() -> Tracker {
        let mut t = Tracker::new();
        t.set_install_dirs([
            (1, r"D:\Games\SteamLibrary\steamapps\common\HITMAN 3".to_string()),
            (2, r"D:\Games\Epic Games\RedDeadRedemption2".to_string()),
            (3, r"D:\Games\Epic Games\RedDeadRedemption2\Bonus".to_string()),
            (9, r"D:\".to_string()),
        ]);
        t
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn matches_exe_inside_install_folder_case_insensitively() {
        let t = tracker();
        assert_eq!(t.game_for_exe(&p(r"d:\games\steamlibrary\steamapps\common\hitman 3\Retail\HITMAN3.exe")), Some(1));
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\RDR2.exe")), Some(2));
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\Bonus\x.exe")), Some(3));
        // A sibling folder with a shared prefix is a different game.
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2Beta\x.exe")), None);
        // A bare drive is never treated as an install folder.
        assert_eq!(t.game_for_exe(&p(r"D:\Tools\notepad.exe")), None);
    }

    #[test]
    fn ignores_helpers() {
        let t = tracker();
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\CrashReportClient.exe")), None);
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\unins000.exe")), None);
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\UnityCrashHandler64.exe")), None);
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\Setup.exe")), None);
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\setup_x64.exe")), None);
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\Redist\vcredist_x64.exe")), None);
    }

    #[test]
    fn a_game_named_like_a_helper_is_still_timed() {
        let t = tracker();
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\Redistrict.exe")), Some(2));
        assert_eq!(t.game_for_exe(&p(r"D:\Games\Epic Games\RedDeadRedemption2\SetupTheSun.exe")), Some(2));
    }

    #[test]
    fn a_short_stretch_between_pauses_still_counts() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2000, PauseReason::Manual);
        t.resume(2, 2100);
        play(&mut t, 2100, 2125);
        assert_eq!(
            t.pause(2, 2125, PauseReason::Manual),
            [Event::Ended(Session { game_id: 2, started_at: 2100, ended_at: 2125 }), Event::Paused { game_id: 2, reason: PauseReason::Manual }]
        );
        assert_eq!(t.paused_list(), [(2, 1025, PauseReason::Manual)]);
        // The same when it closes a few seconds after resuming.
        t.resume(2, 2200);
        play(&mut t, 2200, 2210);
        assert_eq!(t.observe([], 2230), [Event::Ended(Session { game_id: 2, started_at: 2200, ended_at: 2210 })]);
    }

    #[test]
    fn records_one_session_across_launcher_handoff() {
        let mut t = tracker();
        let launcher = p(r"D:\Games\Epic Games\RedDeadRedemption2\PlayRDR2.exe");
        let game = p(r"D:\Games\Epic Games\RedDeadRedemption2\RDR2.exe");

        assert_eq!(t.observe([launcher.as_path()], 1000), [Event::Started { game_id: 2, at: 1000 }]);
        // Launcher exits, game starts 10s later: still the same session.
        assert!(t.observe([], 1005).is_empty());
        for now in (1010..=4600).step_by(5) {
            assert!(t.observe([game.as_path()], now).is_empty());
        }
        assert!(t.observe([], 4605).is_empty());
        assert_eq!(
            t.observe([], 4620),
            [Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 4600 })]
        );
    }

    #[test]
    fn sleep_with_the_game_open_is_not_playtime() {
        let mut t = tracker();
        let game = p(r"D:\Games\Epic Games\RedDeadRedemption2\RDR2.exe");
        for now in (1000..=1600).step_by(5) {
            t.observe([game.as_path()], now);
        }
        // The PC slept for 8 hours; the game is still open when it wakes.
        let woke = 1600 + 8 * 3600;
        assert_eq!(
            t.observe([game.as_path()], woke),
            [
                Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 1600 }),
                Event::Started { game_id: 2, at: woke },
            ]
        );
    }

    #[test]
    fn drops_blips_and_flushes_on_quit() {
        let mut t = tracker();
        let hitman = p(r"D:\Games\SteamLibrary\steamapps\common\HITMAN 3\Retail\HITMAN3.exe");
        t.observe([hitman.as_path()], 0);
        assert!(t.observe([], 20).is_empty(), "a 0s blip is not a session");

        for now in (100..=700).step_by(5) {
            t.observe([hitman.as_path()], now);
        }
        assert_eq!(t.flush(), [Event::Ended(Session { game_id: 1, started_at: 100, ended_at: 700 })]);
        assert!(t.sittings().is_empty());
    }

    #[test]
    fn a_stopped_game_waits_for_its_processes_to_close() {
        let mut t = tracker();
        let helper = p(r"D:\Games\Epic Games\RedDeadRedemption2\Launcher.exe");
        t.observe([helper.as_path()], 1000);
        t.observe([helper.as_path()], 1100);
        assert_eq!(t.stop(2), [Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 1100 })]);
        // The helper is still there: no new session.
        assert!(t.observe([helper.as_path()], 1105).is_empty());
        assert!(t.sittings().is_empty());
        // Once it has closed, the next start is timed again.
        assert!(t.observe([], 1110).is_empty());
        assert_eq!(t.observe([helper.as_path()], 2000), [Event::Started { game_id: 2, at: 2000 }]);
    }

    fn rdr2() -> PathBuf {
        p(r"D:\Games\Epic Games\RedDeadRedemption2\RDR2.exe")
    }

    /// One look at the PC. RDR2 (game 2) has its own window unless a test
    /// says otherwise.
    fn look(now: i64, last_input: i64, front: Option<i64>) -> Presence {
        Presence { now, last_input, front, elsewhere: vec![], windowed: vec![2] }
    }

    /// Plays RDR2 from `from` to `to`, in front, with input all along.
    fn play(t: &mut Tracker, from: i64, to: i64) {
        let game = rdr2();
        for now in (from..=to).step_by(5) {
            t.observe([game.as_path()], now);
            t.presence(look(now, now, Some(2)), |_| true);
        }
    }

    #[test]
    fn pause_ends_the_session_and_input_in_the_game_resumes_it() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        assert_eq!(
            t.pause(2, 2002, PauseReason::Manual),
            [Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 2000 }), Event::Paused { game_id: 2, reason: PauseReason::Manual }]
        );
        let game = rdr2();
        // Still running, not counted. Input elsewhere doesn't resume it.
        assert!(t.observe([game.as_path()], 2100).is_empty());
        assert!(t.presence(look(2100, 2099, None), |_| true).is_empty());
        assert_eq!(t.sittings(), [Sitting { game_id: 2, started_at: None, before: 1000, paused: Some(PauseReason::Manual) }]);
        // Clicked back into the game: the click that brought it to the front
        // (before Hearth saw it there) doesn't count; the next key does.
        assert!(t.presence(look(2495, 2494, Some(2)), |_| true).is_empty());
        assert_eq!(t.presence(look(2500, 2499, Some(2)), |_| true), [Event::Resumed { game_id: 2, at: 2500 }]);
        assert_eq!(t.sittings(), [Sitting { game_id: 2, started_at: Some(2500), before: 1000, paused: None }]);
    }

    #[test]
    fn the_click_on_pause_does_not_resume_it() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2003, PauseReason::Manual);
        // Focus goes back to the game after the click in the tray. The click
        // (2003.9) can read as 2004 once rounded: still not input after it.
        assert!(t.presence(look(2005, 2004, Some(2)), |_| true).is_empty());
        assert!(t.presence(look(2010, 2004, Some(2)), |_| true).is_empty());
    }

    #[test]
    fn typing_elsewhere_then_coming_back_does_not_resume_a_pause() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2002, PauseReason::Manual);
        // Typing in Discord, then Discord closes and the game is in front again.
        assert!(t.presence(look(2100, 2100, None), |_| true).is_empty());
        assert!(t.presence(look(2105, 2100, Some(2)), |_| true).is_empty());
        assert!(t.presence(look(2110, 2100, Some(2)), |_| true).is_empty());
        // A key in the game.
        assert_eq!(t.presence(look(2115, 2112, Some(2)), |_| true), [Event::Resumed { game_id: 2, at: 2115 }]);
    }

    #[test]
    fn stepping_away_stops_counting_at_the_last_input() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        let game = rdr2();
        for now in (2005..=2600).step_by(5) {
            t.observe([game.as_path()], now);
            let ev = t.presence(look(now, 2000, Some(2)), |_| true);
            if now - 2000 >= AWAY_IDLE_SECS {
                assert_eq!(ev, [Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 2000 }), Event::Paused { game_id: 2, reason: PauseReason::Away }]);
                break;
            }
            assert!(ev.is_empty());
        }
    }

    #[test]
    fn leaving_the_game_behind_stops_counting_when_you_left() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        let game = rdr2();
        // In the browser from 2000 on, typing away.
        let mut paused_at = None;
        for now in (2005..=2400).step_by(5) {
            t.observe([game.as_path()], now);
            let ev = t.presence(look(now, now, None), |_| true);
            if !ev.is_empty() {
                assert_eq!(ev[0], Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 2000 }));
                paused_at = Some(now);
                break;
            }
        }
        assert_eq!(paused_at, Some(2000 + AWAY_BACKGROUND_SECS));
        // Typing elsewhere doesn't bring it back; clicking into the game and
        // pressing something does.
        assert!(t.presence(look(2400, 2400, None), |_| true).is_empty());
        assert!(t.presence(look(2405, 2404, Some(2)), |_| true).is_empty());
        assert_eq!(t.presence(look(2410, 2408, Some(2)), |_| true), [Event::Idle(Session { game_id: 2, started_at: 2000, ended_at: 2410 }), Event::Resumed { game_id: 2, at: 2410 }]);
    }

    #[test]
    fn a_game_on_another_screen_gets_longer_but_typing_elsewhere_does_not_keep_it() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        let game = rdr2();
        // In Discord from 2000 on, typing; the game still shows on the other screen.
        let mut paused_at = None;
        for now in (2005..=2800).step_by(5) {
            t.observe([game.as_path()], now);
            let ev = t.presence(Presence { elsewhere: vec![2], ..look(now, now, None) }, |_| true);
            if !ev.is_empty() {
                assert_eq!(ev[0], Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 2000 }));
                paused_at = Some(now);
                break;
            }
        }
        assert_eq!(paused_at, Some(2000 + AWAY_ELSEWHERE_SECS));
    }

    #[test]
    fn a_game_never_seen_in_front_is_only_away_when_idle() {
        let mut t = tracker();
        let game = rdr2();
        for now in (1000..=2000).step_by(5) {
            t.observe([game.as_path()], now);
            // Its window belongs to another process: never "in front".
            assert!(t.presence(look(now, now, None), |_| true).is_empty());
        }
        // Idle for 10 minutes: away, from the last input.
        for now in (2005..=2000 + AWAY_IDLE_SECS).step_by(5) {
            t.observe([game.as_path()], now);
        }
        let ev = t.presence(look(2000 + AWAY_IDLE_SECS, 2000, None), |_| true);
        assert_eq!(ev[0], Event::Ended(Session { game_id: 2, started_at: 1000, ended_at: 2000 }));
        for now in (2605..=2700).step_by(5) {
            t.observe([game.as_path()], now);
        }
        // Back at the PC: Hearth can't see where the input went, so any input resumes.
        assert_eq!(t.presence(look(2700, 2699, None), |_| true), [Event::Idle(Session { game_id: 2, started_at: 2000, ended_at: 2700 }), Event::Resumed { game_id: 2, at: 2700 }]);
    }

    #[test]
    fn a_launcher_seen_once_never_leaves_the_game_stuck_paused() {
        // The launcher's window (in the game's folder) was in front; the game
        // itself runs in a program elsewhere (javaw.exe, an emulator), so the
        // game has no window of its own any more.
        let mut t = tracker();
        play(&mut t, 1000, 1100);
        let game = rdr2();
        let no_window = |now: i64, input: i64| Presence { windowed: vec![], ..look(now, input, None) };
        // An hour of play with input: never "left behind".
        for now in (1105..=4700).step_by(5) {
            t.observe([game.as_path()], now);
            assert!(t.presence(no_window(now, now), |_| true).is_empty(), "paused at {now}");
        }
        // Idle 10 minutes: away; then any input resumes it.
        for now in (4705..=5400).step_by(5) {
            t.observe([game.as_path()], now);
            if now == 4700 + AWAY_IDLE_SECS {
                assert!(!t.presence(no_window(now, 4700), |_| true).is_empty());
            }
        }
        // A key pressed while it's between two processes (the launcher
        // restarting it) doesn't count: it isn't running at that moment.
        assert!(t.presence(no_window(5402, 5401), |_| true).is_empty());
        assert_eq!(t.presence(no_window(5400, 5399), |_| true), [Event::Idle(Session { game_id: 2, started_at: 4700, ended_at: 5400 }), Event::Resumed { game_id: 2, at: 5400 }]);
    }

    #[test]
    fn away_detection_can_be_off() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        assert!(t.presence(look(5000, 2000, None), |_| false).is_empty());
        assert_eq!(t.sittings(), [Sitting { game_id: 2, started_at: Some(1000), before: 0, paused: None }]);
    }

    #[test]
    fn a_paused_game_that_closes_records_nothing_more() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2000, PauseReason::Manual);
        assert!(t.observe([], 2005).is_empty());
        assert_eq!(t.observe([], 2020), [Event::Closed { game_id: 2 }]);
        assert!(t.sittings().is_empty());
    }

    #[test]
    fn a_pause_survives_a_restart_of_hearth() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2002, PauseReason::Manual);
        // Quitting (or restarting for an update) records nothing more and
        // keeps the pause, to be saved.
        assert!(t.flush().is_empty());
        assert_eq!(t.paused_list(), [(2, 1000, PauseReason::Manual)]);
        // The next Hearth: the game is still running, and stays paused.
        let mut next = tracker();
        next.restore_paused(2, 1000, PauseReason::Manual, 3000);
        let game = rdr2();
        assert!(next.observe([game.as_path()], 3005).is_empty());
        assert!(next.presence(look(3010, 3009, None), |_| true).is_empty());
        assert!(next.presence(look(3015, 3012, Some(2)), |_| true).is_empty());
        assert_eq!(next.presence(look(3020, 3018, Some(2)), |_| true), [Event::Resumed { game_id: 2, at: 3020 }]);
        assert_eq!(next.sittings()[0].before, 1000);
    }

    #[test]
    fn waking_from_sleep_keeps_the_time_on_screen() {
        let mut t = tracker();
        play(&mut t, 1000, 1600);
        let game = rdr2();
        let woke = 1600 + 8 * 3600;
        t.observe([game.as_path()], woke);
        assert_eq!(t.sittings(), [Sitting { game_id: 2, started_at: Some(woke), before: 600, paused: None }]);
    }

    #[test]
    fn idle_time_is_kept_apart_without_the_sleep_in_it() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        let game = rdr2();
        // Left at 2000; away after ten minutes without input, back-dated.
        for now in (2005..=3000).step_by(5) {
            t.observe([game.as_path()], now);
            t.presence(look(now, 2000, None), |_| true);
        }
        assert_eq!(t.sittings()[0].paused, Some(PauseReason::Away));
        // The PC slept from 3000: idle time stops there, and starts again on waking.
        assert_eq!(t.observe([game.as_path()], 6600), [Event::Idle(Session { game_id: 2, started_at: 2000, ended_at: 3000 })]);
        t.observe([game.as_path()], 6700);
        // Closed while still idle: that idle time is recorded too.
        assert_eq!(
            t.observe([], 6720),
            [Event::Idle(Session { game_id: 2, started_at: 6600, ended_at: 6700 }), Event::Closed { game_id: 2 }]
        );
    }

    #[test]
    fn a_pause_you_chose_is_never_idle_time() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2000, PauseReason::Manual);
        for now in (2005..=2500).step_by(5) {
            t.observe([rdr2().as_path()], now);
        }
        assert_eq!(t.stop(2), [Event::Closed { game_id: 2 }]);
    }

    #[test]
    fn a_game_set_to_count_idle_time_as_play_is_never_paused() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        let game = rdr2();
        for now in (2005..=4000).step_by(5) {
            t.observe([game.as_path()], now);
            assert!(t.presence(look(now, 2000, None), |id| id != 2).is_empty());
        }
        assert_eq!(t.sittings()[0].paused, None);
    }

    #[test]
    fn quitting_hearth_records_the_idle_time_so_far() {
        let mut t = tracker();
        play(&mut t, 1000, 2000);
        t.pause(2, 2000, PauseReason::Away);
        for now in (2005..=2300).step_by(5) {
            t.observe([rdr2().as_path()], now);
        }
        assert_eq!(t.flush(), [Event::Idle(Session { game_id: 2, started_at: 2000, ended_at: 2300 })]);
        assert!(t.flush().is_empty());
    }
}
