//! Signing in to GOG, keeping that sign-in alive, and signing out. The
//! account's games come in through the GOG provider.

use std::sync::Arc;

use hub_providers::{gog_web, secrets};
use tauri::{AppHandle, Emitter, Manager};

use crate::{close_window, open_auth_window, sync_all, tracking, AppState, CmdResult};

/// The saved sign-in, written back to Credential Manager whenever it changes.
pub fn load(app: &AppHandle, data_dir: &std::path::Path) -> Arc<gog_web::GogWeb> {
    let session = secrets::load("gog").and_then(|s| serde_json::from_str(&s).ok());
    let handle = app.clone();
    Arc::new(gog_web::GogWeb::new(
        data_dir,
        session,
        Box::new(move |s| {
            let json = s.and_then(|s| serde_json::to_string(s).ok());
            if let Err(e) = secrets::save("gog", json.as_deref()) {
                tracking::log(&handle, &format!("gog: couldn't save the sign-in ({e})"));
            }
        }),
    ))
}

/// GOG's sign-in page in a Hearth window. It redirects to a GOG page with a
/// one-time code in the address, which the window picks up.
#[tauri::command]
pub async fn gog_sign_in(app: AppHandle) -> CmdResult<()> {
    open_auth_window(
        &app,
        "gog-login",
        crate::auth::Store::Gog,
        "Sign in to GOG",
        &gog_web::login_url(),
        gog_web::REDIRECT,
        true,
        |app, url| {
            close_window(&app, "gog-login");
            match gog_web::code_from_url(&url) {
                Some(code) => finish(app, code),
                None => {
                    let _ = app.emit("sync-error", "GOG sign-in was cancelled.");
                }
            }
        },
    )
}

fn finish(app: AppHandle, code: String) {
    tauri::async_runtime::spawn(async move {
        let gog = app.state::<AppState>().gog.clone();
        match gog.sign_in(&code).await {
            Ok(()) => {
                tracking::log(&app, "gog: signed in");
                let _ = app.emit("accounts-changed", ());
                sync_all(app);
            }
            Err(e) => {
                let _ = app.emit("accounts-changed", ());
                let _ = app.emit("sync-error", crate::tracking::redact(&format!("{e:#}")));
            }
        }
    });
}

#[tauri::command]
pub fn gog_sign_out(app: AppHandle) -> CmdResult<()> {
    app.state::<AppState>().gog.forget();
    crate::auth::forget(&app, crate::auth::Store::Gog);
    let _ = app.emit("accounts-changed", ());
    Ok(())
}

/// The saved sign-in was rejected (password changed, or signed out on GOG).
pub fn give_up(app: &AppHandle) {
    tracking::log(app, "gog: signed out (GOG rejected the saved sign-in)");
    app.state::<AppState>().gog.forget();
    crate::auth::forget(app, crate::auth::Store::Gog);
    let _ = app.emit("accounts-changed", ());
    let _ = app.emit("sync-error", gog_web::SESSION_EXPIRED);
}

/// Renews the sign-in every 30 minutes while Hearth runs; GOG's access
/// tokens last an hour and each renewal gives a new refresh token.
pub fn keep_alive(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let gog = app.state::<AppState>().gog.clone();
            if gog.signed_in() {
                match gog.keep_alive().await {
                    Ok(()) => {}
                    Err(e) if e.to_string() == gog_web::SESSION_EXPIRED => give_up(&app),
                    // Offline: keep the sign-in and try again next round.
                    Err(e) => tracking::log(&app, &format!("gog: couldn't renew yet ({e})")),
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
        }
    });
}
