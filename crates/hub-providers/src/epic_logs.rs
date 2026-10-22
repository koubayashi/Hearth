//! Epic's API reports how long you played but never when. The Epic Games
//! Launcher does log it, though: every session writes
//! `Started tracking playtime for <namespace:catalog:app>` and
//! `Stopped tracking playtime for …` lines with UTC timestamps. Reading those
//! gives each game's real last-played date, without signing in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The launcher's log folder for the current Windows user.
pub fn log_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("EpicGamesLauncher").join("Saved").join("Logs"))
}

/// Last played (unix seconds) per `namespace:catalog:app`, from every
/// launcher log in `dir` (the current log and its rotated backups).
pub fn last_played(dir: &Path) -> HashMap<String, i64> {
    let mut out: HashMap<String, i64> = HashMap::new();
    let Ok(read) = std::fs::read_dir(dir) else { return out };
    for f in read.flatten() {
        let name = f.file_name().to_string_lossy().to_lowercase();
        if !(name.starts_with("epicgameslauncher") && name.ends_with(".log")) {
            continue;
        }
        // Logs can hold invalid UTF-8; read lossily.
        let Ok(bytes) = std::fs::read(f.path()) else { continue };
        for (id, at) in parse(&String::from_utf8_lossy(&bytes)) {
            let e = out.entry(id).or_insert(at);
            *e = (*e).max(at);
        }
    }
    out
}

fn parse(text: &str) -> Vec<(String, i64)> {
    text.lines()
        .filter_map(|line| {
            // Either end of a session counts: a game still running has only "Started".
            let id = line
                .split_once("Stopped tracking playtime for ")
                .or_else(|| line.split_once("Started tracking playtime for "))?
                .1
                .trim();
            if id.split(':').count() != 3 {
                return None;
            }
            Some((id.to_string(), timestamp(line)?))
        })
        .collect()
}

/// `[2026.10.20-21.10.30:189]…` → unix seconds (the launcher logs in UTC).
fn timestamp(line: &str) -> Option<i64> {
    let inner = line.strip_prefix('[')?.split(']').next()?;
    let (date, time) = inner.split_once('-')?;
    let mut d = date.split('.').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.split(':').next()?;
    let mut t = time.split('.').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next()??);
    Some(days_from_civil(y, m, day) * 86_400 + h * 3600 + min * 60 + s)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
[2026.10.20-15.42.23:368][414]FCommunityPortalLaunchAppTask: Preparing to launch app 'D:/Games/Epic Games/RedDeadRedemption2/PlayRDR2.exe'
[2026.10.20-15.43.08:332][289]LogPortalPublishTime: Started tracking playtime for b30b6d1b4dfd4dcc93b5490be5e094e5:22a7b503221442daa2fb16ad37b6ccbf:Heather
[2026.10.20-21.10.30:189][307]LogPortalPublishTime: Stopped tracking playtime for b30b6d1b4dfd4dcc93b5490be5e094e5:22a7b503221442daa2fb16ad37b6ccbf:Heather
[2026.10.21-09.00.00:000][  1]LogPortalPublishTime: Started tracking playtime for ns:cat:StillRunning
";

    #[test]
    fn reads_session_times_from_launcher_logs() {
        let got: HashMap<_, _> = parse(LOG).into_iter().fold(HashMap::new(), |mut m, (id, at)| {
            let e = m.entry(id).or_insert(at);
            *e = (*e).max(at);
            m
        });
        // 2026-10-20 21:10:30 UTC
        assert_eq!(got["b30b6d1b4dfd4dcc93b5490be5e094e5:22a7b503221442daa2fb16ad37b6ccbf:Heather"], 1_792_530_630);
        assert_eq!(got["ns:cat:StillRunning"], 1_792_573_200);
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn converts_dates_like_the_calendar() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(timestamp("[2024.02.29-00.00.00:000]x"), Some(1_709_164_800));
    }

    #[test]
    fn scans_current_and_rotated_logs() {
        let dir = std::env::temp_dir().join(format!("hearth-epic-logs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("EpicGamesLauncher-backup-2026.10.15.log"), LOG).unwrap();
        std::fs::write(
            dir.join("EpicGamesLauncher.log"),
            "[2026.10.22-10.00.00:000][1]LogPortalPublishTime: Stopped tracking playtime for b30b6d1b4dfd4dcc93b5490be5e094e5:22a7b503221442daa2fb16ad37b6ccbf:Heather\n",
        )
        .unwrap();
        std::fs::write(dir.join("cef3.log"), LOG).unwrap();
        let got = last_played(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(got["b30b6d1b4dfd4dcc93b5490be5e094e5:22a7b503221442daa2fb16ad37b6ccbf:Heather"], 1_792_663_200);
    }
}
