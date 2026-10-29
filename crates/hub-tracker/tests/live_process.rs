//! End to end on a real process: a copied system exe stands in for a game
//! started straight from its folder.
#![cfg(windows)]

use std::process::Command;

use hub_tracker::{Event, ProcessScanner, Tracker};

#[test]
fn detects_a_game_started_from_its_folder() {
    let dir = std::env::temp_dir().join(format!("hearth-fake-game-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("FakeGame.exe");
    std::fs::copy(r"C:\Windows\System32\PING.EXE", &exe).unwrap();

    let mut tracker = Tracker::new();
    tracker.set_install_dirs([(42, dir.to_string_lossy().into_owned())]);
    let mut scanner = ProcessScanner::new();

    let mut child = Command::new(&exe).args(["-n", "30", "127.0.0.1"]).spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    let started = tracker.observe(scanner.exes().iter().map(|p| p.as_path()), 1_000);
    assert_eq!(started, [Event::Started { game_id: 42, at: 1_000 }]);
    assert!(tracker.observe(scanner.exes().iter().map(|p| p.as_path()), 1_100).is_empty());

    child.kill().unwrap();
    child.wait().unwrap();
    let ended = tracker.observe(scanner.exes().iter().map(|p| p.as_path()), 1_200);
    std::fs::remove_dir_all(&dir).ok();

    match ended.as_slice() {
        [Event::Ended(s)] => assert_eq!((s.game_id, s.started_at, s.ended_at), (42, 1_000, 1_100)),
        other => panic!("expected one ended session, got {other:?}"),
    }
}
