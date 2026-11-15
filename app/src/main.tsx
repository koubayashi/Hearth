import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { api } from "./lib/api";
import { applyPrefs, followTaskbar } from "./lib/prefs";
import { startGamepadBridge } from "./lib/gamepad";
import { installSpatialNav } from "./lib/spatial";
import { installFullscreenKey } from "./lib/fullscreen";
import { CrashBoundary, reportUnhandledErrors } from "./components/CrashScreen";
import "./styles/app.css";
import "./styles/views.css";
import "./styles/ways.css";
import "./styles/storage.css";
import "./styles/system.css";
import "./styles/tray.css";
import { TrayMenu } from "./screens/TrayMenu";

reportUnhandledErrors();
applyPrefs();

// Hearth is an app, not a web page: no browser right-click menu (Back,
// Refresh, Save as, Print). Text boxes keep it for cut, copy and paste.
// Games have their own menu; their handlers run first and stop this one
// from mattering. The browser's own shortcuts are off too (webview_lock.rs).
const editable = (t: EventTarget | null) =>
  t instanceof HTMLElement && !!t.closest('input:not([type="checkbox"]):not([type="radio"]):not([type="range"]), textarea, [contenteditable="true"]');
window.addEventListener("contextmenu", (e) => {
  if (!editable(e.target)) e.preventDefault();
});
// The tray icon's menu is this same interface in a small window of its own
// (tray.rs). It has none of the main window's controller or full-screen keys.
const trayMenu = new URLSearchParams(location.search).has("tray");
if (trayMenu) {
  document.documentElement.classList.add("tray-window");
  void api.taskbarLight().then(followTaskbar).catch(() => {});
  void api.onTaskbarTheme((light) => followTaskbar(light === true));
} else {
  installSpatialNav();
  startGamepadBridge();
  installFullscreenKey();
}

// ?crash in the demo shows the crash screen, to design that state.
const demoCrash = api.demo && location.search.includes("crash");
function Crash(): never {
  throw new Error("Demo crash: the library couldn't be drawn");
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CrashBoundary>{trayMenu ? <TrayMenu /> : demoCrash ? <Crash /> : <App />}</CrashBoundary>
  </StrictMode>,
);
