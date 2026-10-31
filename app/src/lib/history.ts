// One game's play history, from the sessions Hearth recorded. Store totals
// have no dates, so this is only what Hearth timed itself.

import type { PlaySession } from "./types";
import { weekStart, WEEKS_SHOWN } from "./stats";

export interface Month {
  /** "2026-10", for keys. */
  key: string;
  /** Local midnight on the 1st, unix seconds. */
  start: number;
  secs: number;
  /** Newest first. */
  sessions: PlaySession[];
}

export interface History {
  secs: number;
  count: number;
  first: number | null;
  longest: PlaySession | null;
  /** Newest month first. */
  months: Month[];
  /** Seconds per week, oldest first, ending with this week. */
  weeks: number[];
}

/** A session this long was probably a game left running. */
export const LONG_SESSION = 8 * 3600;

const length = (s: PlaySession) => Math.max(0, s.ended_at - s.started_at);

export function gameHistory(sessions: PlaySession[], gameId: number, nowMs = Date.now()): History {
  const mine = sessions.filter((s) => s.game_id === gameId).sort((a, b) => b.started_at - a.started_at);
  const months: Month[] = [];
  for (const s of mine) {
    const d = new Date(s.started_at * 1000);
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
    let m = months[months.length - 1];
    if (m?.key !== key) {
      m = { key, start: new Date(d.getFullYear(), d.getMonth(), 1).getTime() / 1000, secs: 0, sessions: [] };
      months.push(m);
    }
    m.secs += length(s);
    m.sessions.push(s);
  }

  const thisWeek = weekStart(Math.floor(nowMs / 1000));
  const weeks = Array.from({ length: WEEKS_SHOWN }, () => 0);
  for (const s of mine) {
    // Count whole weeks between the two Mondays; days can be 23 or 25 hours.
    const ago = Math.round((thisWeek - weekStart(s.started_at)) / (7 * 86400));
    if (ago >= 0 && ago < WEEKS_SHOWN) weeks[WEEKS_SHOWN - 1 - ago] += length(s);
  }

  return {
    secs: mine.reduce((t, s) => t + length(s), 0),
    count: mine.length,
    first: mine.length ? mine[mine.length - 1].started_at : null,
    longest: mine.reduce<PlaySession | null>((best, s) => (!best || length(s) > length(best) ? s : best), null),
    months,
    weeks,
  };
}
