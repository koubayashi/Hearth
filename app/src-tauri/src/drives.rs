//! Notices drives being connected or removed (an external disk, a USB stick,
//! a network share mapped to a letter) and rescans, so games on that drive
//! switch between "installed" and "drive not connected" without a restart.

use std::time::Duration;

use tauri::{AppHandle, Emitter};

/// How often the set of drive letters is checked. One cheap system call.
const EVERY: Duration = Duration::from_secs(3);

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    /// Bit 0 is A:, bit 1 is B:, and so on, for every drive letter in use.
    fn GetLogicalDrives() -> u32;
}

#[cfg(windows)]
fn drive_letters() -> u32 {
    // SAFETY: takes no arguments and only returns a bitmask.
    unsafe { GetLogicalDrives() }
}

#[cfg(not(windows))]
fn drive_letters() -> u32 {
    0
}

pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = drive_letters();
        loop {
            std::thread::sleep(EVERY);
            let now = drive_letters();
            if now == last {
                continue;
            }
            last = now;
            crate::tracking::log(&app, &format!("drives changed ({now:#x}), rescanning"));
            // The library re-checks which drives are present when it lists
            // games, so the view updates at once; the rescan then picks up
            // games the stores report on a drive that just arrived.
            let _ = app.emit("library-changed", ());
            crate::sync_all(app.clone());
        }
    });
}
