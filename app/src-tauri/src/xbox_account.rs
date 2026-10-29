//! Signing in to Xbox with a Microsoft account, keeping that sign-in alive,
//! and signing out. The account's games come in through the Xbox provider.

use std::sync::Arc;

use hub_providers::{secrets, xbox_live};
use tauri::{AppHandle, Emitter, Manager};

use crate::{close_window, open_auth_window, sync_all, tracking, AppState, CmdResult};

/// The saved sign-in, written back to Credential Manager whenever it changes.
pub fn load(app: &AppHandle) -> Arc<xbox_live::XboxLive> {
    let session = secrets::load("xbox").and_then(|s| serde_json::from_str(&s).ok());
    let handle = app.clone();
    Arc::new(xbox_live::XboxLive::new(
        session,
        Box::new(move |s| {
            let json = s.and_then(|s| serde_json::to_string(s).ok());
            if let Err(e) = secrets::save("xbox", json.as_deref()) {
                tracking::log(&handle, &format!("xbox: couldn't save the sign-in ({e})"));
            }
        }),
    ))
}

/// Microsoft's sign-in page in a Hearth window. It redirects to a blank page
/// with a one-time code in the address, which the window picks up.
#[tauri::command]
pub async fn xbox_sign_in(app: AppHandle) -> CmdResult<()> {
    open_auth_window(
        &app,
        "xbox-login",
        crate::auth::Store::Xbox,
        "Sign in to Xbox",
        &xbox_live::login_url(),
        xbox_live::REDIRECT,
        true,
        |app, url| {
            close_window(&app, "xbox-login");
            match xbox_live::code_from_url(&url) {
                Some(code) => finish(app, code),
                None => {
                    let _ = app.emit("sync-error", "Xbox sign-in was cancelled.");
                }
            }
        },
    )
}

fn finish(app: AppHandle, code: String) {
    tauri::async_runtime::spawn(async move {
        let live = app.state::<AppState>().xbox.clone();
        match live.sign_in(&code).await {
            Ok(()) => {
                tracking::log(&app, "xbox: signed in");
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
pub fn xbox_sign_out(app: AppHandle) -> CmdResult<()> {
    app.state::<AppState>().xbox.forget();
    crate::auth::forget(&app, crate::auth::Store::Xbox);
    let _ = app.emit("accounts-changed", ());
    Ok(())
}

/// The saved sign-in was rejected (password changed, or unused for 90 days).
pub fn give_up(app: &AppHandle) {
    tracking::log(app, "xbox: signed out (Microsoft rejected the saved sign-in)");
    app.state::<AppState>().xbox.forget();
    crate::auth::forget(app, crate::auth::Store::Xbox);
    let _ = app.emit("accounts-changed", ());
    let _ = app.emit("sync-error", xbox_live::SESSION_EXPIRED);
}

/// Renews the sign-in every 30 minutes while Hearth runs. Microsoft's access
/// tokens last an hour and each renewal resets the refresh token's 90 days.
pub fn keep_alive(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let live = app.state::<AppState>().xbox.clone();
            if live.signed_in() {
                match live.keep_alive().await {
                    Ok(()) => {}
                    Err(e) if e.to_string() == xbox_live::SESSION_EXPIRED => give_up(&app),
                    // Offline: keep the sign-in and try again next round.
                    Err(e) => tracking::log(&app, &format!("xbox: couldn't renew yet ({e})")),
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
        }
    });
}
