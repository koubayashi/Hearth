//! The store sign-in windows' browser profiles.
//!
//! A store's sign-in page leaves its full website session in the browser's
//! cookies, which is far more than the token Hearth keeps in Credential
//! Manager (a Steam website session can trade and buy). So each store signs
//! in with a browser profile of its own, which goes as soon as the user signs
//! out, and every sign-in window may only visit its store and the sign-in
//! providers that store offers.

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::tracking;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    Steam,
    Epic,
    Xbox,
    Gog,
}

impl Store {
    const ALL: [Store; 4] = [Store::Steam, Store::Epic, Store::Xbox, Store::Gog];

    fn name(self) -> &'static str {
        match self {
            Store::Steam => "steam",
            Store::Epic => "epic",
            Store::Xbox => "xbox",
            Store::Gog => "gog",
        }
    }

    /// Every sign-in and refresh window that uses this store's profile.
    fn windows(self) -> &'static [&'static str] {
        match self {
            Store::Steam => &["steam-login", "steam-refresh"],
            Store::Epic => &["epic-login", "epic-refresh"],
            Store::Xbox => &["xbox-login"],
            Store::Gog => &["gog-login"],
        }
    }

    /// The sites the store's sign-in moves through, by domain (subdomains
    /// included).
    fn own_sites(self) -> &'static [&'static str] {
        match self {
            Store::Steam => &["steampowered.com", "steamcommunity.com"],
            Store::Epic => &["epicgames.com", "unrealengine.com"],
            Store::Xbox => &["live.com", "microsoft.com", "microsoftonline.com", "xbox.com"],
            Store::Gog => &["gog.com"],
        }
    }
}

/// "Sign in with …" providers the stores offer. Their pages usually open in
/// a pop-up, but some redirect the sign-in window itself.
const SIGN_IN_PROVIDERS: &[&str] = &[
    "google.com", "facebook.com", "apple.com", "github.com", "discord.com", "twitch.tv",
    "playstation.com", "sony.com", "sonyentertainmentnetwork.com", "nintendo.com", "nintendo.net",
    "live.com", "microsoft.com", "microsoftonline.com", "xbox.com", "steamcommunity.com",
    "steampowered.com", "lego.com",
];

fn on_domain(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|rest| rest.ends_with('.'))
}

/// Whether a sign-in window may open this page itself.
pub fn allowed(store: Store, url: &tauri::Url) -> bool {
    if url.scheme() == "about" {
        return true;
    }
    let Some(host) = url.host_str().filter(|_| url.scheme() == "https") else { return false };
    store.own_sites().iter().chain(SIGN_IN_PROVIDERS).any(|d| on_domain(host, d))
}

fn root(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_local_data_dir().ok().map(|d| d.join("sign-in"))
}

/// The store's own browser profile.
pub fn profile(app: &AppHandle, store: Store) -> Option<PathBuf> {
    root(app).map(|r| r.join(store.name()))
}

/// A secret for one sign-in window: only Hearth's own script, run on the
/// store's page, knows it, so another page in the window can't pose as the
/// store's answer.
pub fn nonce() -> String {
    use std::hash::{BuildHasher, Hasher};
    // RandomState is seeded from the OS's random source, fresh each call.
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(crate::now() as u64);
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// Closes the store's windows and deletes its profile, cookies and all. The
/// browser lets go of the folder a moment after its last window closes, so
/// this keeps trying for a while; whatever is left goes at the next start.
pub fn forget(app: &AppHandle, store: Store) {
    for label in store.windows() {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.close();
        }
    }
    let Some(dir) = profile(app, store) else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..40 {
            if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
                tracking::log(&app, &format!("{}: removed the sign-in browser profile", store.name()));
                return;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tracking::log(&app, &format!("{}: sign-in browser profile still in use; removed at next start", store.name()));
    });
}

/// At start, before any sign-in window exists: remove the profile of every
/// store that has no sign-in needing it (signed out, or left over from a
/// sign-out the browser still held).
pub fn forget_unused(app: &AppHandle, in_use: impl Fn(Store) -> bool) {
    for store in Store::ALL {
        if in_use(store) {
            continue;
        }
        if let Some(dir) = profile(app, store).filter(|d| d.exists()) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Builds before 2026-10-29 signed in to every store inside the main window's
/// own profile. Those website sessions are deleted once (the interface itself
/// keeps no cookies; its settings live in local storage, which stays).
pub fn forget_shared_cookies(window: &tauri::WebviewWindow) {
    let Ok(dir) = window.app_handle().path().app_data_dir() else { return };
    let marker = dir.join("shared-cookies-cleared");
    if marker.exists() {
        return;
    }
    #[cfg(windows)]
    {
        let app = window.app_handle().clone();
        let _ = window.with_webview(move |webview| {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_2;
            use windows_core::Interface;
            // SAFETY: plain COM calls on the live webview, on its own thread.
            let cleared = unsafe {
                webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.cast::<ICoreWebView2_2>())
                    .and_then(|core| core.CookieManager())
                    .and_then(|cookies| cookies.DeleteAllCookies())
            };
            match cleared {
                Ok(()) => {
                    let _ = std::fs::write(&marker, "");
                    tracking::log(&app, "sign-in: removed store website sessions from the main profile");
                }
                Err(e) => tracking::log(&app, &format!("sign-in: couldn't clear the main profile's cookies ({e})")),
            }
        });
    }
    #[cfg(not(windows))]
    let _ = std::fs::write(&marker, "");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> tauri::Url {
        s.parse().unwrap()
    }

    #[test]
    fn sign_in_windows_stay_on_their_store_and_sign_in_providers() {
        assert!(allowed(Store::Steam, &url("https://store.steampowered.com/login/")));
        assert!(allowed(Store::Epic, &url("https://accounts.google.com/o/oauth2/auth")));
        assert!(allowed(Store::Xbox, &url("https://login.live.com/oauth20_desktop.srf?code=x")));
        assert!(!allowed(Store::Steam, &url("https://steampowered.com.attacker.example/")));
        assert!(!allowed(Store::Steam, &url("https://notsteampowered.com/")));
        assert!(!allowed(Store::Gog, &url("http://www.gog.com/")), "not over plain http");
        assert!(!allowed(Store::Gog, &url("https://example.com/")));
    }

    #[test]
    fn nonces_differ() {
        assert_ne!(nonce(), nonce());
        assert_eq!(nonce().len(), 32);
    }
}
