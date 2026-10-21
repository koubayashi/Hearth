import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, InstallFilter, StoreId } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { formatHoursShort } from "../lib/format";
import { usePrefs } from "../lib/prefs";
import { Art, legacy } from "../components/Art";
import { CoverGrid } from "../components/CoverGrid";
import { Icon } from "../components/Icon";
import { WallLabel } from "../components/WallLabel";

const FILTERS: { id: InstallFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "installed", label: "Installed" },
  { id: "notinstalled", label: "Not installed" },
];

export function Library({
  hiddenView,
  version,
  focused,
  onFocusGame,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
}: {
  hiddenView: boolean;
  /** Bumped by the parent whenever library data changes. */
  version: number;
  focused: Game | null;
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
  const [all, setAll] = useState<Game[]>([]);

  useEffect(() => {
    let live = true;
    api
      .listGames({ install, stores, search: search || null, hidden_only: hiddenView })
      .then((g) => live && setGames(g));
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
        .slice(0, 5),
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
    onFocusGame((showRecent ? recent[0] : undefined) ?? games[0] ?? null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [games, recent]);

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

  return (
    <div className="scroll" ref={scrollRef}>
      <div className="top">
        <label className="search glass">
          <Icon name="search" size={16} />
          <input
            data-nav
            placeholder={`Search ${hiddenView ? "hidden games" : `${all.length} games`}`}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setSearch("")}
            aria-label="Search games"
          />
        </label>
        <div className="seg glass" role="group" aria-label="Install state">
          {FILTERS.map((f) => (
            <button key={f.id} data-nav aria-pressed={install === f.id} onClick={() => setInstall(f.id)}>
              {f.label}
            </button>
          ))}
        </div>
        {ownedStores.map((s) => (
          <button key={s} className="chip glass" data-nav aria-pressed={stores.includes(s)} onClick={() => toggleStore(s)}>
            {STORE_LABEL[s]}
          </button>
        ))}
      </div>

      <div className="page">
        <div>
          <section className="feature" aria-hidden>
            {focused &&
              (focused.logo ? (
                <FeatureLogo key={focused.id} src={focused.logo} title={focused.title} />
              ) : (
                <div className="wordmark">{focused.title}</div>
              ))}
          </section>

          {showRecent && (
            <>
              <h2>Jump back in</h2>
              <div className="recent">
                {recent.map((g) => (
                  <button
                    key={g.id}
                    data-nav
                    data-game={g.id}
                    className={`tile ${focused?.id === g.id ? "lit" : ""}`}
                    aria-label={`${g.title}, played ${formatHoursShort(g.playtime_secs) || "never"}`}
                    onFocus={() => onFocusGame(g)}
                    onClick={() => onFocusGame(g)}
                    onDoubleClick={() => onPrimary(g)}
                    onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), activate(g))}
                  >
                    <Art src={g.hero} title={g.title} />
                    <span className="cap">
                      {g.title}
                      <span>{formatHoursShort(g.playtime_secs) || "New"}</span>
                    </span>
                  </button>
                ))}
              </div>
            </>
          )}

          <h2>
            {hiddenView ? "Hidden games" : "Library"}
            <small>{games ? `${games.length} ${games.length === 1 ? "game" : "games"} · A to Z` : ""}</small>
          </h2>
          {games && games.length === 0 ? (
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
          ) : (
            <CoverGrid
              games={games ?? []}
              focusedId={focused?.id ?? null}
              scrollRef={scrollRef}
              onFocusGame={onFocusGame}
              onActivate={activate}
            />
          )}
        </div>

        <WallLabel
          ref={primaryRef}
          game={focused}
          onPrimary={onPrimary}
          onToggleHidden={onToggleHidden}
          onToggleFavorite={onToggleFavorite}
          onBack={backToFocused}
        />
      </div>
    </div>
  );
}

function FeatureLogo({ src, title }: { src: string; title: string }) {
  const urls = [...new Set([src, legacy(src)])];
  const [attempt, setAttempt] = useState(0);
  if (attempt >= urls.length) return <div className="wordmark">{title}</div>;
  return <img className="logo" src={urls[attempt]} alt="" onError={() => setAttempt((a) => a + 1)} />;
}
