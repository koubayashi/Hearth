import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import * as autostart from "@tauri-apps/plugin-autostart";

/** Start with Windows, for the desk and console settings. `enabled` is null
 *  while unknown (and always in the browser demo). */
export function useStartWithWindows() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    autostart.isEnabled().then(setEnabled).catch(() => setEnabled(null));
  }, []);
  const toggle = async () => {
    await (enabled ? autostart.disable() : autostart.enable());
    setEnabled(await autostart.isEnabled());
  };
  return { enabled, toggle };
}
