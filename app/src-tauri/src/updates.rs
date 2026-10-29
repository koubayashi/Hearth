//! Updates from GitHub Releases. Installed builds check shortly after start
//! and twice a day, download a newer Hearth in the background, and install it
//! only when the user says so (Settings or the tray), so a game being timed
//! is never cut off. Updates are signed; the updater refuses unsigned ones.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{err, lock, tracking, tray, CmdResult};

const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_EVERY: Duration = Duration::from_secs(12 * 3600);

#[derive(Default)]
pub struct Updates {
    /// A downloaded update, waiting for the user to restart into it.
    ready: Mutex<Option<(Update, Vec<u8>)>>,
}

pub fn watch(app: AppHandle) {
    // Dev builds run from the source tree; the installer would replace the wrong thing.
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        // Let the first sync have the network.
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if let Err(e) = fetch(&app).await {
                tracking::log(&app, &format!("update: couldn't check ({e})"));
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

async fn fetch(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else { return Ok(()) };
    let state = app.state::<Updates>();
    if lock(&state.ready).as_ref().is_some_and(|(u, _)| u.version == update.version) {
        return Ok(());
    }
    let bytes = update.download(|_, _| {}, || {}).await?;
    let version = update.version.clone();
    *lock(&state.ready) = Some((update, bytes));
    tray::set_update(app, Some(&version));
    let _ = app.emit("update-ready", &version);
    Ok(())
}

/// Closes Hearth, runs the installer and starts the new version.
pub fn install(app: &AppHandle) -> CmdResult<()> {
    let Some((update, bytes)) = lock(&app.state::<Updates>().ready).take() else {
        return Err("No update is ready.".into());
    };
    tracking::flush(app);
    update.install(bytes).map_err(err)?;
    app.restart();
}

/// The version waiting to be installed, if any.
#[tauri::command]
pub fn update_ready(state: State<Updates>) -> Option<String> {
    lock(&state.ready).as_ref().map(|(u, _)| u.version.clone())
}

#[tauri::command]
pub fn install_update(app: AppHandle) -> CmdResult<()> {
    install(&app)
}
