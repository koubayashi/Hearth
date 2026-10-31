//! System tray: Hearth keeps running here when its window is closed, so the
//! playtime tracker never stops.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::tracking::{self, NowPlaying};

const TRAY_ID: &str = "hearth";

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
    #[cfg(target_os = "windows")]
    let _ = window_vibrancy::apply_mica(window, None);
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// The tray menu, with a restart item while a downloaded update waits.
fn menu(app: &AppHandle, update: Option<&str>) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open Hearth", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hearth", true, None::<&str>)?;
    match update {
        Some(v) => {
            let label = format!("Restart to update to {v}");
            let update = MenuItem::with_id(app, "update", label, true, None::<&str>)?;
            Menu::with_items(app, &[&open, &update, &quit])
        }
        None => Menu::with_items(app, &[&open, &quit]),
    }
}

pub fn set_update(app: &AppHandle, version: Option<&str>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    if let Ok(m) = menu(app, version) {
        let _ = tray.set_menu(Some(m));
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("Hearth")
        .menu(&menu(app, None)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "update" => {
                if let Err(e) = crate::updates::install(app) {
                    tracking::log(app, &format!("update: install failed ({e})"));
                }
            }
            "quit" => {
                tracking::flush(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn set_playing(app: &AppHandle, playing: &[NowPlaying]) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let tip = match playing {
        [] => "Hearth".to_string(),
        [one] => format!("Hearth · timing {}", one.title),
        many => format!("Hearth · timing {} games", many.len()),
    };
    let _ = tray.set_tooltip(Some(tip));
}
