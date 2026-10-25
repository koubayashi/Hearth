// Ways to play: which one the Play button uses, and what to call it.

import type { Game, LaunchOption, Via } from "./types";
import { STORE_LABEL } from "./types";

/** The way to play the Play button uses, or null for the store's own launch. */
export const defaultWay = (g: Game): LaunchOption | null => g.launch_options.find((o) => o.is_default) ?? null;

/** The store Hearth launches or installs through. */
export const storeName = (g: Game) =>
  STORE_LABEL[(g.entries.find((e) => e.installed && !e.offline) ?? g.entries[0])?.store ?? "manual"];

/** The way to play a launch will use: the one picked, else the default. */
export function chosenWay(g: Game, via?: Via): LaunchOption | null {
  if (via?.kind === "store") return null;
  if (via?.kind === "option") return g.launch_options.find((o) => o.id === via.id) ?? null;
  return defaultWay(g);
}
