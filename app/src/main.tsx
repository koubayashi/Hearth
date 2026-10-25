import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { applyPrefs } from "./lib/prefs";
import { startGamepadBridge } from "./lib/gamepad";
import { installSpatialNav } from "./lib/spatial";
import { installFullscreenKey } from "./lib/fullscreen";
import "./styles/app.css";

applyPrefs();
installSpatialNav();
startGamepadBridge();
installFullscreenKey();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
