//! System tray: Hearth keeps running here when its window is closed, so the
//! playtime tracker never stops.

use std::sync::Mutex;

use hub_core::LibraryQuery;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Listener, Manager};

use crate::tracking::{self, NowPlaying};
use crate::{lock, AppState};

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
    crate::webview_lock::apply(window);
    crate::rescan::watch_focus(window);
    #[cfg(target_os = "windows")]
    let _ = window_vibrancy::apply_mica(window, None);
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// Recent games in the tray menu.
const RECENT: usize = 5;

/// The version a downloaded update would install, for the restart item.
static UPDATE: Mutex<Option<String>> = Mutex::new(None);

/// The tray menu: recent games to start, a way to stop timing a game the
/// tracker has wrong, then Open, the update restart while one waits, and Quit.
fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let m = Menu::new(app)?;
    let recent = {
        let state = app.state::<AppState>();
        let mut games = lock(&state.db).list_games(&LibraryQuery::default()).unwrap_or_default();
        games.retain(|g| g.installed && g.last_played.is_some());
        games.sort_by_key(|g| std::cmp::Reverse(g.last_played));
        games.truncate(RECENT);
        games
    };
    for g in &recent {
        m.append(&MenuItem::with_id(app, format!("play:{}", g.id), menu_text(&g.title), true, None::<&str>)?)?;
    }
    let playing = tracking::now_playing(app);
    if !recent.is_empty() || !playing.is_empty() {
        m.append(&PredefinedMenuItem::separator(app)?)?;
    }
    for p in &playing {
        let label = format!("Stop timing {}", menu_text(&p.title));
        m.append(&MenuItem::with_id(app, format!("stop:{}", p.game_id), label, true, None::<&str>)?)?;
    }
    if !playing.is_empty() {
        m.append(&PredefinedMenuItem::separator(app)?)?;
    }
    m.append(&MenuItem::with_id(app, "open", "Open Hearth", true, None::<&str>)?)?;
    if let Some(v) = lock(&UPDATE).as_deref() {
        m.append(&MenuItem::with_id(app, "update", format!("Restart to update to {v}"), true, None::<&str>)?)?;
    }
    m.append(&MenuItem::with_id(app, "quit", "Quit Hearth", true, None::<&str>)?)?;
    Ok(m)
}

/// Windows menus read "&" as an access key: double it so titles show as written.
fn menu_text(title: &str) -> String {
    title.replace('&', "&&")
}

/// Builds the menu again: after a sync, a session, or an update download.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    if let Ok(m) = menu(app) {
        let _ = tray.set_menu(Some(m));
    }
}

pub fn set_update(app: &AppHandle, version: Option<&str>) {
    *lock(&UPDATE) = version.map(str::to_string);
    refresh(app);
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("Hearth")
        .menu(&menu(app)?)
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
            id => {
                if let Some(game) = id.strip_prefix("play:").and_then(|g| g.parse().ok()) {
                    if let Err(e) = crate::launch_game(app.state::<AppState>(), game, None) {
                        tracking::log(app, &format!("tray: couldn't start game {game} ({e})"));
                    }
                } else if let Some(game) = id.strip_prefix("stop:").and_then(|g| g.parse().ok()) {
                    tracking::stop(app, game);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    // Recent games change with every sync and session.
    let handle = app.clone();
    app.listen_any("library-changed", move |_| refresh(&handle));
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
    refresh(app);
}
