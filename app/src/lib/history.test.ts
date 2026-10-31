import { describe, expect, it } from "vitest";
import { gameHistory } from "./history";
import { WEEKS_SHOWN } from "./stats";
import type { PlaySession } from "./types";

const at = (y: number, m: number, d: number, h = 20) => new Date(y, m - 1, d, h).getTime() / 1000;
const session = (id: number, game_id: number, start: number, mins: number): PlaySession => ({
  id,
  game_id,
  started_at: start,
  ended_at: start + mins * 60,
  source: "tracker",
});

describe("gameHistory", () => {
  const now = new Date(2026, 9, 31, 12).getTime();
  const sessions = [
    session(1, 7, at(2026, 9, 2), 90),
    session(2, 7, at(2026, 10, 29), 30),
    session(3, 8, at(2026, 10, 30), 600),
    session(4, 7, at(2026, 10, 3), 120),
  ];

  it("groups one game's sessions by month, newest first", () => {
    const h = gameHistory(sessions, 7, now);
    expect(h.count).toBe(3);
    expect(h.secs).toBe(240 * 60);
    expect(h.months.map((m) => m.key)).toEqual(["2026-10", "2026-09"]);
    expect(h.months[0].sessions.map((s) => s.id)).toEqual([2, 4]);
    expect(h.months[0].secs).toBe(150 * 60);
    expect(h.first).toBe(at(2026, 9, 2));
    expect(h.longest?.id).toBe(4);
  });

  it("puts this week last in the weekly strip", () => {
    const h = gameHistory(sessions, 7, now);
    expect(h.weeks).toHaveLength(WEEKS_SHOWN);
    expect(h.weeks[WEEKS_SHOWN - 1]).toBe(30 * 60);
    expect(h.weeks.reduce((a, b) => a + b, 0)).toBe(240 * 60);
  });

  it("is empty for a game Hearth never timed", () => {
    const h = gameHistory(sessions, 99, now);
    expect(h).toMatchObject({ count: 0, secs: 0, first: null, longest: null, months: [] });
  });
});
