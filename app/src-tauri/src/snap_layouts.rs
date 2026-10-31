//! Snap Layouts on the Maximize button.
//!
//! Windows shows the layouts flyout when the window under the mouse answers
//! "maximize button" (HTMAXBUTTON) to its hit test. Hearth's buttons are part
//! of the page, and the page's window belongs to WebView2, so Windows never
//! asks Hearth. As in Windows Terminal, an invisible window of Hearth's own
//! sits over the page's Maximize button and gives that answer. It also does
//! the button's job: clicking it maximizes or restores, and hovering and
//! pressing are sent to the page (`maximize-button`) so the button still
//! lights up.
//!
//! The page says where the button is, or that something covers it (a dialog,
//! the Console view), with `place_maximize_button`.

#[cfg(windows)]
mod imp {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::OnceLock;

    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager};

    use super::Place;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetAncestor, IsZoomed, RegisterClassExW, SetLayeredWindowAttributes,
        SetWindowPos, ShowWindow, GA_ROOT, HTMAXBUTTON, HWND_TOP, LWA_ALPHA, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        SW_HIDE, SW_MAXIMIZE, SW_RESTORE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP,
        WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED,
        WS_EX_NOREDIRECTIONBITMAP,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    /// The invisible window, once made (0 until then).
    static SINK: AtomicIsize = AtomicIsize::new(0);

    thread_local! {
        static HOVER: Cell<bool> = const { Cell::new(false) };
        static PRESSED: Cell<bool> = const { Cell::new(false) };
    }

    #[derive(Clone, Serialize)]
    struct ButtonState {
        hover: bool,
        pressed: bool,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn set(hover: bool, pressed: bool) {
        if HOVER.replace(hover) == hover && PRESSED.replace(pressed) == pressed {
            return;
        }
        if let Some(app) = APP.get() {
            let _ = app.emit("maximize-button", ButtonState { hover, pressed });
        }
    }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_NCHITTEST => HTMAXBUTTON as LRESULT,
            WM_NCMOUSEMOVE => {
                if !HOVER.get() {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    // SAFETY: a valid struct for a live window of this thread.
                    unsafe { TrackMouseEvent(&mut track) };
                }
                set(true, PRESSED.get());
                0
            }
            WM_NCMOUSELEAVE => {
                set(false, false);
                0
            }
            // Handled here: the default would treat this child as having a
            // caption of its own.
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                set(true, true);
                0
            }
            WM_NCLBUTTONUP => {
                if PRESSED.get() {
                    set(true, false);
                    // SAFETY: the window's own top-level window, alive while it is.
                    unsafe {
                        let top = GetAncestor(hwnd, GA_ROOT);
                        ShowWindow(top, if IsZoomed(top) != 0 { SW_RESTORE } else { SW_MAXIMIZE });
                    }
                }
                0
            }
            // SAFETY: the arguments Windows passed in.
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    pub fn install(window: &tauri::WebviewWindow) {
        let _ = APP.set(window.app_handle().clone());
        let Ok(parent) = window.hwnd() else { return };
        let parent = parent.0 as isize;
        // Its messages have to arrive on the thread that owns the window.
        let _ = window.run_on_main_thread(move || {
            let class = wide("HearthMaximizeButton");
            // SAFETY: plain Win32 calls with valid, live arguments. Registering
            // the class again fails harmlessly.
            unsafe {
                let instance = GetModuleHandleW(std::ptr::null());
                let mut wc: WNDCLASSEXW = std::mem::zeroed();
                wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
                wc.lpfnWndProc = Some(proc);
                wc.hInstance = instance;
                wc.lpszClassName = class.as_ptr();
                RegisterClassExW(&wc);
                // Layered with no redirection bitmap: never drawn, so the page
                // shows through, yet it still takes the mouse.
                let sink = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_CHILD | WS_CLIPSIBLINGS,
                    0,
                    0,
                    0,
                    0,
                    parent as HWND,
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                );
                if sink.is_null() {
                    return;
                }
                SetLayeredWindowAttributes(sink, 0, 255, LWA_ALPHA);
                SINK.store(sink as isize, Ordering::Relaxed);
            }
        });
    }

    pub fn place(window: &tauri::WebviewWindow, place: Option<Place>) {
        let sink = SINK.load(Ordering::Relaxed);
        if sink == 0 {
            return;
        }
        let _ = window.run_on_main_thread(move || {
            let sink = sink as HWND;
            // SAFETY: the window made in `install`, on its own thread.
            unsafe {
                match place {
                    // Above the page's window every time: WebView2 may have
                    // put its own back on top.
                    Some(p) => {
                        SetWindowPos(
                            sink,
                            HWND_TOP,
                            p.x.round() as i32,
                            p.y.round() as i32,
                            p.width.round() as i32,
                            p.height.round() as i32,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                    None => {
                        ShowWindow(sink, SW_HIDE);
                        set(false, false);
                    }
                }
            }
        });
    }
}

#[cfg(windows)]
pub use imp::install;

#[cfg(not(windows))]
pub fn install(_window: &tauri::WebviewWindow) {}

/// The button's place in the page, in physical pixels.
#[derive(serde::Deserialize)]
pub struct Place {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Where the page's Maximize button is, or None while something covers it.
#[tauri::command]
pub fn place_maximize_button(window: tauri::WebviewWindow, place: Option<Place>) {
    #[cfg(windows)]
    imp::place(&window, place);
    #[cfg(not(windows))]
    let _ = (window, place);
}
