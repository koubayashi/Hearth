import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Collection, Game, NowPlaying, Via } from "../lib/types";
import { defaultWay, folderGone, FOLDER_GONE_HINT } from "../lib/ways";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { formatLastPlayed, formatPlaytime, inSentence } from "../lib/format";
import { isFullscreen, setFullscreen } from "../lib/fullscreen";
import { usePrefs } from "../lib/prefs";
import { liveSecs } from "../lib/clock";
import { useGamerTag } from "../lib/gamertag";
import { Art, legacy } from "../components/Art";
import { isMenuKey } from "../components/CoverGrid";
import { GameMenu, type PanelAction } from "../components/GameMenu";
import { Icon } from "../components/Icon";
import { StoreIcon } from "../components/StoreIcon";
import { ArtworkPicker } from "../components/ArtworkPicker";
import { StoreMatch } from "../components/StoreMatch";
import { WaysToPlay } from "../components/WaysToPlay";
import { SmartEditor } from "../components/SmartEditor";
import { PlayHistory } from "../components/PlayHistory";
import { ConsoleStats } from "./ConsoleStats";
import { ConsoleSettings } from "./ConsoleSettings";

type Section = "games" | "stats" | "settings";
const SECTIONS: { id: Section; label: string }[] = [
  { id: "games", label: "Games" },
  { id: "stats", label: "Stats" },
  { id: "settings", label: "Settings" },
];

/** One row of games under a tab: LB/RB move between them. */
interface Shelf {
  id: string;
  label: string;
  games: Game[];
  /** Shown when the shelf is empty; only the first three can be. */
  empty: string;
  /** Index of the first game after the "Not installed" divider. */
  split?: number;
}

/** Recently played: what you can start right now first, then the rest of
 *  your history after a divider, each part most recent first. */
function recentShelf(all: Game[]): Shelf {
  const played = all.filter((g) => g.last_played !== null).sort((a, b) => b.last_played! - a.last_played!).slice(0, 20);
  const ready = played.filter((g) => g.installed);
  const rest = played.filter((g) => !g.installed);
  return {
    id: "recent",
    label: "Recently played",
    games: [...ready, ...rest],
    empty: "Games you play show up here.",
    split: ready.length > 0 && rest.length > 0 ? ready.length : undefined,
  };
}

/** The three fixed shelves, then favourites, each status and each collection
 *  that holds games, so nothing appears that would only say "empty". */
function buildShelves(all: Game[], collections: Collection[]): Shelf[] {
  const shelves: Shelf[] = [
    recentShelf(all),
    { id: "installed", label: "Installed", games: all.filter((g) => g.installed), empty: "No games installed." },
    { id: "all", label: "All games", games: all, empty: "Your library is empty. Connect a store in the desktop view." },
  ];
  const add = (id: string, label: string, games: Game[]) => games.length > 0 && shelves.push({ id, label, games, empty: "" });
  add("favorites", "Favourites", all.filter((g) => g.favorite));
  for (const s of STATUSES) add(`status-${s.id}`, s.label, all.filter((g) => g.status === s.id));
  for (const c of collections) add(`collection-${c.id}`, c.name, all.filter((g) => g.collections.includes(c.id)));
  return shelves;
}

/**
 * Fullscreen and controller-first, for the couch. A top menu (Games, Stats,
 * Settings) above one row of games under the selected game's art. D-pad or
 * stick to move, A to play, Y for options, LB/RB for the shelves, B to go
 * back, Start (or the Exit button) to leave.
 */
export function Console({
  version,
  playing,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
  onExit,
  onOpenDesktopSettings,
}: {
  version: number;
  playing: NowPlaying[];
  onPrimary: (g: Game, via?: Via) => void;
  onToggleHidden: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onExit: () => void;
  /** Leave the console for the desk Settings page. */
  onOpenDesktopSettings: () => void;
}) {
  const prefs = usePrefs();
  const [section, setSection] = useState<Section>("games");
  const name = useGamerTag();
  const [statsArt, setStatsArt] = useState<string | null>(null);
  const [games, setGames] = useState<Game[] | null>(null);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [tab, setTab] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [menu, setMenu] = useState<DOMRect | null>(null);
  // Opened from the arrow beside Play: straight on the ways to play.
  const [playMenu, setPlayMenu] = useState(false);
  const waysRef = useRef<HTMLButtonElement>(null);
  const [panel, setPanel] = useState<PanelAction | null>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  const playRef = useRef<HTMLButtonElement>(null);
  const moreRef = useRef<HTMLButtonElement>(null);

  // Fullscreen while the console is open (unless turned off in its
  // settings); back to how the window was after.
  useEffect(() => {
    let was = false;
    isFullscreen().then((f) => {
      was = f;
      if (!f && prefs.consoleFullscreen) void setFullscreen(true);
    });
    return () => {
      if (!was) void setFullscreen(false);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then(setGames).catch(() => setGames([]));
    api.listCollections().then(setCollections).catch(() => setCollections([]));
  }, [version]);

  const shelves = useMemo(() => buildShelves(games ?? [], collections), [games, collections]);
  // Start where there's something: recent games, else installed, else
  // everything. A shelf that empties (a collection deleted) falls back too.
  const [recentShelf, installedShelf, allShelf] = shelves;
  const shelf =
    shelves.find((s) => s.id === tab) ??
    (recentShelf.games.length > 0 ? recentShelf : installedShelf.games.length > 0 ? installedShelf : allShelf);
  const current = shelf.id;
  const row = shelf.games;
  const at = Math.min(index, Math.max(0, row.length - 1));
  const game = row[at] ?? null;
  // Only the tiles that can be on screen exist: a few behind the selection
  // (so they slide out rather than vanish) and enough after it to fill the
  // widest screen at the smallest tile size. A move touches a dozen tiles,
  // not the whole shelf.
  const first = Math.max(0, at - 3);
  const last = at + Math.ceil(window.innerWidth / 112) + 2;

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

  // With many collections the tabs scroll: keep the current one in view.
  useEffect(() => {
    document.querySelector(".cx-tab[aria-current]")?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [current, section]);

  const switchTab = (dir: 1 | -1) => {
    const i = shelves.findIndex((s) => s.id === current);
    const next = shelves[(i + dir + shelves.length) % shelves.length].id;
    setTab(next);
    setIndex(0);
    setMenu(null);
  };
  const backToGames = () => {
    setSection("games");
    requestAnimationFrame(tileFocus);
  };
  const move = (to: number) => {
    const i = Math.max(0, Math.min(row.length - 1, to));
    setIndex(i);
    rowRef.current?.querySelector<HTMLElement>(`[data-index="${i}"]`)?.focus({ preventScroll: true });
  };

  // Bumpers, B and Start work from anywhere in the console, except inside a
  // dialog. Bumpers change shelves in Games; B leaves Stats and Settings.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (menu || panel || e.defaultPrevented) return;
      if ((e.key === "PageUp" || e.key === "PageDown") && section === "games") {
        e.preventDefault();
        switchTab(e.key === "PageDown" ? 1 : -1);
      } else if (e.key === "Escape" && section !== "games") {
        e.preventDefault();
        backToGames();
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
  // The running session, counted in as it grows (the clock above ticks).
  const live = liveSecs(playingSince, Math.floor(now.getTime() / 1000));
  const played = game ? game.playtime_secs + live : 0;
  const status = game?.status ? STATUSES.find((s) => s.id === game.status)?.label : null;
  const stores = game ? [...new Set(game.entries.map((e) => e.store))] : [];

  // Games shows the selected game's art, Stats the most played game's;
  // Settings keeps whatever was behind it.
  const gameArt = game?.hero ?? game?.cover ?? null;

  return (
    <div className={`console${prefs.consoleHints ? "" : " no-hints"}`} data-section={section} aria-label="Console">
      <ConsoleArt src={section === "stats" ? statsArt : gameArt} />

      <header className="cx-top">
        {/* Moving along the menu switches section at once; Down goes into it. */}
        <nav className="cx-menu" aria-label="Console">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              className="cx-menu-item"
              data-nav
              aria-current={section === s.id ? "page" : undefined}
              onFocus={() => setSection(s.id)}
              onClick={() => setSection(s.id)}
            >
              {s.label}
            </button>
          ))}
        </nav>
        <span className="spacer" />
        {name && <span className="cx-name">{name}</span>}
        <time className="cx-clock">{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
        <button className="cx-exit" data-nav title="Leave the console (Start)" onClick={onExit}>
          <Icon name="close" size={16} />
          Exit
        </button>
      </header>

      {section === "stats" && <ConsoleStats version={version} onArt={setStatsArt} />}
      {section === "settings" && <ConsoleSettings onOpenDesktop={onOpenDesktopSettings} />}
      {section !== "games" && (
        <p className="cx-hints cx-page-hints" aria-hidden>
          {section === "settings" && (
            <span>
              <kbd>A</kbd> Change
            </span>
          )}
          <span>
            <kbd>B</kbd> Back to games
          </span>
        </p>
      )}

      {section === "games" && (
        <>
          <nav className="cx-tabs" aria-label="Shelves">
            <kbd className="cx-bumper" aria-hidden>
              LB
            </kbd>
            {shelves.map((s) => (
              <button
                key={s.id}
                className="cx-tab"
                aria-current={current === s.id ? "page" : undefined}
                data-nav
                onClick={() => {
                  setTab(s.id);
                  setIndex(0);
                }}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    tileFocus();
                  } else if (e.key === "ArrowUp") {
                    // Up is always the current section, wherever the tabs have scrolled.
                    e.preventDefault();
                    document.querySelector<HTMLElement>(".cx-menu-item[aria-current]")?.focus();
                  }
                }}
              >
                {s.label}
                <small>{s.games.length}</small>
              </button>
            ))}
            <kbd className="cx-bumper" aria-hidden>
              RB
            </kbd>
          </nav>

          <div
            className="cx-row"
            ref={rowRef}
            role="listbox"
            aria-label={shelf.label}
            onWheel={(e) => {
              const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
              if (Math.abs(d) > 20) move(at + Math.sign(d));
            }}
          >
            <div className="cx-track" style={{ "--at": at, "--past": shelf.split !== undefined && at >= shelf.split ? 1 : 0 } as React.CSSProperties}>
              {row.slice(first, last).map((g, n) => {
                const i = first + n;
                return (
                  <button
                    key={g.id}
                    className={`cx-tile${i === at ? " on" : i > at ? " after" : ""}${g.installed ? "" : " dim"}`}
                    style={{ "--i": i, "--s": shelf.split !== undefined && i >= shelf.split ? 1 : 0 } as React.CSSProperties}
                    role="option"
                    aria-selected={i === at}
                    aria-label={g.title}
                    aria-posinset={i + 1}
                    aria-setsize={row.length}
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
                        if (!g.offline_drive && !folderGone(g)) onPrimary(g);
                      } else if (isMenuKey(e)) {
                        e.preventDefault();
                        setMenu(moreRef.current?.getBoundingClientRect() ?? e.currentTarget.getBoundingClientRect());
                      }
                    }}
                  >
                    <Art src={g.cover ?? g.hero} title={g.title} />
                    {playing.some((p) => p.game_id === g.id) && <span className="cx-live" title="Playing now" />}
                  </button>
                );
              })}
              {/* Between the games you can start and the rest: its own element, so
                  it never grows with a selected cover. */}
              {shelf.split !== undefined && (
                <span
                  className={`cx-divider${at < shelf.split ? " after" : ""}`}
                  style={{ "--i": shelf.split } as React.CSSProperties}
                  aria-hidden
                >
                  <span>Not installed</span>
                </span>
              )}
              {games && row.length === 0 && <p className="cx-empty">{shelf.empty}</p>}
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
                  <span className="cx-now">{live < 60 ? "Playing now" : `Playing now · ${formatPlaytime(live)}`}</span>
                ) : (
                  <span>{game.installed ? "Ready to play" : game.offline_drive ? `Drive ${game.offline_drive} not connected` : "Not installed"}</span>
                )}
                {played > 0 && <span>{formatPlaytime(played)} played</span>}
                {game.last_played !== null && <span>Last played {inSentence(formatLastPlayed(game.last_played))}</span>}
                {played === 0 && game.last_played === null && <span>Never played</span>}
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
                  disabled={!!game.offline_drive || folderGone(game)}
                  title={folderGone(game) ? FOLDER_GONE_HINT : undefined}
                  onClick={() => onPrimary(game)}
                >
                  <Icon name={game.offline_drive || folderGone(game) ? "folder" : game.installed || defaultWay(game) ? "play" : "download"} size={22} />
                  {game.offline_drive ? (
                    "Drive not connected"
                  ) : folderGone(game) ? (
                    "Not installed"
                  ) : defaultWay(game) ? (
                    <>
                      Play <small className="via">{defaultWay(game)!.name}</small>
                    </>
                  ) : game.installed ? (
                    "Play"
                  ) : (
                    "Install"
                  )}
                </button>
                {game.launch_options.length > 0 && (
                  <button
                    ref={waysRef}
                    className="cx-icon"
                    data-nav
                    aria-label="Other ways to play"
                    aria-haspopup="menu"
                    onClick={(e) => {
                      setPlayMenu(true);
                      setMenu(e.currentTarget.getBoundingClientRect());
                    }}
                  >
                    <Icon name="chevronDown" size={22} />
                  </button>
                )}
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
        </>
      )}

      {menu && game && (
        <GameMenu
          game={game}
          x={0}
          y={0}
          anchor={menu}
          start={playMenu ? "play" : "main"}
          onClose={() => {
            setMenu(null);
            if (playMenu) waysRef.current?.focus();
            else tileFocus();
            setPlayMenu(false);
          }}
          onPrimary={onPrimary}
          onToggleFavorite={onToggleFavorite}
          onToggleHidden={onToggleHidden}
          onPanel={setPanel}
        />
      )}
      {panel === "artwork" && game && <ArtworkPicker game={game} onClose={() => setPanel(null)} onDone={() => {}} />}
      {panel === "ways" && game && <WaysToPlay game={game} onClose={() => setPanel(null)} />}
      {panel === "smart" && game && <SmartEditor game={game} onClose={() => setPanel(null)} />}
      {panel === "history" && game && <PlayHistory game={game} onClose={() => setPanel(null)} />}
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
