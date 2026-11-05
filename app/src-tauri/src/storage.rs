//! How full each drive is, for the Storage page. The page works out which
//! games sit on which drive from their install folders; this only reports the
//! drives themselves: name, size and free space.

use serde::Serialize;

#[derive(Serialize)]
pub struct DriveSpace {
    /// "C:" for a lettered drive, "\\server\share" for a network path.
    pub root: String,
    /// The volume's name ("Games", "Windows"), when it has one.
    pub label: Option<String>,
    pub total: u64,
    pub free: u64,
}

/// Every local drive (fixed or removable), plus the network roots the page
/// asks for because games are installed there. A root that can't be read
/// (a disconnected drive) is left out; the page shows it as not connected.
#[tauri::command]
pub async fn storage_drives(roots: Vec<String>) -> Vec<DriveSpace> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut all = local_drives();
        for r in roots {
            if !all.iter().any(|d: &String| d.eq_ignore_ascii_case(&r)) {
                all.push(r);
            }
        }
        all.into_iter().filter_map(|r| space(&r)).collect()
    })
    .await
    .unwrap_or_default()
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// The API wants a trailing backslash on a root: "C:\", "\\server\share\".
#[cfg(windows)]
fn api_root(root: &str) -> String {
    format!("{}\\", root.trim_end_matches('\\'))
}

#[cfg(windows)]
fn local_drives() -> Vec<String> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    // SAFETY: takes no arguments and only returns a bitmask.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:", (b'A' + i) as char))
        .filter(|r| {
            let w = wide(&api_root(r));
            // SAFETY: `w` is a NUL-terminated UTF-16 string that outlives the call.
            let t = unsafe { GetDriveTypeW(w.as_ptr()) };
            t == DRIVE_FIXED || t == DRIVE_REMOVABLE
        })
        .collect()
}

#[cfg(windows)]
fn space(root: &str) -> Option<DriveSpace> {
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumeInformationW};
    let w = wide(&api_root(root));
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `w` is NUL-terminated and the out-pointers are valid locals.
    // `avail` is what this user may write, which is what "free" means here.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) };
    // An empty card reader slot reports success with a zero size.
    if ok == 0 || total == 0 {
        return None;
    }
    let mut name = [0u16; 261];
    // SAFETY: the name buffer's length is passed; the other outputs are optional and null.
    let named = unsafe {
        GetVolumeInformationW(
            w.as_ptr(),
            name.as_mut_ptr(),
            name.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    let label = (named != 0)
        .then(|| {
            let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            String::from_utf16_lossy(&name[..end]).trim().to_string()
        })
        .filter(|s| !s.is_empty());
    Some(DriveSpace { root: root.to_string(), label, total, free: avail })
}

#[cfg(not(windows))]
fn local_drives() -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
fn space(_root: &str) -> Option<DriveSpace> {
    None
}
