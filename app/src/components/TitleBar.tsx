import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon } from "./Icon";

/** Frameless window: drag anywhere along the top, Windows-style buttons on the right. */
export function TitleBar() {
  if (!isTauri()) return null;
  return <WindowButtons />;
}

function WindowButtons() {
  const win = getCurrentWindow();
  // Maximized shows "Restore down", as in every Windows app. Snapping,
  // double-clicking the top and Win+Up all change it, so follow resizes.
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const read = () => void win.isMaximized().then(setMaximized);
    read();
    const off = win.onResized(read);
    return () => void off.then((f) => f());
  }, [win]);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <button className="winbtn" aria-label="Minimize" title="Minimize" onClick={() => win.minimize()}>
        <Icon name="minimize" size={14} />
      </button>
      <button
        className="winbtn"
        aria-label={maximized ? "Restore down" : "Maximize"}
        title={maximized ? "Restore down" : "Maximize"}
        onClick={() => win.toggleMaximize()}
      >
        <Icon name={maximized ? "restore" : "maximize"} size={12} />
      </button>
      <button className="winbtn close" aria-label="Close" title="Close" onClick={() => win.close()}>
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}
