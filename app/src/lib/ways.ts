// Ways to play: which one the Play button uses, and what to call it.

import type { Game, LaunchOption, Via } from "./types";
import { STORE_LABEL } from "./types";

/** The way to play the Play button uses, or null for the store's own launch. */
export const defaultWay = (g: Game): LaunchOption | null => g.launch_options.find((o) => o.is_default) ?? null;

/** The entry Hearth launches through, else one a store can install from. */
const launchEntry = (g: Game) =>
  g.entries.find((e) => e.installed && !e.offline) ?? g.entries.find((e) => e.store !== "manual") ?? g.entries[0];

/** The store Hearth launches or installs through. */
export const storeName = (g: Game) => STORE_LABEL[launchEntry(g)?.store ?? "manual"];

/** For a folder game, the program its Play starts: the one the user picked,
 * else the one the folder scan chose. Null for store games. */
export function folderProgram(g: Game): { path: string; name: string; picked: boolean } | null {
  const e = launchEntry(g);
  if (e?.store !== "manual" || !e.installed) return null;
  const path = e.launch_exe && e.install_dir ? `${e.install_dir.replace(/[\\/]$/, "")}\\${e.launch_exe}` : e.store_game_id;
  return { path, name: path.split(/[\\/]/).pop() ?? path, picked: !!e.launch_exe };
}

/** A game found in a folder that's since been deleted: no store can install
 * it, so it stays in the library (playtime and all) with nothing to start.
 * A default way to play still counts as a way in, unless its program went
 * with the folder. */
export const folderGone = (g: Game) =>
  !g.installed && !g.offline_drive && !(defaultWay(g) && !defaultWay(g)!.missing) && launchEntry(g)?.store === "manual";

/**
 * "Jump back in": the games you were last playing that you can pick up
 * again. Not ones you've uninstalled (their drive merely being unplugged is
 * fine), and not ones you've marked Completed or Abandoned: you're done with those.
 */
export function jumpBackIn(games: Game[], count = 6): Game[] {
  return games
    .filter((g) => g.last_played !== null && (g.installed || g.offline_drive) && g.status !== "completed" && g.status !== "abandoned")
    .sort((a, b) => b.last_played! - a.last_played!)
    .slice(0, count);
}

export const FOLDER_GONE_HINT = "Put it back in a game folder Hearth watches and it can be played again.";

/** The way to play a launch will use: the one picked, else the default. */
export function chosenWay(g: Game, via?: Via): LaunchOption | null {
  if (via?.kind === "store" || via?.kind === "entry") return null;
  if (via?.kind === "option") return g.launch_options.find((o) => o.id === via.id) ?? null;
  return defaultWay(g);
}
