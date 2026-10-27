import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { api } from "./lib/api";
import { applyPrefs } from "./lib/prefs";
import { startGamepadBridge } from "./lib/gamepad";
import { installSpatialNav } from "./lib/spatial";
import { installFullscreenKey } from "./lib/fullscreen";
import { CrashBoundary, reportUnhandledErrors } from "./components/CrashScreen";
import "./styles/app.css";
import "./styles/views.css";
import "./styles/ways.css";
import "./styles/system.css";

reportUnhandledErrors();
applyPrefs();
installSpatialNav();
startGamepadBridge();
installFullscreenKey();

// ?crash in the demo shows the crash screen, to design that state.
const demoCrash = api.demo && location.search.includes("crash");
function Crash(): never {
  throw new Error("Demo crash: the library couldn't be drawn");
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CrashBoundary>{demoCrash ? <Crash /> : <App />}</CrashBoundary>
  </StrictMode>,
);
