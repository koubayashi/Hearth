//! Errors Hearth couldn't handle, written to `errors.log` in the app data
//! folder: panics on the Rust side, and errors the interface reports (a
//! screen that failed to draw, an unhandled promise). The crash screen opens
//! this folder so the log can be read or sent along with a bug report.

use std::io::Write;
use std::path::{Path, PathBuf};

use hub_launch::LaunchAction;
use tauri::{AppHandle, Manager};

use crate::{now, CmdResult};

const FILE: &str = "errors.log";
/// Start over once the log passes this size.
const MAX_LEN: u64 = 1_000_000;
/// One report can't fill the log (a stack trace is a few KB).
const MAX_REPORT: usize = 16_000;

fn append(dir: &Path, msg: &str) {
    let path = dir.join(FILE);
    if std::fs::metadata(&path).map(|m| m.len() > MAX_LEN).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {msg}", now());
    }
}

/// Panics on any thread (a sync task, the tracker) are logged before the
/// default hook runs; otherwise they vanish with the console.
pub fn install_panic_hook(dir: PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let backtrace = std::backtrace::Backtrace::force_capture();
        append(&dir, &format!("panic on thread {name}: {info}\n{backtrace}"));
        default(info);
    }));
}

#[tauri::command]
pub fn report_error(app: AppHandle, message: String) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    let mut message = message;
    if message.len() > MAX_REPORT {
        let mut end = MAX_REPORT;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
    }
    append(&dir, &format!("interface: {message}"));
}

#[tauri::command]
pub fn open_error_log(app: AppHandle) -> CmdResult<()> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    hub_launch::run(&LaunchAction::Process {
        exe: "explorer.exe".into(),
        args: vec![dir.to_string_lossy().into_owned()],
        working_dir: None,
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_over_past_the_limit() {
        let dir = std::env::temp_dir().join(format!("hearth-errors-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), vec![b'x'; MAX_LEN as usize + 1]).unwrap();
        append(&dir, "fresh");
        let log = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(log.ends_with(" fresh\n") && log.len() < 100);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
