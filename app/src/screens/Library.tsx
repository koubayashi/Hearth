import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, InstallFilter, NowPlaying, StoreId } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { formatHoursShort, formatLastPlayed, formatPlaytime } from "../lib/format";
import { usePrefs } from "../lib/prefs";
import { Art } from "../components/Art";
import { CoverGrid, isMenuKey } from "../components/CoverGrid";
import { GameMenu, type PanelAction } from "../components/GameMenu";
import { Icon } from "../components/Icon";
import { Poster } from "../components/Poster";
import { StoreIcon } from "../components/StoreIcon";

const FILTERS: { id: InstallFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "installed", label: "Installed" },
  { id: "notinstalled", label: "Not installed" },
];

export function Library({
  hiddenView,
  version,
  focused,
  playing,
  onFocusGame,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
}: {
  hiddenView: boolean;
  /** Bumped by the parent whenever library data changes. */
  version: number;
  focused: Game | null;
  playing: NowPlaying[];
  onFocusGame: (g: Game | null) => void;
  onPrimary: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
}) {
  const prefs = usePrefs();
  const scrollRef = useRef<HTMLDivElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const [install, setInstall] = useState<InstallFilter>("all");
  const [stores, setStores] = useState<StoreId[]>([]);
  const [search, setSearch] = useState("");
  const [games, setGames] = useState<Game[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [all, setAll] = useState<Game[]>([]);
  const [reveal, setReveal] = useState<{ id: number; nonce: number } | null>(null);
  const [menu, setMenu] = useState<{ game: Game; x: number; y: number } | null>(null);
  const [panel, setPanel] = useState<{ action: PanelAction; nonce: number } | null>(null);
  const openMenu = (g: Game, x: number, y: number) => {
    onFocusGame(g);
    setMenu({ game: g, x, y });
  };

  useEffect(() => {
    let live = true;
    api
      .listGames({ install, stores, search: search || null, hidden_only: hiddenView })
      .then((g) => {
        if (!live) return;
        setGames(g);
        setLoadError(null);
      })
      .catch((e) => live && setLoadError(String(e)));
    return () => {
      live = false;
    };
  }, [install, stores, search, hiddenView, version]);

  // Unfiltered list for the store chips and "Jump back in".
  useEffect(() => {
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then(setAll);
  }, [version]);

  const recent = useMemo(
    () =>
      all
        .filter((g) => g.last_played !== null)
        .sort((a, b) => b.last_played! - a.last_played!)
        .slice(0, 6),
    [all],
  );
  const ownedStores = useMemo(
    () => [...new Set(all.flatMap((g) => g.entries.map((e) => e.store)))],
    [all],
  );

  // Something should always be in the light: default to the most recent game.
  useEffect(() => {
    if (focused && games?.some((g) => g.id === focused.id)) {
      const fresh = games.find((g) => g.id === focused.id)!;
      if (fresh !== focused) onFocusGame(fresh);
      return;
    }
    if (!games) return;
    const showRecent = prefs.showRecent && !hiddenView;
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

  const showRecent = prefs.showRecent && !hiddenView && recent.length > 0;
  const filtered = install !== "all" || stores.length > 0 || search !== "";

  // Installed games first, then everything you own but haven't installed.
  const sections = useMemo(() => {
    const list = games ?? [];
    if (hiddenView) return [{ title: "Hidden games", games: list }];
    return [
      { title: "Library", games: list.filter((g) => g.installed) },
      { title: "Not installed", games: list.filter((g) => !g.installed) },
    ].filter((s) => s.games.length > 0);
  }, [games, hiddenView]);

  return (
    <div className="library">
      <Poster
        ref={primaryRef}
        game={focused}
        playingSince={playing.find((p) => p.game_id === focused?.id)?.started_at ?? null}
        onPrimary={onPrimary}
        onToggleHidden={onToggleHidden}
        onToggleFavorite={onToggleFavorite}
        onBack={backToFocused}
        request={panel}
      />
      {menu && (
        <GameMenu
          game={menu.game}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          onPrimary={onPrimary}
          onToggleFavorite={onToggleFavorite}
          onToggleHidden={onToggleHidden}
          onPanel={(action) => setPanel({ action, nonce: Date.now() })}
        />
      )}
    <div className="scroll" ref={scrollRef}>
      <div className="top" role="toolbar" aria-label="Library filters">
        <label className={`search ${search ? "has-value" : ""}`} title="Search">
          <Icon name="search" size={16} />
          <input
            data-nav
            placeholder={hiddenView ? "Search hidden" : "Search"}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setSearch("")}
            aria-label="Search games"
          />
        </label>
        <div className="seg" role="group" aria-label="Install state">
          {FILTERS.map((f) => (
            <button key={f.id} data-nav aria-pressed={install === f.id} onClick={() => setInstall(f.id)}>
              {f.label}
            </button>
          ))}
        </div>
        <div className="chips">
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
      </div>

      <div className="page">
        <div>
          {showRecent && (
            <>
              <h2>Jump back in</h2>
              <div className="recent">
                {recent.map((g) => (
                  <button
                    key={g.id}
                    data-nav
                    data-game={g.id}
                    className={`piece ${focused?.id === g.id ? "lit" : ""}`}
                    aria-label={`${g.title}, played ${formatHoursShort(g.playtime_secs) || "never"}`}
                    onFocus={() => onFocusGame(g)}
                    onClick={() => onFocusGame(g)}
                    onDoubleClick={() => onPrimary(g)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        activate(g);
                      } else if (isMenuKey(e)) {
                        e.preventDefault();
                        const r = e.currentTarget.getBoundingClientRect();
                        openMenu(g, r.left + r.width / 2, r.top + r.height / 3);
                      }
                    }}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      openMenu(g, e.clientX, e.clientY);
                    }}
                  >
                    <span className="frame">
                      <Art src={g.hero} title={g.title} />
                    </span>
                    <span className="plaque">
                      <b>
                        {prefs.showStoreIcons && <StoreIcon store={g.entries[0]?.store ?? "manual"} size={13} />}
                        <span className="plaque-title">{g.title}</span>
                      </b>
                      <span>
                        {formatPlaytime(g.playtime_secs)} · {formatLastPlayed(g.last_played)}
                      </span>
                    </span>
                  </button>
                ))}
              </div>
            </>
          )}

          {loadError ? (
            <div className="empty" role="alert">
              <h3>Couldn't load your library</h3>
              {loadError}
            </div>
          ) : games && games.length === 0 ? (
            <>
            <h2>{hiddenView ? "Hidden games" : "Library"}</h2>
            <div className="empty">
              {hiddenView ? (
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
                    {s.games.length} {s.games.length === 1 ? "game" : "games"} · A to Z
                  </small>
                </h2>
                <div className="section-grid">
                  <CoverGrid
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
  );
}
