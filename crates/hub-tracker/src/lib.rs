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

/// Helper processes that live in game folders but don't mean you're playing.
const IGNORED_EXES: &[&str] = &[
    "unins", "crashreport", "crashhandler", "crashpad", "bugsplat", "vcredist", "vc_redist",
    "dxsetup", "dxwebsetup", "dotnetfx", "ue4prereq", "ueprereq", "easyanticheat_setup", "installer",
    "setup.exe", "redist",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub game_id: i64,
    pub started_at: i64,
    pub ended_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Started { game_id: i64, at: i64 },
    Ended(Session),
}

#[derive(Debug)]
struct Active {
    started_at: i64,
    last_seen: i64,
}

#[derive(Debug, Default)]
pub struct Tracker {
    /// (normalized dir with trailing separator, game id), longest dir first.
    dirs: Vec<(String, i64)>,
    active: HashMap<i64, Active>,
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
        if !file.ends_with(".exe") || IGNORED_EXES.iter().any(|i| file.contains(i)) {
            return None;
        }
        self.dirs.iter().find(|(d, _)| path.starts_with(d.as_str())).map(|(_, id)| *id)
    }

    /// Running sessions as (game id, started, last seen), for checkpoints.
    pub fn active(&self) -> Vec<(i64, i64, i64)> {
        self.active.iter().map(|(id, a)| (*id, a.started_at, a.last_seen)).collect()
    }

    pub fn running(&self) -> Vec<(i64, i64)> {
        self.active.iter().map(|(id, a)| (*id, a.started_at)).collect()
    }

    /// Feed one snapshot of running executables taken at `now` (unix seconds).
    pub fn observe<'a>(&mut self, exes: impl IntoIterator<Item = &'a Path>, now: i64) -> Vec<Event> {
        let mut seen: Vec<i64> = exes.into_iter().filter_map(|e| self.game_for_exe(e)).collect();
        seen.sort_unstable();
        seen.dedup();

        let mut events = Vec::new();
        // A long silence means the PC slept (or Hearth stalled) with the game
        // open: that time wasn't play. The session ends when it was last seen,
        // and one still running starts a new session now.
        let stale: Vec<i64> = self.active.iter().filter(|(_, a)| now - a.last_seen > MAX_GAP_SECS).map(|(id, _)| *id).collect();
        for id in stale {
            events.extend(self.end(id));
        }
        for id in &seen {
            match self.active.get_mut(id) {
                Some(a) => a.last_seen = now,
                None => {
                    self.active.insert(*id, Active { started_at: now, last_seen: now });
                    events.push(Event::Started { game_id: *id, at: now });
                }
            }
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

    /// Close every open session, e.g. when Hearth quits.
    pub fn flush(&mut self) -> Vec<Event> {
        let ids: Vec<i64> = self.active.keys().copied().collect();
        ids.into_iter().filter_map(|id| self.end(id)).collect()
    }

    fn end(&mut self, id: i64) -> Option<Event> {
        let a = self.active.remove(&id)?;
        // The session ends when the game was last seen, not when we noticed.
        let s = Session { game_id: id, started_at: a.started_at, ended_at: a.last_seen };
        (s.ended_at - s.started_at >= MIN_SESSION_SECS).then_some(Event::Ended(s))
    }
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
        assert!(t.running().is_empty());
    }
}
