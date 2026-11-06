//! A quick look at what's installed, when Hearth's window comes back into
//! focus: a game restored from the Recycle Bin, installed or uninstalled in
//! its store while Hearth sat in the background. Only local scans (store
//! install files, the registry, game folders), never the stores' servers;
//! the full sync with the stores stays on startup, its timer and Refresh.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use crate::{lock, now, AppState};

/// Switching back and forth quickly only rescans once in this long. The scan
/// itself takes milliseconds; this only stops a flurry of focus changes from
/// running several side by side.
const AT_MOST_EVERY: Duration = Duration::from_secs(10);

static LAST: Mutex<Option<Instant>> = Mutex::new(None);

/// Rescans whenever Windows activates Hearth's window (a click, Alt+Tab, the
/// taskbar, the tray). Watched here rather than in the page: the page only
/// hears about it when the web view itself gets the focus, which a click on
/// the taskbar doesn't always give it.
pub fn watch_focus(window: &tauri::WebviewWindow) {
    let app = window.app_handle().clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(true) = event {
            tauri::async_runtime::spawn(rescan_installed(app.clone()));
        }
    });
}

async fn rescan_installed(app: AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut last = lock(&LAST);
        if last.is_some_and(|t| t.elapsed() < AT_MOST_EVERY) {
            return;
        }
        // A full sync already running finds the same and more.
        if lock(&state.syncing).busy {
            return;
        }
        *last = Some(Instant::now());
    }
    let mut changed = false;
    for provider in state.providers.clone() {
        let store = provider.id();
        if store == hub_core::StoreId::Manual {
            // Folders other stores own are theirs, even inside a game folder.
            let owned = lock(&state.db).store_install_dirs(hub_core::StoreId::Manual).unwrap_or_default();
            lock(&state.folders).set_skip(owned.into_iter().map(Into::into).collect());
        }
        // A scan that fails says nothing about what's installed: skip that
        // store rather than mark all its games uninstalled.
        let Ok(installed) = provider.installed().await else { continue };
        match lock(&state.db).sync_installed(store, &installed, now()) {
            Ok(c) => changed |= c,
            Err(e) => eprintln!("rescan {}: {e}", store.as_str()),
        }
    }
    if !changed {
        return;
    }
    crate::tracking::log(&app, "rescan: installed games changed");
    crate::tracking::refresh_dirs(&app);
    let _ = app.emit("library-changed", ());
    // A game installed for the first time needs its artwork.
    crate::refresh_art(&app).await;
    let _ = app.emit("library-changed", ());
}
