//! Hearth's window is an app, not a browser tab: the browser's own shortcuts
//! (Ctrl+R reload, Ctrl+P print, Ctrl+F find, Ctrl+S save page, Alt+← back,
//! F7 caret browsing) are switched off in the main window, so a stray key
//! can't reload the page or navigate it away. Keys still reach the page, so
//! Hearth's own shortcuts (F5 refresh, Esc, the controller) work. Store
//! sign-in windows keep them: those are real web pages where people type
//! passwords and may want to paste or go back.
//!
//! The right-click menu is the page's job (see main.tsx): none, except
//! cut/copy/paste in text boxes.

pub fn apply(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        let _ = window.with_webview(|webview| {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
            use windows_core::Interface;
            // SAFETY: plain COM calls on the live webview, on its own thread.
            let _ = unsafe {
                webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.Settings())
                    .and_then(|settings| settings.cast::<ICoreWebView2Settings3>())
                    .and_then(|settings| settings.SetAreBrowserAcceleratorKeysEnabled(false))
            };
        });
    }
    #[cfg(not(windows))]
    let _ = window;
}
