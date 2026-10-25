//! Full screen that really fills the screen, from a maximized window too.
//!
//! The window has no Windows frame, so tao (Tauri's windowing layer) sizes its
//! content itself, and for a maximized window it keeps the content above the
//! taskbar. It does that in full screen as well: the window covers the whole
//! monitor but the page stops at the taskbar's edge, leaving a bare strip
//! (tao 0.37, the WM_NCCALCSIZE handler doesn't check for full screen).
//!
//! The fix sits in front of tao's window procedure and answers that one
//! question when the window covers its monitor exactly: then the content
//! covers it too. Every other message goes on to tao unchanged.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;
    type SubclassProc = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;

    #[repr(C)]
    #[derive(Default, Clone, Copy, PartialEq, Eq)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    /// NCCALCSIZE_PARAMS: the first rectangle is the proposed window rectangle
    /// on the way in, and the content rectangle on the way out.
    #[repr(C)]
    struct CalcSizeParams {
        rects: [Rect; 3],
        pos: *mut c_void,
    }

    const WM_NCCALCSIZE: u32 = 0x0083;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    /// "HRTH": tells this subclass apart from tao's own.
    const SUBCLASS_ID: usize = 0x4852_5448;

    #[link(name = "user32")]
    extern "system" {
        fn IsZoomed(hwnd: Hwnd) -> i32;
        fn MonitorFromRect(rect: *const Rect, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }

    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(hwnd: Hwnd, proc: SubclassProc, id: usize, data: usize) -> i32;
        fn DefSubclassProc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
    }

    unsafe extern "system" fn subclass(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, _: usize, _: usize) -> isize {
        // SAFETY: for WM_NCCALCSIZE with wparam set, lparam points at a valid
        // NCCALCSIZE_PARAMS for the duration of the call.
        if msg == WM_NCCALCSIZE && wparam != 0 && unsafe { IsZoomed(hwnd) } != 0 {
            let params = unsafe { &*(lparam as *const CalcSizeParams) };
            let proposed = params.rects[0];
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            let monitor = unsafe { MonitorFromRect(&proposed, MONITOR_DEFAULTTONEAREST) };
            // Maximized and exactly the monitor's size: that's full screen. (A
            // plain maximized window overhangs the screen by its resize border.)
            // Returning 0 leaves the content rectangle equal to the window's.
            if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 && proposed == info.monitor {
                return 0;
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    pub fn install(window: &tauri::WebviewWindow) {
        let Ok(hwnd) = window.hwnd() else { return };
        let hwnd = hwnd.0 as usize;
        // A subclass has to be added on the thread that owns the window.
        let _ = window.run_on_main_thread(move || {
            // SAFETY: a live window owned by this thread; adding the same id
            // again (the window recreated) only replaces it.
            unsafe { SetWindowSubclass(hwnd as Hwnd, subclass, SUBCLASS_ID, 0) };
        });
    }
}

#[cfg(windows)]
pub use imp::install;

#[cfg(not(windows))]
pub fn install(_window: &tauri::WebviewWindow) {}
