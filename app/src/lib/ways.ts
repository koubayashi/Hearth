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

/** A game found in a folder that's since been deleted: no store can install
 * it, so it stays in the library (playtime and all) with nothing to start. */
export const folderGone = (g: Game) =>
  !g.installed && !g.offline_drive && !defaultWay(g) && launchEntry(g)?.store === "manual";

export const FOLDER_GONE_HINT = "Put it back in a game folder Hearth watches and it can be played again.";

/** The way to play a launch will use: the one picked, else the default. */
export function chosenWay(g: Game, via?: Via): LaunchOption | null {
  if (via?.kind === "store") return null;
  if (via?.kind === "option") return g.launch_options.find((o) => o.id === via.id) ?? null;
  return defaultWay(g);
}
