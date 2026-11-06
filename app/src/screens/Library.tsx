import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import type { Game, GameStatus, InstallFilter, NowPlaying, StoreId, Via } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { formatHoursShort, formatLastPlayed, formatPlaytime } from "../lib/format";
import { setPrefs, usePrefs } from "../lib/prefs";
import { savedInfo, useSavedInfo } from "../lib/gameInfo";
import { coverMeta, genresOf, narrowGames, NO_NARROW, sortGames, SORTS, type InfoOf, type Narrow } from "../lib/arrange";
import { jumpBackIn } from "../lib/ways";
import { ArrangeMenu } from "../components/ArrangeMenu";
import { Art } from "../components/Art";
import { CoverGrid, isMenuKey } from "../components/CoverGrid";
import { GameMenu, type PanelAction } from "../components/GameMenu";
import { Icon } from "../components/Icon";
import { HiddenBar } from "../components/HiddenBar";
import { Poster } from "../components/Poster";
import { StoreIcon } from "../components/StoreIcon";

const FILTERS: { id: InstallFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "installed", label: "Installed" },
  { id: "notinstalled", label: "Not installed" },
];

/** Narrows the library to one status or collection (the Collections page). */
export interface LibraryScope {
  status?: GameStatus;
  collection?: number;
  /** Only starred games (the Favourites shelf). */
  favorites?: boolean;
  /** Only these games (a series or company shelf). */
  ids?: Set<number>;
  /** Instead of games, this (the cards of every series or company), given the search. */
  browse?: (search: string) => ReactNode;
  title: string;
  /** Shown under the toolbar, above the games. */
  header?: ReactNode;
  emptyText: string;
}

export function Library({
  hiddenView,
  onLeaveHidden,
  scope,
  version,
  focused,
  playing,
  onFocusGame,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
  revealId,
  hiddenCount = 0,
  onShowHidden,
}: {
  /** Scroll this game into view once the library has loaded (opened from Stats). */
  revealId?: number | null;
  /** For the way to hidden games in the sort and filter menu. */
  hiddenCount?: number;
  onShowHidden?: () => void;
  hiddenView: boolean;
  /** Leaves the hidden view. */
  onLeaveHidden?: () => void;
  scope?: LibraryScope;
  /** Bumped by the parent whenever library data changes. */
  version: number;
  focused: Game | null;
  playing: NowPlaying[];
  onFocusGame: (g: Game | null) => void;
  onPrimary: (g: Game, via?: Via) => void;
  onToggleHidden: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
}) {
  const prefs = usePrefs();
  const scrollRef = useRef<HTMLDivElement>(null);
  const wallRef = useRef<HTMLDivElement>(null);
  const barRef = useRef<HTMLDivElement>(null);
  // The toolbar floats over the scroll area; its height sets the space and
  // the fade at the top of that area.
  useEffect(() => {
    const bar = barRef.current;
    const wall = wallRef.current;
    if (!bar || !wall) return;
    const ro = new ResizeObserver(() => wall.style.setProperty("--bar-h", `${bar.offsetHeight}px`));
    ro.observe(bar);
    return () => ro.disconnect();
  }, []);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const [install, setInstall] = useState<InstallFilter>("all");
  const [stores, setStores] = useState<StoreId[]>([]);
  const [search, setSearch] = useState("");
  const [listed, setGames] = useState<Game[] | null>(null);
  // Favourites and genres narrow what the store query returned, here: they
  // need the saved store details, which only the front end holds.
  const [narrow, setNarrow] = useState<Narrow>(NO_NARROW);
  const [browseSearch, setBrowseSearch] = useState("");
  const [arrangeAt, setArrangeAt] = useState<DOMRect | null>(null);
  const sort = prefs.librarySort;
  const infoTick = useSavedInfo();
  const info: InfoOf = (id) => {
    const i = savedInfo(id);
    return i && { details: i.details, bytes: i.install?.bytes ?? null };
  };
  const games = useMemo(
    () => {
      if (!listed) return null;
      const inScope = scope?.ids ? listed.filter((g) => scope.ids!.has(g.id)) : listed;
      return sortGames(narrowGames(inScope, { ...narrow, favorites: narrow.favorites || !!scope?.favorites }, info), sort, info);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [listed, narrow, sort, infoTick, scope?.favorites, scope?.ids],
  );
  const [loadError, setLoadError] = useState<string | null>(null);
  const [all, setAll] = useState<Game[]>([]);
  const [reveal, setReveal] = useState<{ id: number; nonce: number } | null>(null);
  const [menu, setMenu] = useState<{ game: Game; x: number; y: number } | null>(null);
  const [panel, setPanel] = useState<{ action: PanelAction; nonce: number } | null>(null);
  const openMenu = (g: Game, x: number, y: number) => {
    onFocusGame(g);
    setMenu({ game: g, x, y });
  };

  // With nothing filtered, the shown list is also the unfiltered one (for the
  // store chips and "Jump back in"), so one call does for both.
  const unfiltered = install === "all" && stores.length === 0 && !search && !hiddenView && !scope?.status && scope?.collection == null;
  useEffect(() => {
    let live = true;
    api
      .listGames({
        install,
        stores,
        search: search || null,
        hidden_only: hiddenView,
        status: scope?.status ?? null,
        collection: scope?.collection ?? null,
      })
      .then((g) => {
        if (!live) return;
        setGames(g);
        if (unfiltered) setAll(g);
        setLoadError(null);
      })
      .catch((e) => live && setLoadError(String(e)));
    return () => {
      live = false;
    };
  }, [install, stores, search, hiddenView, version, scope?.status, scope?.collection]);

  useEffect(() => {
    if (unfiltered) return;
    let live = true;
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then((g) => live && setAll(g));
    return () => {
      live = false;
    };
  }, [version, unfiltered]);

  const recent = useMemo(() => jumpBackIn(all), [all]);
  const ownedStores = useMemo(
    () => [...new Set(all.flatMap((g) => g.entries.map((e) => e.store)))],
    [all],
  );

  const filtered = install !== "all" || stores.length > 0 || search !== "" || narrow.favorites || narrow.genres.length > 0;
  const genres = useMemo(() => (arrangeAt ? genresOf(all, info) : []), [arrangeAt, all, infoTick]);

  // Something should always be in the light: default to the most recent game.
  useEffect(() => {
    if (focused && games?.some((g) => g.id === focused.id)) {
      const fresh = games.find((g) => g.id === focused.id)!;
      if (fresh !== focused) onFocusGame(fresh);
      return;
    }
    if (!games) return;
    // A search or filter that matches nothing keeps the last game in view
    // rather than blanking the poster while you type.
    if (games.length === 0 && filtered && focused) return;
    const showRecent = prefs.showRecent && !hiddenView && !scope;
    onFocusGame((showRecent ? recent[0] : undefined) ?? games.find((g) => g.installed) ?? games[0] ?? null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [games, recent]);

  // A random game from what's on screen, preferring ones you can play right now.
  const surprise = () => {
    const pool = games ?? [];
    const playable = pool.filter((g) => g.installed);
    const from = (playable.length > 0 ? playable : pool).filter((g) => g.id !== focused?.id);
    const pick = from[Math.floor(Math.random() * from.length)] ?? pool[0];
    if (!pick) return;
    onFocusGame(pick);
    setReveal({ id: pick.id, nonce: Date.now() });
  };

  // Arriving with a game to show: bring its cover into view once it's listed.
  const revealed = useRef(false);
  useEffect(() => {
    if (revealed.current || !revealId || !games?.some((g) => g.id === revealId)) return;
    revealed.current = true;
    setReveal({ id: revealId, nonce: Date.now() });
  }, [games, revealId]);

  const toggleStore = (s: StoreId) =>
    setStores((cur) => (cur.includes(s) ? cur.filter((x) => x !== s) : [...cur, s]));

  const activate = (g: Game) => {
    onFocusGame(g);
    primaryRef.current?.focus();
  };
  const backToFocused = () => {
    if (!focused) return;
    document.querySelector<HTMLElement>(`[data-game="${focused.id}"]`)?.focus();
  };

  // The row of wide art is the library's front page: only when nothing is filtered.
  const showRecent = prefs.showRecent && !hiddenView && !scope && !filtered && recent.length > 0;
  const tileProps = { focusedId: focused?.id ?? null, onFocusGame, onPrimary, onActivate: activate, onMenu: openMenu, showStoreIcons: prefs.showStoreIcons };

  // Installed games first, then everything you own but haven't installed.
  const sections = useMemo(() => {
    const list = games ?? [];
    if (hiddenView) return [{ title: "Hidden games", games: list }];
    if (scope) return [{ title: scope.title, games: list }];
    // Favourites have no section of their own: the toolbar's ★ chip shows
    // them, and each game stays with the installed or not-installed games.
    return [
      { title: "Library", games: list.filter((g) => g.installed) },
      // Installed, but on a drive that's unplugged right now.
      { title: "Drive not connected", games: list.filter((g) => !g.installed && g.offline_drive) },
      { title: "Not installed", games: list.filter((g) => !g.installed && !g.offline_drive) },
    ].filter((s) => s.games.length > 0);
  }, [games, hiddenView, scope?.title]);

  return (
    // A view with nothing in it (no hidden games, an empty collection) drops
    // the poster column instead of showing an empty frame beside the message.
    <div className={`library${games?.length === 0 && !focused ? " solo" : ""}${hiddenView ? " hidden-view" : ""}`}>
      {(games?.length !== 0 || focused) && (
      <Poster
        ref={primaryRef}
        game={focused}
        playingSince={playing.find((p) => p.game_id === focused?.id)?.started_at ?? null}
        onPrimary={onPrimary}
        onToggleHidden={onToggleHidden}
        onToggleFavorite={onToggleFavorite}
        onBack={backToFocused}
        request={panel}
        plain={hiddenView}
      />
      )}
      {menu && (
        <GameMenu
          game={menu.game}
          x={menu.x}
          y={menu.y}
          onClose={() => {
            // Back to the cover the menu came from, for keyboard and controller.
            document.querySelector<HTMLElement>(`[data-game="${menu.game.id}"]`)?.focus();
            setMenu(null);
          }}
          onPrimary={onPrimary}
          onToggleFavorite={onToggleFavorite}
          onToggleHidden={onToggleHidden}
          onPanel={(action) => setPanel({ action, nonce: Date.now() })}
        />
      )}
    <div className="wall" ref={wallRef}>
      <div className={`top${scope?.browse ? " browsing" : ""}`} role="toolbar" aria-label="Library filters" ref={barRef}>
        <label className={`search ${search ? "has-value" : ""}`} title="Search">
          <Icon name="search" size={16} />
          {scope?.browse ? (
            <input
              data-nav
              placeholder={`Search ${scope.title.toLowerCase()}`}
              value={browseSearch}
              onChange={(e) => setBrowseSearch(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setBrowseSearch("")}
              aria-label={`Search ${scope.title.toLowerCase()}`}
            />
          ) : (
            <input
              data-nav
              placeholder={hiddenView ? "Search hidden" : "Search"}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setSearch("")}
              aria-label="Search games"
            />
          )}
        </label>
        {/* Cards of series or companies: only the search applies to them. */}
        {!scope?.browse && (
        <>
        <div className="seg" role="group" aria-label="Install state">
          {FILTERS.map((f) => (
            <button key={f.id} data-nav aria-pressed={install === f.id} onClick={() => setInstall(f.id)}>
              {f.label}
            </button>
          ))}
        </div>
        <div className="chips">
        {!scope?.favorites && (
          <button
            className="chip fav"
            data-nav
            title="Only favourites"
            aria-pressed={narrow.favorites}
            onClick={() => setNarrow((n) => ({ ...n, favorites: !n.favorites }))}
          >
            <Icon name="star" size={15} />
            <span className="label">Favourites</span>
          </button>
        )}
        {ownedStores.map((s) => (
          <button
            key={s}
            className="chip"
            data-nav
            title={STORE_LABEL[s]}
            aria-pressed={stores.includes(s)}
            onClick={() => toggleStore(s)}
          >
            <StoreIcon store={s} size={15} />
            <span className="label">{STORE_LABEL[s]}</span>
          </button>
        ))}
        </div>
        <span className="spacer" />
        <button
          className={`chip arrange${narrow.genres.length > 0 || sort !== "title" ? " active" : ""}`}
          data-nav
          title="Sort and filter"
          aria-haspopup="menu"
          aria-expanded={arrangeAt !== null}
          onMouseDown={(e) => arrangeAt && e.stopPropagation()}
          onClick={(e) => {
            const at = e.currentTarget.getBoundingClientRect();
            setArrangeAt((cur) => (cur ? null : at));
          }}
        >
          <Icon name="sliders" size={15} />
          <span className="label">
            {SORTS.find((s) => s.id === sort)?.label}
            {narrow.genres.length > 0 && ` · ${narrow.genres.length === 1 ? narrow.genres[0] : `${narrow.genres.length} genres`}`}
          </span>
        </button>
        {arrangeAt && (
          <ArrangeMenu
            anchor={arrangeAt}
            sort={sort}
            narrow={narrow}
            genres={genres}
            hiddenCount={hiddenCount}
            onSort={(s) => setPrefs({ librarySort: s })}
            onNarrow={setNarrow}
            onShowHidden={hiddenView ? undefined : onShowHidden}
            onClose={() => setArrangeAt(null)}
          />
        )}
        <button
          className="surprise"
          data-nav
          title="Pick a random game"
          aria-label="Pick a random game"
          disabled={!games || games.length === 0}
          onClick={surprise}
        >
          <Icon name="dice" size={16} />
          <span className="label">Surprise me</span>
        </button>
        </>
        )}
      </div>

    <div className="scroll" ref={scrollRef}>
      {hiddenView && onLeaveHidden && <HiddenBar count={games?.length ?? null} onBack={onLeaveHidden} />}
      {scope?.header}
      <div className="page">
        <div>
          {showRecent && <TileRow title="Jump back in" games={recent} {...tileProps} />}

          {scope?.browse ? (
            <>
              <h2>{scope.title}</h2>
              {scope.browse(browseSearch)}
            </>
          ) : loadError ? (
            <div className="empty" role="alert">
              <h3>Couldn't load your library</h3>
              {loadError}
            </div>
          ) : games && games.length === 0 ? (
            <>
            <h2>{hiddenView ? "Hidden games" : (scope?.title ?? "Library")}</h2>
            <div className="empty">
              {scope && !filtered ? (
                <>
                  <h3>Nothing here yet</h3>
                  {scope.emptyText}
                </>
              ) : hiddenView ? (
                <>
                  <h3>Nothing hidden</h3>
                  Games you hide from the library land here, and you can bring them back any time.
                </>
              ) : filtered ? (
                <>
                  <h3>No games match</h3>
                  Try a different search, or clear the filters above.
                </>
              ) : (
                <>
                  <h3>Your library is empty</h3>
                  Connect a store in Settings and your games will show up here.
                </>
              )}
            </div>
            </>
          ) : (
            sections.map((s) => (
              <section key={s.title} aria-label={s.title}>
                <h2>
                  {s.title}
                  <small>
                    {s.games.length} {s.games.length === 1 ? "game" : "games"} · {SORTS.find((x) => x.id === sort)?.short}
                  </small>
                </h2>
                <div className="section-grid">
                  <CoverGrid
                    meta={(g) => coverMeta(g, sort, info)}
                    reveal={reveal}
                    onContext={openMenu}
                    games={s.games}
                    focusedId={focused?.id ?? null}
                    scrollRef={scrollRef}
                    onFocusGame={onFocusGame}
                    onActivate={activate}
                  />
                </div>
              </section>
            ))
          )}
        </div>
      </div>
    </div>
    </div>
    </div>
  );
}

/** A row of wide art ("Jump back in", "Favourites"): as many 260px+ tiles as
 *  fit on one line, never a partial second row. */
function TileRow({
  title,
  games,
  focusedId,
  showStoreIcons,
  onFocusGame,
  onPrimary,
  onActivate,
  onMenu,
}: {
  title: string;
  games: Game[];
  focusedId: number | null;
  showStoreIcons: boolean;
  onFocusGame: (g: Game) => void;
  onPrimary: (g: Game) => void;
  onActivate: (g: Game) => void;
  onMenu: (g: Game, x: number, y: number) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [cols, setCols] = useState(3);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const gap = 24;
    const ro = new ResizeObserver(() => setCols(Math.max(1, Math.floor((el.clientWidth + gap) / (260 + gap)))));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <>
      <h2>{title}</h2>
      <div className="recent" ref={ref} style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
        {games.slice(0, cols).map((g) => (
          <button
            key={g.id}
            data-nav
            data-game={g.id}
            className={`piece ${focusedId === g.id ? "lit" : ""}`}
            aria-label={`${g.title}, played ${formatHoursShort(g.playtime_secs) || "never"}`}
            onFocus={() => onFocusGame(g)}
            onClick={() => onFocusGame(g)}
            onDoubleClick={() => onPrimary(g)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                onActivate(g);
              } else if (isMenuKey(e)) {
                e.preventDefault();
                const r = e.currentTarget.getBoundingClientRect();
                onMenu(g, r.left + r.width / 2, r.top + r.height / 3);
              }
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              onMenu(g, e.clientX, e.clientY);
            }}
          >
            <span className="frame">
              <Art src={g.hero} title={g.title} />
            </span>
            <span className="plaque">
              <b>
                {showStoreIcons && <StoreIcon store={g.entries[0]?.store ?? "manual"} size={13} />}
                <span className="plaque-title">{g.title}</span>
              </b>
              <span>
                {g.playtime_secs > 0 && `${formatPlaytime(g.playtime_secs)} · `}
                {formatLastPlayed(g.last_played)}
              </span>
            </span>
          </button>
        ))}
      </div>
    </>
  );
}
