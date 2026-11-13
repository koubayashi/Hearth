//! What the tracker needs to tell "playing" from "stepped away": when the
//! last keyboard, mouse or controller input was, which program's window is
//! in front, and which games have a window of their own (and where). Only
//! times and programs' paths are read, never what was pressed, and nothing
//! touches the game itself.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Last controller input, in Windows' milliseconds-since-boot (the same
/// clock as keyboard and mouse input, which doesn't jump when the time of
/// day is changed). 0: none yet.
static PAD_INPUT: AtomicU64 = AtomicU64::new(0);
/// A game is being timed or paused: controllers are watched closely only then.
static WATCHING: AtomicBool = AtomicBool::new(false);

pub fn watch(on: bool) {
    WATCHING.store(on, Ordering::Relaxed);
}

/// Last keyboard, mouse or controller input, in unix seconds.
#[cfg(windows)]
pub fn last_input(now: i64) -> i64 {
    use windows_sys::Win32::System::SystemInformation::GetTickCount64;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    // SAFETY: plain query.
    let tick = unsafe { GetTickCount64() };
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: plain out-parameter with its size set.
    let keys_ago = if unsafe { GetLastInputInfo(&mut info) } == 0 {
        0
    } else {
        // The low 32 bits of the same tick count, wrapping every 49 days.
        u64::from((tick as u32).wrapping_sub(info.dwTime))
    };
    let pad = PAD_INPUT.load(Ordering::Relaxed);
    let pad_ago = if pad == 0 { u64::MAX } else { tick.saturating_sub(pad) };
    now - (keys_ago.min(pad_ago) / 1000).min(i64::MAX as u64) as i64
}

#[cfg(not(windows))]
pub fn last_input(now: i64) -> i64 {
    now
}

/// The program of the window in front, then the programs that started it
/// (a game's window may belong to javaw.exe or an emulator that a launcher
/// in the game's folder started). Empty for Hearth's own windows.
#[cfg(windows)]
pub fn front_exes() -> Vec<PathBuf> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    // SAFETY: the window handle is only passed back to Windows.
    let pid = unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return Vec::new();
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid
    };
    if pid == 0 || pid == std::process::id() {
        return Vec::new();
    }
    let parents = parents();
    let mut chain = vec![pid];
    while chain.len() < 4 {
        match parents.iter().find(|(p, _)| *p == *chain.last().unwrap()) {
            Some((_, parent)) if *parent != 0 && !chain.contains(parent) => chain.push(*parent),
            _ => break,
        }
    }
    chain.into_iter().filter_map(exe_of).collect()
}

#[cfg(not(windows))]
pub fn front_exes() -> Vec<PathBuf> {
    Vec::new()
}

/// Every process as (pid, parent pid).
#[cfg(windows)]
fn parents() -> Vec<(u32, u32)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    let mut out = Vec::new();
    // SAFETY: the snapshot handle is checked and closed; the entry has its size set.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut e) != 0 {
            loop {
                out.push((e.th32ProcessID, e.th32ParentProcessID));
                if Process32NextW(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    out
}

/// The programs with windows, for away detection.
#[derive(Default)]
pub struct Windows {
    /// Programs with a window of their own, shown or minimised.
    pub windowed: Vec<PathBuf>,
    /// Programs with a window showing on another screen than the window in
    /// front: a game left on a second monitor while you're in Discord.
    pub elsewhere: Vec<PathBuf>,
}

#[cfg(windows)]
pub fn windows() -> Windows {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONULL};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible};

    struct Seen {
        front_monitor: isize,
        windowed: Vec<u32>,
        elsewhere: Vec<u32>,
    }
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the `Seen` passed to EnumWindows below, alive for the call.
        let seen = unsafe { &mut *(data as *mut Seen) };
        unsafe {
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            // On another virtual desktop, or a suspended Store app: not shown.
            let mut cloaked = 0u32;
            if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut _, 4) == 0 && cloaked != 0 {
                return 1;
            }
            let minimised = IsIconic(hwnd) != 0;
            let mut r: RECT = std::mem::zeroed();
            if !minimised && (GetWindowRect(hwnd, &mut r) == 0 || r.right - r.left < 200 || r.bottom - r.top < 150) {
                return 1;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return 1;
            }
            if !seen.windowed.contains(&pid) {
                seen.windowed.push(pid);
            }
            if !minimised {
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) as isize;
                if monitor != 0 && monitor != seen.front_monitor && !seen.elsewhere.contains(&pid) {
                    seen.elsewhere.push(pid);
                }
            }
        }
        1
    }
    // SAFETY: the callback only touches `seen` while EnumWindows runs.
    unsafe {
        let front = GetForegroundWindow();
        let front_monitor = if front.is_null() { 0 } else { MonitorFromWindow(front, MONITOR_DEFAULTTONULL) as isize };
        let mut seen = Seen { front_monitor, windowed: Vec::new(), elsewhere: Vec::new() };
        EnumWindows(Some(each), &mut seen as *mut Seen as LPARAM);
        // Nothing in front (the desktop): no screen to be "another" one.
        if front_monitor == 0 {
            seen.elsewhere.clear();
        }
        Windows {
            windowed: seen.windowed.into_iter().filter_map(exe_of).collect(),
            elsewhere: seen.elsewhere.into_iter().filter_map(exe_of).collect(),
        }
    }
}

#[cfg(not(windows))]
pub fn windows() -> Windows {
    Windows::default()
}

#[cfg(windows)]
fn exe_of(pid: u32) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: the handle is checked before use and closed after.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(process);
        ok.then(|| PathBuf::from(std::ffi::OsString::from_wide(&buf[..len as usize])))
    }
}

/// Watches controllers on a thread of its own: four times a second while a
/// game is timed, otherwise only a glance every few seconds. An empty slot is
/// asked again only every 3 seconds (asking an empty slot is slow). A stick
/// resting off-centre or drifting isn't input: only presses and real
/// movement count.
pub fn start() {
    #[cfg(windows)]
    std::thread::Builder::new()
        .name("controller-input".into())
        .spawn(|| {
            use std::time::Instant;
            let mut last: [Option<Pad>; 4] = [None; 4];
            let mut retry: [Option<Instant>; 4] = [None; 4];
            loop {
                if !WATCHING.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
                for slot in 0..4 {
                    if retry[slot].is_some_and(|t| t.elapsed() < Duration::from_secs(3)) {
                        continue;
                    }
                    let now = pad_state(slot as u32);
                    retry[slot] = if now.is_none() { Some(Instant::now()) } else { None };
                    if let (Some(a), Some(b)) = (last[slot], now) {
                        if a.moved_from(&b) {
                            // SAFETY: plain query.
                            PAD_INPUT.store(unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }, Ordering::Relaxed);
                        }
                    }
                    // Sticks are compared with where they were when input was
                    // last noticed, so slow drift never adds up to "movement".
                    if now.is_none() || last[slot].is_none() || now.zip(last[slot]).is_some_and(|(b, a)| a.moved_from(&b)) {
                        last[slot] = now;
                    }
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        })
        .expect("start controller watcher");
}

#[derive(Clone, Copy)]
struct Pad {
    buttons: u16,
    triggers: [bool; 2],
    sticks: [i32; 4],
}

impl Pad {
    /// A button or trigger changed, or a stick moved by a quarter of its range.
    fn moved_from(&self, other: &Pad) -> bool {
        self.buttons != other.buttons
            || self.triggers != other.triggers
            || self.sticks.iter().zip(other.sticks).any(|(a, b)| (a - b).abs() > 8192)
    }
}

/// A controller's state, sticks inside the standard dead zone read as
/// centred; None if no controller is in that slot.
#[cfg(windows)]
fn pad_state(slot: u32) -> Option<Pad> {
    use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
    let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
    // SAFETY: out-parameter of the documented size.
    if unsafe { XInputGetState(slot, &mut state) } != 0 {
        return None;
    }
    let g = state.Gamepad;
    let stick = |v: i16| -> i32 {
        let v = i32::from(v);
        if v.abs() < 7849 { 0 } else { v }
    };
    Some(Pad {
        buttons: g.wButtons,
        triggers: [g.bLeftTrigger > 30, g.bRightTrigger > 30],
        sticks: [stick(g.sThumbLX), stick(g.sThumbLY), stick(g.sThumbRX), stick(g.sThumbRY)],
    })
}
