use std::path::PathBuf;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Lists running executables. Only paths are read, so a scan stays cheap
/// enough to run every few seconds from the tray.
pub struct ProcessScanner {
    sys: System,
}

impl ProcessScanner {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    pub fn exes(&mut self) -> Vec<PathBuf> {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        self.sys
            .processes()
            .values()
            .filter_map(|p| p.exe().map(|e| e.to_path_buf()))
            .collect()
    }
}

impl Default for ProcessScanner {
    fn default() -> Self {
        Self::new()
    }
}
