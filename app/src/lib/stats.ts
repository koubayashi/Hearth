// Everything the Stats page shows, computed from the library and Hearth's
// recorded sessions. Store totals have no dates, so anything over time
// (weeks, this year, streaks) comes from sessions only.

import type { Game, GameStatus, PlaySession, StoreId } from "./types";

export interface StoreShare {
  store: StoreId;
  secs: number;
  games: number;
}

export interface Week {
  /** Monday 00:00 local time, unix seconds. */
  start: number;
  secs: number;
  games: number;
}

export interface Stats {
  totalSecs: number;
  games: number;
  installed: number;
  played: number;
  /** Most played first, at most 10, only games with playtime. */
  top: { game: Game; secs: number }[];
  stores: StoreShare[];
  statuses: Record<GameStatus, number>;
  weeks: Week[];
  sessions: number;
  thisWeekSecs: number;
  thisMonthSecs: number;
  longest: { game: Game | null; secs: number } | null;
  year: { year: number; secs: number; games: number; top: Game | null } | null;
}

export const WEEKS_SHOWN = 26;
const WEEK = 7 * 86400;

/** Monday 00:00 (local) of the week containing `unix`. */
export function weekStart(unix: number): number {
  const d = new Date(unix * 1000);
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
  return Math.floor(d.getTime() / 1000);
}

/** The store a game's playtime is credited to: the one that reported the most. */
function homeStore(g: Game): StoreId {
  let best = g.entries[0];
  for (const e of g.entries) {
    if ((e.store_playtime_secs ?? 0) > (best?.store_playtime_secs ?? 0)) best = e;
  }
  return best?.store ?? "manual";
}

export function computeStats(games: Game[], sessions: PlaySession[], nowMs = Date.now()): Stats {
  const now = Math.floor(nowMs / 1000);
  const byId = new Map(games.map((g) => [g.id, g]));

  const stores = new Map<StoreId, StoreShare>();
  const statuses: Record<GameStatus, number> = { playing: 0, backlog: 0, completed: 0, abandoned: 0 };
  let totalSecs = 0;
  for (const g of games) {
    totalSecs += g.playtime_secs;
    if (g.status) statuses[g.status]++;
    const store = homeStore(g);
    const s = stores.get(store) ?? { store, secs: 0, games: 0 };
    s.secs += g.playtime_secs;
    s.games++;
    stores.set(store, s);
  }

  // Weekly buckets: a session counts in the week it started.
  const thisWeek = weekStart(now);
  const weeks: Week[] = [];
  for (let i = WEEKS_SHOWN - 1; i >= 0; i--) {
    // Step by calendar weeks via weekStart so DST shifts don't drift the grid.
    weeks.push({ start: weekStart(thisWeek - i * WEEK + 3 * 86400), secs: 0, games: 0 });
  }
  const weekGames = weeks.map(() => new Set<number>());
  const monthStart = new Date(nowMs);
  monthStart.setHours(0, 0, 0, 0);
  monthStart.setDate(1);
  const month = monthStart.getTime() / 1000;
  const year = new Date(nowMs).getFullYear();
  const yearStart = new Date(year, 0, 1).getTime() / 1000;

  let thisMonthSecs = 0;
  let longest: Stats["longest"] = null;
  const yearByGame = new Map<number, number>();
  for (const s of sessions) {
    const secs = Math.max(0, s.ended_at - s.started_at);
    const game = byId.get(s.game_id);
    if (!game) continue;
    const ws = weekStart(s.started_at);
    const i = weeks.findIndex((w) => w.start === ws);
    if (i >= 0) {
      weeks[i].secs += secs;
      weekGames[i].add(s.game_id);
    }
    if (s.started_at >= month) thisMonthSecs += secs;
    if (s.started_at >= yearStart) yearByGame.set(s.game_id, (yearByGame.get(s.game_id) ?? 0) + secs);
    if (!longest || secs > longest.secs) longest = { game, secs };
  }
  weeks.forEach((w, i) => (w.games = weekGames[i].size));

  let yearStats: Stats["year"] = null;
  if (yearByGame.size > 0) {
    let topId = -1;
    let topSecs = -1;
    let secs = 0;
    for (const [id, s] of yearByGame) {
      secs += s;
      if (s > topSecs) [topId, topSecs] = [id, s];
    }
    yearStats = { year, secs, games: yearByGame.size, top: byId.get(topId) ?? null };
  }

  return {
    totalSecs,
    games: games.length,
    installed: games.filter((g) => g.installed).length,
    played: games.filter((g) => g.playtime_secs > 0).length,
    top: games
      .filter((g) => g.playtime_secs > 0)
      .sort((a, b) => b.playtime_secs - a.playtime_secs)
      .slice(0, 10)
      .map((game) => ({ game, secs: game.playtime_secs })),
    stores: [...stores.values()].filter((s) => s.games > 0).sort((a, b) => b.secs - a.secs || b.games - a.games),
    statuses,
    weeks,
    sessions: sessions.filter((s) => byId.has(s.game_id)).length,
    thisWeekSecs: weeks[weeks.length - 1].secs,
    thisMonthSecs,
    longest,
    year: yearStats,
  };
}

/** Whole hours with thousands separators: "1,204h"; "<1h" and "0h" below an hour. */
export function hours(secs: number): string {
  if (secs === 0) return "0h";
  if (secs < 3600) return "<1h";
  return `${Math.round(secs / 3600).toLocaleString()}h`;
}
