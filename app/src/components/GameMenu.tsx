import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { Icon, type IconName } from "./Icon";

export type PanelAction = "artwork" | "match";

/** Right-click (or Menu key / controller Y) menu for a game. */
export function GameMenu({
  game,
  x,
  y,
  onClose,
  onPrimary,
  onToggleFavorite,
  onToggleHidden,
  onPanel,
}: {
  game: Game;
  x: number;
  y: number;
  onClose: () => void;
  onPrimary: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  /** Opens a picker in the game panel. */
  onPanel: (action: PanelAction) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // Keep the menu on screen near the pointer.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - r.height - 8)),
    });
    el.querySelector<HTMLElement>("button")?.focus();
  }, [x, y]);

  useEffect(() => {
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && onClose();
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", esc);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", esc);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const store = STORE_LABEL[game.entries[0]?.store ?? "manual"];
  const run = (f: () => void) => () => {
    onClose();
    f();
  };
  const item = (icon: IconName, label: string, f: () => void) => (
    <button role="menuitem" onClick={run(f)}>
      <Icon name={icon} />
      {label}
    </button>
  );

  return (
    <div
      ref={ref}
      className="menu context-menu"
      role="menu"
      aria-label={`${game.title} options`}
      style={{ left: pos.x, top: pos.y }}
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => {
        // Arrow keys move through the menu, not the library behind it.
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          e.stopPropagation();
          const items = [...(ref.current?.querySelectorAll<HTMLElement>("button") ?? [])];
          const i = items.indexOf(document.activeElement as HTMLElement);
          const next = e.key === "ArrowDown" ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
          items[next]?.focus();
        }
      }}
    >
      {item(game.installed ? "play" : "download", game.installed ? "Play" : `Install from ${store}`, () => onPrimary(game))}
      {item("star", game.favorite ? "Remove from favourites" : "Add to favourites", () => onToggleFavorite(game))}
      <hr />
      {item("library", "Change artwork…", () => onPanel("artwork"))}
      {item("search", "Find on Steam store…", () => onPanel("match"))}
      {item("external", "Open store page", () => api.openStorePage(game.id))}
      {game.installed && item("folder", "Open install folder", () => api.openInstallFolder(game.id))}
      <hr />
      {item(game.hidden ? "eye" : "hidden", game.hidden ? "Show in library" : "Hide from library", () => onToggleHidden(game))}
    </div>
  );
}
