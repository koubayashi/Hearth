import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { setPrefs, usePrefs } from "../lib/prefs";
import { STORE_LABEL, type Game } from "../lib/types";
import { pairKey, suggestCombines, versionsOf } from "../lib/versions";
import { Icon } from "./Icon";
import { StoreIcon } from "./StoreIcon";

// Any menu or panel can open a game's versions; App shows the one dialog.
let opener: ((g: Game) => void) | null = null;
export const openVersions = (g: Game) => opener?.(g);
let toast: (text: string) => void = () => {};

// The whole library, for suggestions in whichever panel is showing: App
// reloads it whenever the library changes.
let library: Game[] = [];
const listeners = new Set<() => void>();
export async function loadVersionSuggestions() {
  try {
    library = await api.listGames({ install: "all", stores: [], search: null, hidden_only: false });
    listeners.forEach((l) => l());
  } catch {
    // No suggestions this time.
  }
}
function useLibrary() {
  const [, rerender] = useState(0);
  useEffect(() => {
    const l = () => rerender((n) => n + 1);
    listeners.add(l);
    return () => void listeners.delete(l);
  }, []);
  return library;
}

/**
 * A game's versions (GTA V and GTA V Enhanced as one game): each can be
 * taken back out, and another game can be combined in.
 */
export function VersionsHost({ say }: { say: (text: string) => void }) {
  const [game, setGame] = useState<Game | null>(null);
  const [all, setAll] = useState<Game[]>([]);
  const [query, setQuery] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  toast = say;

  useEffect(() => {
    opener = (g) => {
      setGame(g);
      setQuery("");
      setProblem(null);
      api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then(setAll, () => setAll([]));
    };
    return () => {
      opener = null;
    };
  }, []);
  useEffect(() => {
    if (game) searchRef.current?.focus();
  }, [game?.id]);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!game || q.length < 2) return [];
    return all.filter((g) => g.id !== game.id && g.title.toLowerCase().includes(q)).slice(0, 6);
  }, [all, query, game]);

  if (!game) return null;
  const close = () => setGame(null);
  const vs = versionsOf(game);

  const combine = async (other: Game) => {
    setProblem(null);
    try {
      await api.combineGames(game.id, other.id);
      say(`${other.title} is now a version of ${game.title}`);
      close();
    } catch (e) {
      setProblem(String(e));
    }
  };
  const separate = async (entryId: number, title: string) => {
    setProblem(null);
    try {
      await api.separateVersion(game.id, entryId);
      say(`${title} is a game of its own again`);
      close();
    } catch (e) {
      setProblem(String(e));
    }
  };

  return (
    <div className="art-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div
        className="ways-dialog versions-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Versions of ${game.title}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
          }
        }}
      >
        <div className="match-head">
          <span>Versions · {game.title}</span>
          <button className="icon-btn small" data-nav aria-label="Close" onClick={close}>
            <Icon name="close" size={16} />
          </button>
        </div>
        <p className="note ways-intro">
          Versions of the same game (an original and its remaster or Enhanced edition) share one place in your library:
          one status, one playtime, one set of collections. Play starts the one you choose.
        </p>

        {vs.length > 0 && (
          <ul className="version-list">
            {vs.map((v) => (
              <li key={v.title}>
                <span className="version-name">
                  <b>{v.label}</b>
                  <small>
                    {v.title} · {v.installed ? "installed" : "not installed"}
                  </small>
                </span>
                <span className="version-stores">
                  {[...new Set(v.entries.map((e) => e.store))].map((s) => (
                    <StoreIcon key={s} store={s} size={14} />
                  ))}
                </span>
                <button className="btn" data-nav onClick={() => void separate(v.entries[0].id, v.title)} title={`Make ${v.title} its own game again, with its playtime`}>
                  Separate
                </button>
              </li>
            ))}
          </ul>
        )}

        <label className="version-search">
          <span>Combine another game into this one</span>
          <input
            ref={searchRef}
            type="search"
            placeholder="Search your library…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        {matches.length > 0 && (
          <ul className="version-list">
            {matches.map((g) => (
              <li key={g.id}>
                <span className="version-name">
                  <b>{g.title}</b>
                  <small>{[...new Set(g.entries.map((e) => STORE_LABEL[e.store]))].join(", ")}</small>
                </span>
                <span />
                <button className="btn" data-nav onClick={() => void combine(g)}>
                  Combine
                </button>
              </li>
            ))}
          </ul>
        )}

        {problem && (
          <p className="note error" role="alert">
            {problem}
          </p>
        )}
        <div className="way-actions">
          <button className="btn btn-primary" data-nav onClick={close}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * In a game's panel, when another game in the library looks like a version
 * of it: one quiet question, answered once.
 */
export function CombineSuggestion({ game }: { game: Game }) {
  const p = usePrefs();
  const all = useLibrary();
  const [busy, setBusy] = useState(false);
  const pairs = useMemo(() => suggestCombines(all, p.dismissedCombines), [all, p.dismissedCombines]);
  const pair = pairs.find(([k, o]) => k.id === game.id || o.id === game.id);
  const say = toast;
  if (!pair) return null;
  const [keep, other] = pair;
  const shown = keep.id === game.id ? other : keep;
  return (
    <div className="combine-note" role="note">
      <p>
        <b>{shown.title}</b> is also in your library. Is it a version of the same game?
      </p>
      <div>
        <button
          className="btn"
          data-nav
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            try {
              await api.combineGames(keep.id, other.id);
              say(`${other.title} is now a version of ${keep.title}`);
            } catch (e) {
              say(`Couldn't combine them: ${e}`);
            } finally {
              setBusy(false);
            }
          }}
        >
          Combine
        </button>
        <button className="link" data-nav onClick={() => setPrefs({ dismissedCombines: [...p.dismissedCombines, pairKey(keep.id, other.id)] })}>
          No, different games
        </button>
      </div>
    </div>
  );
}
