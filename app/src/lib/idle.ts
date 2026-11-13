import { formatPlaytime } from "./format";
import type { Game, IdleMode } from "./types";

/** Idle time under this is a short break, not worth a mention. */
export const IDLE_SHOWN_FROM = 10 * 60;

/** How a game's idle time is treated: its own choice, or the setting. */
export function idleModeOf(game: Game, setting: IdleMode): IdleMode {
  return game.idle_mode ?? setting;
}

/**
 * "38h active · 2h idle" for a game whose idle time is kept separate, once
 * there's enough of it to mention. `live` is the running session's time.
 */
export function idleSplit(game: Game, setting: IdleMode, live = 0): string | null {
  const idle = game.idle_secs ?? 0;
  if (idleModeOf(game, setting) !== "separate" || idle < IDLE_SHOWN_FROM) return null;
  const active = Math.max(game.playtime_secs + live - idle, 0);
  return `${formatPlaytime(active)} active · ${formatPlaytime(idle)} idle`;
}
