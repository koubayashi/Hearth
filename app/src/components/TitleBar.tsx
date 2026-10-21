import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon } from "./Icon";

/** Frameless window: drag anywhere along the top, Windows-style buttons on the right. */
export function TitleBar() {
  if (!isTauri()) return null;
  const win = getCurrentWindow();
  return (
    <div className="titlebar" data-tauri-drag-region>
      <button className="winbtn" aria-label="Minimize" onClick={() => win.minimize()}>
        <Icon name="minimize" size={14} />
      </button>
      <button className="winbtn" aria-label="Maximize" onClick={() => win.toggleMaximize()}>
        <Icon name="maximize" size={12} />
      </button>
      <button className="winbtn close" aria-label="Close" onClick={() => win.close()}>
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}
