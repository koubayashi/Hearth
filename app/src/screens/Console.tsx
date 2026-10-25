import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, NowPlaying } from "../lib/types";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { isFullscreen, setFullscreen } from "../lib/fullscreen";
import { Art, legacy } from "../components/Art";
import { isMenuKey } from "../components/CoverGrid";
import { GameMenu, type PanelAction } from "../components/GameMenu";
import { Icon } from "../components/Icon";
import { StoreIcon } from "../components/StoreIcon";
import { ArtworkPicker } from "../components/ArtworkPicker";
import { StoreMatch } from "../components/StoreMatch";

type Tab = "recent" | "installed" | "all";
const TABS: { id: Tab; label: string }[] = [
  { id: "recent", label: "Recently played" },
  { id: "installed", label: "Installed" },
  { id: "all", label: "All games" },
];

/**
 * Fullscreen and controller-first, for the couch: one row of games under the
 * selected game's art. D-pad or stick to move, A to play, Y for options,
 * LB/RB for the tabs, Start (or the Exit button) to leave.
 */
export function Console({
  version,
  playing,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
  onExit,
}: {
  version: number;
  playing: NowPlaying[];
  onPrimary: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onExit: () => void;
}) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [tab, setTab] = useState<Tab | null>(null);
  const [index, setIndex] = useState(0);
  const [menu, setMenu] = useState<DOMRect | null>(null);
  const [panel, setPanel] = useState<PanelAction | null>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  const playRef = useRef<HTMLButtonElement>(null);
  const moreRef = useRef<HTMLButtonElement>(null);

  // Fullscreen while the console is open; back to how the window was after.
  useEffect(() => {
    let was = false;
    isFullscreen().then((f) => {
      was = f;
      if (!f) void setFullscreen(true);
    });
    return () => {
      if (!was) void setFullscreen(false);
    };
  }, []);

  useEffect(() => {
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then(setGames).catch(() => setGames([]));
  }, [version]);

  const lists = useMemo(() => {
    const all = games ?? [];
    return {
      recent: all
        .filter((g) => g.last_played !== null)
        .sort((a, b) => b.last_played! - a.last_played!)
        .slice(0, 20),
      installed: all.filter((g) => g.installed),
      all,
    };
  }, [games]);

  // Start where there's something: recent games, else installed, else everything.
  const current: Tab = tab ?? (lists.recent.length > 0 ? "recent" : lists.installed.length > 0 ? "installed" : "all");
  const row = lists[current];
  const at = Math.min(index, Math.max(0, row.length - 1));
  const game = row[at] ?? null;

  // Keep the same game selected when the library refreshes under us.
  const selectedId = useRef<number | null>(null);
  useEffect(() => {
    const i = selectedId.current === null ? -1 : row.findIndex((g) => g.id === selectedId.current);
    if (i >= 0 && i !== index) setIndex(i);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row]);
  selectedId.current = game?.id ?? null;

  const tileFocus = () => rowRef.current?.querySelector<HTMLElement>(`[data-index="${at}"]`)?.focus({ preventScroll: true });
  // The first tile has focus on arrival, so a controller works at once.
  useEffect(() => {
    if (games) tileFocus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [games !== null, current]);

  const switchTab = (dir: 1 | -1) => {
    const i = TABS.findIndex((t) => t.id === current);
    const next = TABS[(i + dir + TABS.length) % TABS.length].id;
    setTab(next);
    setIndex(0);
    setMenu(null);
  };
  const move = (to: number) => {
    const i = Math.max(0, Math.min(row.length - 1, to));
    setIndex(i);
    rowRef.current?.querySelector<HTMLElement>(`[data-index="${i}"]`)?.focus({ preventScroll: true });
  };

  // Bumpers and Start work from anywhere in the console, except inside a dialog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (menu || panel) return;
      if (e.key === "PageUp" || e.key === "PageDown") {
        e.preventDefault();
        switchTab(e.key === "PageDown" ? 1 : -1);
      } else if (e.key === "GamepadStart") {
        e.preventDefault();
        onExit();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(t);
  }, []);

  const playingSince = game ? (playing.find((p) => p.game_id === game.id)?.started_at ?? null) : null;
  const status = game?.status ? STATUSES.find((s) => s.id === game.status)?.label : null;
  const stores = game ? [...new Set(game.entries.map((e) => e.store))] : [];

  return (
    <div className="console" aria-label="Console">
      <ConsoleArt src={game?.hero ?? game?.cover ?? null} />

      <header className="cx-top">
        <nav className="cx-tabs" aria-label="Shelves">
          <kbd className="cx-bumper" aria-hidden>
            LB
          </kbd>
          {TABS.map((t) => (
            <button
              key={t.id}
              className="cx-tab"
              aria-current={current === t.id ? "page" : undefined}
              data-nav
              onClick={() => {
                setTab(t.id);
                setIndex(0);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  tileFocus();
                }
              }}
            >
              {t.label}
              <small>{lists[t.id].length}</small>
            </button>
          ))}
          <kbd className="cx-bumper" aria-hidden>
            RB
          </kbd>
        </nav>
        <span className="spacer" />
        <time className="cx-clock">{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
        <button className="cx-exit" data-nav title="Leave the console (Start)" onClick={onExit}>
          <Icon name="close" size={16} />
          Exit
        </button>
      </header>

      <div
        className="cx-row"
        ref={rowRef}
        role="listbox"
        aria-label={TABS.find((t) => t.id === current)!.label}
        onWheel={(e) => {
          const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
          if (Math.abs(d) > 20) move(at + Math.sign(d));
        }}
      >
        <div className="cx-track" style={{ "--at": at } as React.CSSProperties}>
          {row.map((g, i) => (
            <button
              key={g.id}
              className={`cx-tile${i === at ? " on" : i > at ? " after" : ""}${g.installed ? "" : " dim"}`}
              role="option"
              aria-selected={i === at}
              aria-label={g.title}
              data-nav
              data-index={i}
              tabIndex={i === at ? 0 : -1}
              onFocus={() => setIndex(i)}
              onClick={() => (i === at ? playRef.current?.focus() : move(i))}
              onDoubleClick={() => onPrimary(g)}
              onKeyDown={(e) => {
                if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
                  e.preventDefault();
                  move(at + (e.key === "ArrowRight" ? 1 : -1));
                } else if (e.key === "ArrowDown") {
                  e.preventDefault();
                  playRef.current?.focus();
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  document.querySelector<HTMLElement>(".cx-tab[aria-current]")?.focus();
                } else if (e.key === "Enter") {
                  e.preventDefault();
                  if (!g.offline_drive) onPrimary(g);
                } else if (isMenuKey(e)) {
                  e.preventDefault();
                  setMenu(moreRef.current?.getBoundingClientRect() ?? e.currentTarget.getBoundingClientRect());
                }
              }}
            >
              <Art src={g.cover ?? g.hero} title={g.title} />
              {playing.some((p) => p.game_id === g.id) && <span className="cx-live" title="Playing now" />}
            </button>
          ))}
          {games && row.length === 0 && (
            <p className="cx-empty">
              {current === "recent" ? "Games you play show up here." : current === "installed" ? "No games installed." : "Your library is empty. Connect a store in Settings."}
            </p>
          )}
        </div>
      </div>

      {game && (
        <section
          className="cx-info"
          key={game.id}
          aria-label={game.title}
          onKeyDown={(e) => {
            if (e.defaultPrevented) return;
            if (e.key === "ArrowUp" || e.key === "Escape") {
              e.preventDefault();
              tileFocus();
            }
          }}
        >
          <h1 className="cx-title">{game.title}</h1>
          <p className="cx-meta">
            {playingSince ? (
              <span className="cx-now">Playing now</span>
            ) : (
              <span>{game.installed ? "Ready to play" : game.offline_drive ? `Drive ${game.offline_drive} not connected` : "Not installed"}</span>
            )}
            <span>{formatPlaytime(game.playtime_secs)} played</span>
            <span>Last played {formatLastPlayed(game.last_played).toLowerCase()}</span>
            {status && <span>{status}</span>}
            {stores.map((s) => (
              <span key={s} className="cx-store">
                <StoreIcon store={s} size={16} />
                {STORE_LABEL[s]}
              </span>
            ))}
          </p>
          <div className="cx-actions">
            <button
              ref={playRef}
              className="cx-play"
              data-nav
              disabled={!!game.offline_drive}
              onClick={() => onPrimary(game)}
            >
              <Icon name={game.offline_drive ? "folder" : game.installed ? "play" : "download"} size={22} />
              {game.offline_drive ? "Drive not connected" : game.installed ? "Play" : "Install"}
            </button>
            <button
              className="cx-icon"
              data-nav
              aria-label={game.favorite ? "Remove from favourites" : "Add to favourites"}
              aria-pressed={game.favorite}
              onClick={() => onToggleFavorite(game)}
            >
              <Icon name="star" size={22} />
            </button>
            <button
              ref={moreRef}
              className="cx-icon"
              data-nav
              aria-label="More options"
              aria-haspopup="menu"
              aria-expanded={menu !== null}
              onMouseDown={(e) => menu && e.stopPropagation()}
              onClick={(e) => {
                const r = e.currentTarget.getBoundingClientRect();
                setMenu((m) => (m ? null : r));
              }}
            >
              <Icon name="more" size={22} />
            </button>
          </div>
          <p className="cx-hints" aria-hidden>
            <span><kbd>A</kbd> Play</span>
            <span><kbd>Y</kbd> Options</span>
            <span><kbd>LB</kbd><kbd>RB</kbd> Shelves</span>
            <span><kbd>Start</kbd> Exit</span>
          </p>
        </section>
      )}

      {menu && game && (
        <GameMenu
          game={game}
          x={0}
          y={0}
          anchor={menu}
          onClose={() => {
            setMenu(null);
            tileFocus();
          }}
          onPrimary={onPrimary}
          onToggleFavorite={onToggleFavorite}
          onToggleHidden={onToggleHidden}
          onPanel={setPanel}
        />
      )}
      {panel === "artwork" && game && <ArtworkPicker game={game} onClose={() => setPanel(null)} onDone={() => setPanel(null)} />}
      {panel === "match" && game && <StoreMatch game={game} onClose={() => setPanel(null)} onDone={() => setPanel(null)} />}
    </div>
  );
}

/** The selected game's art, full-bleed, cross-fading as the selection moves. */
function ConsoleArt({ src }: { src: string | null }) {
  const [layers, setLayers] = useState<{ src: string; shown: boolean }[]>([]);
  useEffect(() => {
    if (!src) return setLayers([]);
    setLayers((cur) => (cur.at(-1)?.src === src ? cur : [...cur.slice(-1), { src, shown: false }]));
  }, [src]);
  return (
    <div className="cx-art" aria-hidden>
      {layers.map((l, i) => (
        <img
          key={l.src + i}
          src={l.src}
          alt=""
          className={l.shown ? "shown" : undefined}
          onLoad={() => setLayers((cur) => cur.map((x) => (x === l ? { ...x, shown: true } : x)))}
          onError={(e) => {
            const img = e.currentTarget;
            const alt = legacy(l.src);
            if (img.src !== alt) img.src = alt;
          }}
        />
      ))}
    </div>
  );
}
