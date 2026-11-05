import { describe, expect, it } from "vitest";
import { driveRoot, groupByDrive, idleHint, installPath, type StoredGame } from "./storage";
import type { Game } from "./types";

const DAY = 86400;
const NOW = 1_800_000_000 * 1000;

function game(id: number, title: string, extra: Partial<Game> = {}): Game {
  return {
    id,
    title,
    favorite: false,
    hidden: false,
    installed: true,
    added_at: 0,
    last_played: null,
    playtime_secs: 0,
    cover: null,
    hero: null,
    logo: null,
    description: null,
    status: null,
    collections: [],
    offline_drive: null,
    launch_options: [],
    entries: [],
    ...extra,
  };
}

const stored = (id: number, title: string, path: string, bytes: number | null): StoredGame => ({ game: game(id, title), path, bytes });

describe("driveRoot", () => {
  it("reads letters and network shares", () => {
    expect(driveRoot("d:\\Games\\Foo")).toBe("D:");
    expect(driveRoot("E:/SteamLibrary")).toBe("E:");
    expect(driveRoot("\\\\NAS\\Games\\Foo")).toBe("\\\\nas\\games");
    expect(driveRoot("Games\\Foo")).toBeNull();
  });
});

describe("installPath", () => {
  it("prefers the installed entry, then one on a missing drive", () => {
    const entry = (installed: boolean, offline: boolean, dir: string) => ({
      id: 1, game_id: 1, store: "steam" as const, store_game_id: "1", installed, install_dir: dir, hidden: false,
      store_playtime_secs: null, store_baseline_secs: null, launch_exe: null, offline,
    });
    expect(installPath(game(1, "A", { entries: [entry(false, true, "E:\\A"), entry(true, false, "C:\\A")] }))).toBe("C:\\A");
    expect(installPath(game(1, "A", { entries: [entry(false, true, "E:\\A")] }))).toBe("E:\\A");
    expect(installPath(game(1, "A", { entries: [entry(false, false, "C:\\A")] }))).toBeNull();
  });
});

describe("groupByDrive", () => {
  const drives = [
    { root: "C:", label: "Windows", total: 1000, free: 400 },
    { root: "D:", label: "Games", total: 2000, free: 100 },
    { root: "F:", label: null, total: 500, free: 500 },
  ];

  it("puts games on their drive, largest first, unmeasured last", () => {
    const out = groupByDrive(
      [stored(1, "Small", "d:\\a", 10), stored(2, "Big", "D:\\b", 900), stored(3, "Zed", "D:\\c", null), stored(4, "Alpha", "D:\\d", null)],
      drives,
    );
    expect(out[0].root).toBe("D:");
    expect(out[0].games.map((g) => g.game.title)).toEqual(["Big", "Small", "Alpha", "Zed"]);
    expect(out[0].gameBytes).toBe(910);
  });

  it("orders drives by game bytes, then empty drives by letter", () => {
    const out = groupByDrive([stored(1, "A", "C:\\a", 50), stored(2, "B", "D:\\b", 500)], drives);
    expect(out.map((d) => d.root)).toEqual(["D:", "C:", "F:"]);
  });

  it("shows a drive that isn't connected, with no space", () => {
    const out = groupByDrive([stored(1, "A", "E:\\a", 70)], drives);
    const e = out.find((d) => d.root === "E:")!;
    expect(e.space).toBeNull();
    expect(e.games).toHaveLength(1);
  });
});

describe("idleHint", () => {
  it("says nothing for recent play", () => {
    expect(idleHint(game(1, "A", { last_played: NOW / 1000 - 10 * DAY }), NOW)).toBeNull();
  });
  it("counts months, then years", () => {
    expect(idleHint(game(1, "A", { last_played: NOW / 1000 - 250 * DAY }), NOW)).toBe("Not played in 8 months");
    expect(idleHint(game(1, "A", { last_played: NOW / 1000 - 400 * DAY }), NOW)).toBe("Not played in a year");
    expect(idleHint(game(1, "A", { last_played: NOW / 1000 - 800 * DAY }), NOW)).toBe("Not played in 2 years");
  });
  it("knows never played from played-without-a-date", () => {
    expect(idleHint(game(1, "A"), NOW)).toBe("Never played");
    expect(idleHint(game(1, "A", { playtime_secs: 3600 }), NOW)).toBeNull();
  });
});
