import { forwardRef, useEffect, useRef, useState } from "react";
import type { Game } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { Art } from "./Art";
import { Icon } from "./Icon";

/** Everything you need about the focused game, pinned beside the library. */
export const WallLabel = forwardRef<
  HTMLButtonElement,
  {
    game: Game | null;
    /** Unix seconds when the tracker saw this game start, if it is running now. */
    playingSince: number | null;
    onPrimary: (g: Game) => void;
    onToggleHidden: (g: Game) => void;
    onToggleFavorite: (g: Game) => void;
    onBack: () => void;
  }
>(function WallLabel({ game, playingSince, onPrimary, onToggleHidden, onToggleFavorite, onBack }, primaryRef) {
  const [menu, setMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => setMenu(false), [game?.id]);
  useEffect(() => {
    if (!menu) return;
    menuRef.current?.querySelector("button")?.focus();
    const close = (e: MouseEvent) => !menuRef.current?.contains(e.target as Node) && setMenu(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [menu]);

  if (!game) {
    return (
      <aside className="wall-label">
        <p className="label-empty">Select a game to see its details.</p>
      </aside>
    );
  }

  const stores = game.entries.map((e) => STORE_LABEL[e.store]).join(" · ");
  const installStore = STORE_LABEL[game.entries[0]?.store ?? "manual"];

  return (
    <aside
      className="wall-label"
      aria-label={`${game.title} details`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          if (menu) setMenu(false);
          else onBack();
        }
      }}
    >
      <Art src={game.hero} title={game.title} className="cover-sm" />
      <h1>{game.title}</h1>
      <div className="where">{stores}</div>
      <dl>
        <dt>Played</dt>
        <dd>{formatPlaytime(game.playtime_secs)}</dd>
        <dt>Last played</dt>
        <dd>{formatLastPlayed(game.last_played)}</dd>
        <dt>Status</dt>
        <dd className={playingSince ? "playing" : undefined}>
          {playingSince
            ? `Playing now · ${formatPlaytime(Math.max(60, Math.floor(Date.now() / 1000) - playingSince))}`
            : game.installed
              ? "Ready to play"
              : "Not installed"}
        </dd>
      </dl>
      <div className="actions">
        <button ref={primaryRef} className="primary" data-nav onClick={() => onPrimary(game)}>
          <Icon name={game.installed ? "play" : "download"} size={16} />
          {game.installed ? "Play" : `Install from ${installStore}`}
        </button>
        <button
          className="icon-btn"
          data-nav
          aria-label={game.favorite ? "Remove from favourites" : "Add to favourites"}
          aria-pressed={game.favorite}
          onClick={() => onToggleFavorite(game)}
        >
          <Icon name="star" />
        </button>
        <button
          className="icon-btn"
          data-nav
          aria-label="More options"
          aria-haspopup="menu"
          aria-expanded={menu}
          onClick={() => setMenu((m) => !m)}
        >
          <Icon name="more" />
        </button>
        {menu && (
          <div className="menu" role="menu" ref={menuRef}>
            <button role="menuitem" onClick={() => onToggleHidden(game)}>
              <Icon name={game.hidden ? "eye" : "hidden"} />
              {game.hidden ? "Show in library" : "Hide from library"}
            </button>
          </div>
        )}
      </div>
    </aside>
  );
});
