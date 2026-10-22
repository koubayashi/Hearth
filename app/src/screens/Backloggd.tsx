import { useEffect, useRef } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { api } from "../lib/api";
import { Icon } from "../components/Icon";

/**
 * backloggd.com inside Hearth. The page itself is a separate browser view
 * placed exactly over the area below the toolbar (Backloggd can't load in an
 * iframe); this component keeps that view's position in sync.
 */
export function Backloggd() {
  const area = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = area.current;
    if (!el || !isTauri()) return;
    const place = () => {
      const r = el.getBoundingClientRect();
      api.backloggdShow(r.left, r.top, r.width, r.height).catch(() => {});
    };
    const ro = new ResizeObserver(place);
    ro.observe(el);
    window.addEventListener("resize", place);
    place();
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", place);
      api.backloggdHide().catch(() => {});
    };
  }, []);

  const nav = (action: "back" | "forward" | "reload" | "home") => () => api.backloggdNav(action);

  return (
    <div className="web-section">
      <div className="top web-bar" role="toolbar" aria-label="Backloggd">
        <button className="icon-btn small" data-nav aria-label="Back" title="Back" onClick={nav("back")}>
          <Icon name="arrowLeft" size={16} />
        </button>
        <button className="icon-btn small" data-nav aria-label="Forward" title="Forward" onClick={nav("forward")}>
          <Icon name="arrowRight" size={16} />
        </button>
        <button className="icon-btn small" data-nav aria-label="Reload" title="Reload" onClick={nav("reload")}>
          <Icon name="refresh" size={16} />
        </button>
        <button className="chip" data-nav onClick={nav("home")}>
          Backloggd home
        </button>
        <span className="spacer" />
        <span className="web-note">Ads blocked by uBlock Origin Lite</span>
        <button className="chip" data-nav onClick={() => api.backloggdOpenExternal()}>
          <Icon name="external" size={15} />
          Open in browser
        </button>
      </div>
      <div className="web-area" ref={area}>
        {!isTauri() && <p className="empty">Backloggd opens here in the desktop app.</p>}
      </div>
    </div>
  );
}
