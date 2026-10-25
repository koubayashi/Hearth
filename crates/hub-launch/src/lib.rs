//! Starting games and store installers.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// How to start something: either a store URI handed to the OS, or a process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LaunchAction {
    /// e.g. `steam://rungameid/1145360`, opened by the registered handler.
    Uri { uri: String },
    Process {
        exe: PathBuf,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("Can't find {what}. It may have been moved or deleted.")]
    Missing { what: String },
    #[error("failed to start {what}: {source}")]
    Spawn {
        what: String,
        source: std::io::Error,
    },
}

pub fn run(action: &LaunchAction) -> Result<(), LaunchError> {
    let (mut cmd, what) = match action {
        LaunchAction::Uri { uri } => {
            // `explorer.exe <uri>` hands the URI to its protocol handler without
            // going through a shell, so nothing in the URI is interpreted.
            let mut c = Command::new("explorer.exe");
            c.arg(uri);
            (c, uri.clone())
        }
        LaunchAction::Process { exe, args, working_dir } => {
            let mut c = Command::new(exe);
            c.args(args);
            if let Some(dir) = working_dir.as_ref().or(exe.parent().map(|p| p.to_path_buf()).as_ref()) {
                c.current_dir(dir);
            }
            (c, exe.display().to_string())
        }
    };
    cmd.spawn().map(drop).map_err(|source| LaunchError::Spawn { what, source })
}

/// Runs a program the user picked as a way to play: an .exe directly, a .bat
/// or .cmd script in its own console window, anything else (a shortcut) as
/// Windows would open it. Arguments go through exactly as typed, as in a
/// shortcut. It starts in `working_dir`, or the program's own folder, since
/// scripts often use paths relative to where they live.
pub fn run_program(path: &Path, args: &str, working_dir: Option<&Path>) -> Result<(), LaunchError> {
    let what = path.display().to_string();
    if !path.is_file() {
        return Err(LaunchError::Missing { what });
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut cmd = match ext.as_str() {
        "exe" | "com" => {
            let mut c = Command::new(path);
            raw_args(&mut c, args);
            c
        }
        "bat" | "cmd" => {
            // /S: strip exactly the outer quotes, so a quoted path with spaces
            // and the arguments after it reach the script as written.
            let mut c = Command::new("cmd.exe");
            raw_args(&mut c, &format!("/S /C \"\"{}\" {}\"", path.display(), args));
            new_console(&mut c);
            c
        }
        // Shortcuts and anything else: Windows opens it with its own settings
        // (a shortcut keeps its own arguments and start folder).
        _ => {
            let mut c = Command::new("explorer.exe");
            c.arg(path);
            c
        }
    };
    if let Some(dir) = working_dir.or(path.parent()) {
        cmd.current_dir(dir);
    }
    cmd.spawn().map(drop).map_err(|source| LaunchError::Spawn { what, source })
}

#[cfg(windows)]
fn raw_args(c: &mut Command, args: &str) {
    use std::os::windows::process::CommandExt;
    if !args.trim().is_empty() {
        c.raw_arg(args);
    }
}

#[cfg(not(windows))]
fn raw_args(c: &mut Command, args: &str) {
    c.args(args.split_whitespace());
}

/// A script gets a console of its own, so its output shows and closing it
/// can't take Hearth down.
#[cfg(windows)]
fn new_console(c: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x10;
    c.creation_flags(CREATE_NEW_CONSOLE);
}

#[cfg(not(windows))]
fn new_console(_c: &mut Command) {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// A script in a folder with spaces gets its arguments as typed and starts
    /// in its own folder (as a server replacement's start script expects).
    #[test]
    fn runs_a_script_with_spaces_and_quoted_arguments() {
        let dir = std::env::temp_dir().join(format!("hearth ways {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("Start Server.cmd");
        std::fs::write(&script, "@echo off\r\n> out.txt echo %~1^|%~2^|%CD%\r\nexit\r\n").unwrap();

        run_program(&script, r#""hello world" second"#, None).unwrap();

        let out = dir.join("out.txt");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !out.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        let text = std::fs::read_to_string(&out).expect("the script ran and wrote its output");
        assert_eq!(text.trim(), format!("hello world|second|{}", dir.display()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_program_is_a_clear_error() {
        let err = run_program(Path::new(r"C:\nowhere\Start Server.cmd"), "", None).unwrap_err();
        assert!(err.to_string().starts_with("Can't find"));
    }
}
