import { describe, expect, it } from "vitest";
import { computeStats, hours, weekStart, WEEKS_SHOWN } from "./stats";
import type { Game, PlaySession, StoreId } from "./types";

const H = 3600;

function game(id: number, playtime: number, stores: [StoreId, number | null][], extra: Partial<Game> = {}): Game {
  return {
    id,
    title: `Game ${id}`,
    favorite: false,
    hidden: false,
    installed: false,
    added_at: 0,
    last_played: null,
    playtime_secs: playtime,
    cover: null,
    hero: null,
    logo: null,
    description: null,
    status: null,
    collections: [],
    entries: stores.map(([store, secs], i) => ({
      id: id * 10 + i,
      game_id: id,
      store,
      store_game_id: `${id}`,
      installed: false,
      install_dir: null,
      hidden: false,
      store_playtime_secs: secs,
      store_baseline_secs: secs,
    })),
    ...extra,
  };
}

const session = (game_id: number, start: Date, secs: number): PlaySession => {
  const started_at = Math.floor(start.getTime() / 1000);
  return { id: started_at, game_id, started_at, ended_at: started_at + secs, source: "tracker" };
};

describe("computeStats", () => {
  // Thursday 15 Oct 2026, 12:00 local.
  const now = new Date(2026, 9, 15, 12).getTime();

  it("totals the library and ranks the most played", () => {
    const games = [
      game(1, 10 * H, [["steam", 10 * H]], { installed: true, status: "playing" }),
      game(2, 0, [["epic", null]], { status: "backlog" }),
      game(3, 40 * H, [["epic", 40 * H]]),
    ];
    const s = computeStats(games, [], now);
    expect(s.totalSecs).toBe(50 * H);
    expect([s.games, s.installed, s.played]).toEqual([3, 1, 2]);
    expect(s.top.map((t) => t.game.id)).toEqual([3, 1]);
    expect(s.statuses).toEqual({ playing: 1, backlog: 1, completed: 0, abandoned: 0 });
    expect(s.year).toBeNull();
    expect(s.longest).toBeNull();
  });

  it("credits a merged game to the store that reported the most time", () => {
    const merged = game(1, 12 * H, [["epic", 2 * H], ["steam", 12 * H]]);
    const s = computeStats([merged, game(2, 0, [["epic", null]])], [], now);
    expect(s.stores).toEqual([
      { store: "steam", secs: 12 * H, games: 1 },
      { store: "epic", secs: 0, games: 1 },
    ]);
  });

  it("buckets sessions by the week they started, Monday first", () => {
    const games = [game(1, 5 * H, [["steam", 0]]), game(2, H, [["epic", 0]])];
    const sessions = [
      session(1, new Date(2026, 9, 12, 20), 2 * H), // Monday this week
      session(2, new Date(2026, 9, 14, 9), H), // Wednesday this week
      session(1, new Date(2026, 9, 11, 22), 3 * H), // Sunday last week
      session(1, new Date(2025, 0, 5, 10), H), // too old for the chart
    ];
    const s = computeStats(games, sessions, now);
    expect(s.weeks).toHaveLength(WEEKS_SHOWN);
    expect(s.weeks.at(-1)).toMatchObject({ start: weekStart(now / 1000), secs: 3 * H, games: 2 });
    expect(s.weeks.at(-2)?.secs).toBe(3 * H);
    expect(s.thisWeekSecs).toBe(3 * H);
    expect(s.thisMonthSecs).toBe(6 * H);
    expect(s.longest).toMatchObject({ secs: 3 * H });
    expect(s.year).toMatchObject({ year: 2026, secs: 6 * H, games: 2 });
    expect(s.year?.top?.id).toBe(1);
    expect(s.sessions).toBe(4);
  });

  it("ignores sessions of games that aren't in the list (hidden)", () => {
    const s = computeStats([game(1, H, [["steam", 0]])], [session(9, new Date(2026, 9, 14), H)], now);
    expect(s.sessions).toBe(0);
    expect(s.thisWeekSecs).toBe(0);
  });
});

describe("hours", () => {
  it("rounds to whole hours", () => {
    expect([hours(0), hours(1200), hours(5400), hours(1_234 * H)]).toEqual(["0h", "<1h", "2h", "1,234h"]);
  });
});
