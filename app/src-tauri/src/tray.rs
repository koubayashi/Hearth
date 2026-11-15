//! System tray: Hearth keeps running here when its window is closed, so the
//! playtime tracker never stops.
//!
//! Right-clicking the icon opens Hearth's own small menu window (the plain
//! Windows menu can't take Hearth's look): the update when one waits, the
//! game being timed with Pause, recent games, Open and Quit. It's the same
//! interface as the main window, loaded as `index.html?tray`, made the first
//! time it's opened and kept, hidden, after that so it opens at once.
//!
//! It behaves like a Windows menu, not a window: it opens at the pointer,
//! never takes focus (so the hidden-icons panel it was opened from stays
//! open, as it does for every other app's menu), closes on a click anywhere
//! else or a switch of window, and while open takes the menu keys (arrows,
//! Enter, Esc, Home, End) the way a Windows menu does.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::lock;
use crate::tracking::{self, NowPlaying};

const TRAY_ID: &str = "hearth";
const MENU: &str = "tray";
/// The menu's width in logical pixels; its height follows its contents.
const MENU_WIDTH: f64 = 324.0;

/// Where the pointer was when the menu was asked for (physical pixels).
static AT: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// When the menu last closed on a click outside it. Right-clicking the icon
/// of an open menu closes it on the press; the release that follows mustn't
/// open it again.
static CLOSED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// The menu is showing: its watcher and key handling run only then.
static OPEN: AtomicBool = AtomicBool::new(false);

/// Shows the main window, loading the interface again if it was unloaded
/// when the window was closed to the tray.
pub fn show_main(app: &AppHandle) {
    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => {
            let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main") else { return };
            match tauri::WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()) {
                Ok(w) => {
                    style_main(&w);
                    w
                }
                Err(_) => return,
            }
        }
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Mica needs Windows 11; on Windows 10 the CSS background shows instead.
pub fn style_main(window: &tauri::WebviewWindow) {
    crate::auth::forget_shared_cookies(window);
    crate::snap_layouts::install(window);
    crate::webview_lock::apply(window);
    crate::rescan::watch_focus(window);
    #[cfg(target_os = "windows")]
    let _ = window_vibrancy::apply_mica(window, None);
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// The tray's own icon: the app icon's flame without its tile, so it fills
/// the square like other tray icons do (tray-dark.svg, tray-light.svg). Cream
/// on a dark taskbar, charcoal on a light one.
fn tray_icon() -> Image<'static> {
    if crate::jumplist::light_taskbar() {
        tauri::include_image!("icons/tray-light.png")
    } else {
        tauri::include_image!("icons/tray-dark.png")
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon())
        .tooltip("Hearth")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event else { return };
            let app = tray.app_handle();
            match button {
                MouseButton::Left => {
                    hide_menu(app);
                    show_main(app);
                }
                MouseButton::Right => toggle_menu(app, (position.x, position.y)),
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

fn toggle_menu(app: &AppHandle, at: (f64, f64)) {
    if let Some(w) = app.get_webview_window(MENU) {
        if shown(&w) {
            hide_menu(app);
            return;
        }
    }
    if lock(&CLOSED_AT).is_some_and(|t| t.elapsed() < Duration::from_millis(400)) {
        return;
    }
    *lock(&AT) = Some(at);
    match app.get_webview_window(MENU) {
        // It refreshes, measures itself and calls `tray_menu_fit`, which
        // places and shows it.
        Some(w) => {
            let _ = w.emit("tray-open", ());
        }
        None => {
            if let Err(e) = build_menu(app) {
                tracking::log(app, &format!("tray: couldn't open the menu ({e})"));
                show_main(app);
            }
        }
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, MENU, WebviewUrl::App("index.html?tray".into()))
        .title("Hearth")
        .inner_size(MENU_WIDTH, 360.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        // No Tauri shadow: it keeps a 1px window border along the top, which
        // Windows paints white on a window that never becomes active. The
        // rounded corners (menu_win) bring Windows' own shadow instead.
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()?;
    crate::webview_lock::apply(&w);
    // No Windows acrylic: its flyout material draws a light rim around the
    // window that no setting removes. The menu is solid, in Hearth's colours.
    #[cfg(windows)]
    menu_win::never_activate(&w);
    Ok(())
}

fn shown(w: &tauri::WebviewWindow) -> bool {
    #[cfg(windows)]
    return menu_win::is_shown(w);
    #[cfg(not(windows))]
    w.is_visible().unwrap_or(false)
}

fn hide_menu(app: &AppHandle) {
    OPEN.store(false, Ordering::Relaxed);
    #[cfg(windows)]
    menu_win::release_keys(app);
    if let Some(w) = app.get_webview_window(MENU) {
        #[cfg(windows)]
        menu_win::hide(&w);
        #[cfg(not(windows))]
        let _ = w.hide();
    }
}

/// The menu has drawn itself `height` logical pixels tall: size it and place
/// it at the pointer the way Windows places its menus (above and to the right
/// of it, flipped where the screen ends), then show it without taking focus.
#[tauri::command]
pub fn tray_menu_fit(app: AppHandle, height: f64, show: bool) {
    let Some(w) = app.get_webview_window(MENU) else { return };
    // Contents changed while hidden: it's measured again when next opened.
    if !show && !shown(&w) {
        return;
    }
    let Some((px, py)) = *lock(&AT) else { return };
    let Some(monitor) = app.monitor_from_point(px, py).ok().flatten().or_else(|| app.primary_monitor().ok().flatten()) else { return };
    let scale = monitor.scale_factor();
    // Kept inside the work area, as Windows' menus are: clicked on the taskbar
    // itself (not in the hidden-icons panel), the pointer is over the taskbar.
    let area = monitor.work_area();
    let (left, top) = (f64::from(area.position.x), f64::from(area.position.y));
    let (right, bottom) = (left + f64::from(area.size.width), top + f64::from(area.size.height));
    let (mw, mh) = (MENU_WIDTH * scale, (height * scale).min(bottom - top));
    let x = if px + mw <= right { px } else { px - mw };
    let y = if py - mh >= top { py - mh } else { py };
    let (x, y) = (x.clamp(left, (right - mw).max(left)), y.clamp(top, (bottom - mh).max(top)));
    // Moved first, then sized: moving onto a screen with other scaling makes
    // Windows rescale the window, and the size set after is the one that holds.
    let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    let _ = w.set_size(PhysicalSize::new(mw.round() as u32, mh.round() as u32));
    if show && !OPEN.swap(true, Ordering::Relaxed) {
        #[cfg(windows)]
        {
            menu_win::show_without_focus(&w);
            menu_win::take_keys(&app);
            menu_win::watch_clicks(app.clone());
        }
        #[cfg(not(windows))]
        let _ = w.show();
    }
}

/// The Windows side of behaving like a menu.
#[cfg(windows)]
mod menu_win {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_RETURN, VK_UP};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx,
        GWL_EXSTYLE, HWND_TOPMOST, KBDLLHOOKSTRUCT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    use super::{hide_menu, CLOSED_AT, MENU, OPEN};
    use crate::lock;

    static HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static WATCHER: AtomicU64 = AtomicU64::new(0);
    static APP: OnceLock<AppHandle> = OnceLock::new();

    fn hwnd(w: &WebviewWindow) -> Option<*mut c_void> {
        w.hwnd().ok().map(|h| h.0)
    }

    /// Clicks reach it without making it the active window.
    pub fn never_activate(w: &WebviewWindow) {
        let Some(h) = hwnd(w) else { return };
        // SAFETY: the menu's own window, still alive.
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize);
            // Rounded like Windows' own menus, with no system border: the
            // page draws its own, in Hearth's colours, along the same curve.
            use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
            let round = DWMWCP_ROUND;
            DwmSetWindowAttribute(h, DWMWA_WINDOW_CORNER_PREFERENCE as u32, (&round as *const i32).cast(), 4);
            let none: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
            DwmSetWindowAttribute(h, DWMWA_BORDER_COLOR as u32, (&none as *const u32).cast(), 4);
        }
    }

    /// Shown and hidden through Windows directly: Tauri doesn't know the
    /// window was shown (it never took focus), so its own hide does nothing.
    pub fn hide(w: &WebviewWindow) {
        if let Some(h) = hwnd(w) {
            // SAFETY: the menu's own window.
            unsafe { ShowWindow(h, SW_HIDE) };
        }
    }

    pub fn is_shown(w: &WebviewWindow) -> bool {
        // SAFETY: as above.
        hwnd(w).is_some_and(|h| unsafe { IsWindowVisible(h) } != 0)
    }

    pub fn show_without_focus(w: &WebviewWindow) {
        let Some(h) = hwnd(w) else {
            let _ = w.show();
            return;
        };
        // SAFETY: as above.
        unsafe {
            ShowWindow(h, SW_SHOWNOACTIVATE);
            SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
    }

    /// Closes the menu on a click anywhere outside it, or when another window
    /// becomes active (Alt+Tab, a notification clicked).
    pub fn watch_clicks(app: AppHandle) {
        let Some(h) = app.get_webview_window(MENU).and_then(|w| hwnd(&w)) else {
            // No window to watch: don't leave the menu keys taken.
            hide_menu(&app);
            return;
        };
        let h = h as usize;
        // Each opening has its own watcher; one left from an earlier opening
        // (closed and reopened within a few milliseconds) stops at once.
        let generation = WATCHER.fetch_add(1, Ordering::Relaxed) + 1;
        std::thread::spawn(move || {
            let down = || unsafe { [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|&b| GetAsyncKeyState(b as i32) as u16 & 0x8000 != 0) };
            let front = unsafe { GetForegroundWindow() } as usize;
            let mut was_down = down();
            while OPEN.load(Ordering::Relaxed) && WATCHER.load(Ordering::Relaxed) == generation {
                std::thread::sleep(Duration::from_millis(30));
                let now_down = down();
                let pressed = now_down && !was_down;
                was_down = now_down;
                // SAFETY: plain queries; `h` is the menu, which outlives this.
                let outside = pressed && unsafe {
                    let mut r: RECT = std::mem::zeroed();
                    let mut p: POINT = std::mem::zeroed();
                    GetWindowRect(h as *mut c_void, &mut r);
                    GetCursorPos(&mut p);
                    p.x < r.left || p.x >= r.right || p.y < r.top || p.y >= r.bottom
                };
                // Another window became active; the menu itself doesn't count,
                // should a click inside it ever activate it.
                let fg = unsafe { GetForegroundWindow() } as usize;
                let switched = fg != front && fg != h;
                if outside || switched {
                    *lock(&CLOSED_AT) = Some(Instant::now());
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || hide_menu(&handle));
                    break;
                }
            }
        });
    }

    /// The menu keys, while it's open, as a Windows menu takes them; every
    /// other key goes where it would anyway.
    unsafe extern "system" fn on_key(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            // SAFETY: for WH_KEYBOARD_LL, `l` points at a KBDLLHOOKSTRUCT.
            let vk = unsafe { (*(l as *const KBDLLHOOKSTRUCT)).vkCode } as u16;
            let name = match vk {
                VK_UP => "ArrowUp",
                VK_DOWN => "ArrowDown",
                VK_RETURN => "Enter",
                VK_ESCAPE => "Escape",
                VK_HOME => "Home",
                VK_END => "End",
                _ => "",
            };
            let msg = w as u32;
            if !name.is_empty() {
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    if let Some(app) = APP.get() {
                        let _ = app.emit_to(MENU, "tray-key", name);
                    }
                }
                if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
                    return 1;
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
    }

    pub fn take_keys(app: &AppHandle) {
        let _ = APP.set(app.clone());
        // Installed on the main thread, whose message loop calls it.
        let _ = app.run_on_main_thread(|| {
            if !HOOK.load(Ordering::Relaxed).is_null() {
                return;
            }
            // SAFETY: a low-level hook with a valid callback; removed on close.
            let h = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_key), GetModuleHandleW(std::ptr::null()), 0) };
            HOOK.store(h, Ordering::Relaxed);
        });
    }

    pub fn release_keys(app: &AppHandle) {
        let _ = app.run_on_main_thread(|| {
            let h = HOOK.swap(std::ptr::null_mut(), Ordering::Relaxed);
            if !h.is_null() {
                // SAFETY: the hook installed by `take_keys`.
                unsafe { UnhookWindowsHookEx(h) };
            }
        });
    }
}

#[tauri::command]
pub fn tray_menu_hide(app: AppHandle) {
    hide_menu(&app);
}

// Async on purpose: it may build the main window, and building a window inside
// a synchronous command deadlocks the main thread on Windows.
#[tauri::command]
pub async fn open_hearth(app: AppHandle) {
    hide_menu(&app);
    show_main(&app);
}

#[tauri::command]
pub fn quit_hearth(app: AppHandle) {
    // Timing stops on the way out (`RunEvent::Exit`), once.
    app.exit(0);
}

/// The tooltip names the game being timed, and the icon carries a pause mark
/// while one is paused, so a pause you'd forgotten shows at a glance.
pub fn set_playing(app: &AppHandle, playing: &[NowPlaying]) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let paused: Vec<&NowPlaying> = playing.iter().filter(|p| p.paused.is_some()).collect();
    let tip = match playing {
        [] => "Hearth".to_string(),
        [one] if one.paused.is_some() => format!("Hearth · {} paused", one.title),
        [one] => format!("Hearth · timing {}", one.title),
        many => format!("Hearth · timing {} games", many.len()),
    };
    // Only redrawn when the tooltip, the pause mark or the taskbar's theme changes.
    static SHOWN: Mutex<Option<(String, bool, bool)>> = Mutex::new(None);
    let now = (tip, !paused.is_empty(), crate::jumplist::light_taskbar());
    {
        let mut shown = lock(&SHOWN);
        if shown.as_ref() == Some(&now) {
            return;
        }
        *shown = Some(now.clone());
    }
    // Set with the lock released: from another thread these wait on the main
    // thread, which may itself be waiting on this lock.
    let _ = tray.set_tooltip(Some(now.0));
    let icon = tray_icon();
    let _ = tray.set_icon(Some(if now.1 { with_pause_mark(&icon) } else { icon }));
}

/// The app icon with a small pause badge in its lower right corner.
fn with_pause_mark(icon: &Image<'_>) -> Image<'static> {
    let (w, h) = (icon.width() as i32, icon.height() as i32);
    let mut px = icon.rgba().to_vec();
    let r = (w as f32 * 0.24).max(4.0);
    let (cx, cy) = (w as f32 - r - 0.5, h as f32 - r - 0.5);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            if d > r + 1.0 {
                continue;
            }
            // A dark ring separates the badge from the icon under it.
            let bar = dy.abs() < r * 0.5 && (dx.abs() - r * 0.27).abs() < r * 0.14;
            let color: [u8; 3] = if d > r - 0.5 || bar { [24, 24, 28] } else { [236, 236, 240] };
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&color);
            px[i + 3] = 255;
        }
    }
    Image::new_owned(px, w as u32, h as u32)
}
