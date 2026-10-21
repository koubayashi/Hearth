//! Starting games and store installers.

use std::path::PathBuf;
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
