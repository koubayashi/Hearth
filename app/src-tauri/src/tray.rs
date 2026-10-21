//! System tray: Hearth keeps running here when its window is closed, so the
//! playtime tracker never stops.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::tracking::{self, NowPlaying};

const TRAY_ID: &str = "hearth";

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Hearth", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hearth", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("Hearth")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
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
