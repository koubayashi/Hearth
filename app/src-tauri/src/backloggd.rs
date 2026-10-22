//! The Backloggd section: backloggd.com in a browser view placed over the
//! page's content area. Backloggd refuses to load in an iframe
//! (X-Frame-Options: SAMEORIGIN), so it gets its own child webview, with its
//! own browser profile (its login stays separate from Hearth) and uBlock
//! Origin Lite loaded as an ad blocker.

use tauri::webview::{NewWindowResponse, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

use crate::{err, CmdResult};

const LABEL: &str = "backloggd";
const HOME: &str = "https://backloggd.com/";

/// Places (or creates) the Backloggd view over the page's content area.
/// Coordinates are CSS pixels relative to the window.
#[tauri::command]
pub async fn backloggd_show(app: AppHandle, x: f64, y: f64, width: f64, height: f64) -> CmdResult<()> {
    let pos = LogicalPosition::new(x, y);
    let size = LogicalSize::new(width.max(1.0), height.max(1.0));
    if let Some(view) = app.get_webview(LABEL) {
        view.set_position(pos).map_err(err)?;
        view.set_size(size).map_err(err)?;
        return view.show().map_err(err);
    }
    let window = app.get_window("main").ok_or("The main window isn't open")?;
    let data = app.path().app_data_dir().map_err(err)?.join("backloggd-profile");
    let mut builder = WebviewBuilder::new(LABEL, WebviewUrl::External(HOME.parse().map_err(err)?))
        // Its own profile: separate cookies, and its own extension settings.
        .data_directory(data)
        // Sign-in and share links may open popups; keep them to https.
        .on_new_window(|url, _| {
            if url.scheme() == "https" {
                NewWindowResponse::Allow
            } else {
                NewWindowResponse::Deny
            }
        });
    if let Some(dir) = extensions_dir(&app) {
        builder = builder.browser_extensions_enabled(true).extensions_path(dir);
    }
    window.add_child(builder, pos, size).map_err(err)?;
    Ok(())
}

/// Bundled extensions (uBlock Origin Lite), if present.
fn extensions_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    let candidates = [
        app.path().resource_dir().ok().map(|d| d.join("extensions")),
        // Development builds run from target/: use the source folder.
        Some(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("extensions")),
    ];
    candidates.into_iter().flatten().find(|d| d.join("ubolite").join("manifest.json").is_file())
}

/// Hides the view when leaving the section; it stays loaded so the page and
/// login survive switching back.
#[tauri::command]
pub fn backloggd_hide(app: AppHandle) -> CmdResult<()> {
    if let Some(view) = app.get_webview(LABEL) {
        view.hide().map_err(err)?;
    }
    Ok(())
}

/// Toolbar actions: back, forward, reload, home.
#[tauri::command]
pub fn backloggd_nav(app: AppHandle, action: String) -> CmdResult<()> {
    let Some(view) = app.get_webview(LABEL) else { return Ok(()) };
    match action.as_str() {
        "back" => view.eval("history.back()").map_err(err),
        "forward" => view.eval("history.forward()").map_err(err),
        "reload" => view.reload().map_err(err),
        "home" => view.navigate(HOME.parse::<Url>().map_err(err)?).map_err(err),
        _ => Err(format!("Unknown action {action}")),
    }
}

/// Opens the page currently shown in the default browser.
#[tauri::command]
pub fn backloggd_open_external(app: AppHandle) -> CmdResult<()> {
    let url = app
        .get_webview(LABEL)
        .and_then(|v| v.url().ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| HOME.to_string());
    if !url.starts_with("https://") {
        return Err("Only web pages can be opened".into());
    }
    hub_launch::run(&hub_launch::LaunchAction::Uri { uri: url }).map_err(err)
}

