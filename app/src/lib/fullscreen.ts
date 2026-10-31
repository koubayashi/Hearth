// Fullscreen for the whole window: F11 anywhere, the Settings switch, and the
// Console view (which is always fullscreen while it's open). From a maximized
// window too (tao 0.37.2 and later fill the whole screen then).

import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export async function isFullscreen(): Promise<boolean> {
  if (isTauri()) return getCurrentWindow().isFullscreen();
  return document.fullscreenElement !== null;
}

export async function setFullscreen(on: boolean): Promise<void> {
  if (isTauri()) return getCurrentWindow().setFullscreen(on);
  if (on && !document.fullscreenElement) await document.documentElement.requestFullscreen().catch(() => {});
  if (!on && document.fullscreenElement) await document.exitFullscreen().catch(() => {});
}

export async function toggleFullscreen(): Promise<void> {
  await setFullscreen(!(await isFullscreen()));
}

/** F11 toggles fullscreen, as in most Windows apps. */
export function installFullscreenKey(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "F11") return;
    e.preventDefault();
    void toggleFullscreen();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
