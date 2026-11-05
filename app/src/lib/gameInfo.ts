import { useEffect, useReducer } from "react";
import { api } from "./api";
import type { Game, GameDetails, InstallInfo } from "./types";

interface Info {
  details: GameDetails | null;
  install: InstallInfo | null;
}

// Every game's saved details and install info, kept in memory so selecting a
// game draws the whole panel in its first frame. Asking per selection (after a
// pause, so scrolling didn't fire a lookup per cover) left the panel filling in
// piece by piece and briefly showing the previous game's details.
const cache = new Map<number, Info>();
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((l) => l());

/** Loads every game's saved info in one call: from disk, no network. */
export async function preloadGameInfo() {
  try {
    const all = await api.savedGameInfo();
    for (const i of all) cache.set(i.id, { details: i.details, install: i.install });
    changed();
  } catch {
    // The panel still asks per game.
  }
}

/** Measures a game's install folder (again, with `refresh`) and keeps the
 *  result, so every page showing sizes sees it. */
export async function measureInstall(id: number, refresh: boolean) {
  const install = await api.installInfo(id, refresh);
  const old = cache.get(id);
  cache.set(id, { details: old?.details ?? null, install });
  changed();
  return install;
}

/** What's in memory for a game right now, without asking (for sorting and filters). */
export const savedInfo = (id: number): Info | undefined => cache.get(id);

/** Re-renders whenever more saved info arrives. */
export function useSavedInfo() {
  const [n, rerender] = useReducer((x: number) => x + 1, 0);
  useEffect(() => {
    listeners.add(rerender);
    return () => void listeners.delete(rerender);
  }, []);
  return n;
}

/** Store-page details and install info for a game: what's in memory at once,
 *  then a fresh copy for this game (which may fetch it if never seen). */
export function useGameInfo(game: Game | null, nonce: number) {
  const [, rerender] = useReducer((n: number) => n + 1, 0);
  const id = game?.id ?? null;
  const installed = game?.installed ?? false;

  useEffect(() => {
    listeners.add(rerender);
    return () => void listeners.delete(rerender);
  }, []);

  useEffect(() => {
    if (id === null) return;
    let live = true;
    Promise.all([
      api.gameDetails(id).catch(() => cache.get(id)?.details ?? null),
      installed ? api.installInfo(id, false).catch(() => cache.get(id)?.install ?? null) : Promise.resolve(null),
    ]).then(([details, install]) => {
      const old = cache.get(id);
      cache.set(id, { details, install });
      // Only redraw if something is new: usually it's what's already shown.
      if (live && JSON.stringify(old) !== JSON.stringify({ details, install })) rerender();
    });
    return () => {
      live = false;
    };
  }, [id, installed, nonce]);

  const info = id === null ? undefined : cache.get(id);
  return { details: info?.details ?? null, install: installed ? (info?.install ?? null) : null };
}
