import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setFullscreen } from "../lib/fullscreen";
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
  // In full screen (F11) the same button leaves full screen.
  const [maximized, setMaximized] = useState(false);
  const [full, setFull] = useState(false);
  useEffect(() => {
    const read = () => {
      void win.isMaximized().then(setMaximized);
      void win.isFullscreen().then(setFull);
    };
    read();
    const off = win.onResized(read);
    return () => void off.then((f) => f());
  }, [win]);
  const label = full ? "Exit full screen" : maximized ? "Restore down" : "Maximize";

  const maxRef = useRef<HTMLButtonElement>(null);
  const [mouse, setMouse] = useState({ hover: false, pressed: false });
  // Snap Layouts don't apply to a full-screen window.
  useSnapLayouts(maxRef, setMouse, !full);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <button className="winbtn" aria-label="Minimize" title="Minimize" onClick={() => win.minimize()}>
        <Icon name="minimize" size={14} />
      </button>
      <button
        ref={maxRef}
        className="winbtn"
        aria-label={label}
        title={label}
        data-hover={(!full && mouse.hover) || undefined}
        data-pressed={(!full && mouse.pressed) || undefined}
        onClick={() => (full ? setFullscreen(false) : win.toggleMaximize())}
      >
        <Icon name={full || maximized ? "restore" : "maximize"} size={12} />
      </button>
      <button className="winbtn close" aria-label="Close" title="Close" onClick={() => win.close()}>
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}

/**
 * Snap Layouts: an invisible window of Hearth's own sits over the Maximize
 * button so Windows shows the layouts flyout (snap_layouts.rs). It's told
 * where the button is, or that something covers it (a dialog, the Console
 * view), so it never takes a click meant for something else. The mouse then
 * reaches that window, not the page, so hover and press come back from it.
 */
function useSnapLayouts(
  ref: React.RefObject<HTMLButtonElement | null>,
  onMouse: (m: { hover: boolean; pressed: boolean }) => void,
  on: boolean,
) {
  useEffect(() => {
    if (!on) {
      void invoke("place_maximize_button", { place: null });
      return;
    }
    let sent = "";
    let frame = 0;
    const report = () => {
      frame = 0;
      const button = ref.current;
      const r = button?.getBoundingClientRect();
      const top = r && document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      const s = devicePixelRatio;
      const place = r && top && button?.contains(top) ? { x: r.left * s, y: r.top * s, width: r.width * s, height: r.height * s } : null;
      const key = JSON.stringify(place);
      if (key === sent) return;
      sent = key;
      void invoke("place_maximize_button", { place });
    };
    const soon = () => {
      if (!frame) frame = requestAnimationFrame(report);
    };
    report();
    // Layout changes with the window's size and DPI; covers come and go with the page.
    window.addEventListener("resize", soon);
    const covers = new MutationObserver(soon);
    covers.observe(document.body, { childList: true, subtree: true });
    const off = listen<{ hover: boolean; pressed: boolean }>("maximize-button", (e) => onMouse(e.payload));
    return () => {
      window.removeEventListener("resize", soon);
      covers.disconnect();
      cancelAnimationFrame(frame);
      void off.then((f) => f());
      void invoke("place_maximize_button", { place: null });
    };
  }, [ref, onMouse, on]);
}
