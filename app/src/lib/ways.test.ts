import { describe, expect, it } from "vitest";
import { folderGone, jumpBackIn } from "./ways";
import type { Game, GameStatus, LaunchOption } from "./types";

function game(id: number, extra: Partial<Game> & { store?: "manual" | "steam" } = {}): Game {
  const { store = "steam", ...rest } = extra;
  return {
    id,
    title: `Game ${id}`,
    favorite: false,
    hidden: false,
    installed: true,
    added_at: 0,
    last_played: 1000 + id,
    playtime_secs: 3600,
    cover: null,
    hero: null,
    logo: null,
    description: null,
    status: null,
    collections: [],
    offline_drive: null,
    launch_options: [],
    entries: [
      {
        id, game_id: id, store, store_game_id: `${id}`, installed: rest.installed ?? true, install_dir: null, hidden: false,
        store_playtime_secs: null, store_baseline_secs: null, launch_exe: null, offline: false,
      },
    ],
    ...rest,
  };
}

const way = (missing: boolean): LaunchOption => ({
  id: 1, game_id: 1, name: "Single Player", path: "C:\Games\X\sp.exe", args: "", working_dir: null, is_default: true, missing,
});

describe("folderGone", () => {
  it("knows a deleted folder game, even with a way to play whose program went with it", () => {
    expect(folderGone(game(1, { store: "manual", installed: false }))).toBe(true);
    expect(folderGone(game(1, { store: "manual", installed: false, launch_options: [way(true)] }))).toBe(true);
  });
  it("keeps a way to play that still exists", () => {
    expect(folderGone(game(1, { store: "manual", installed: false, launch_options: [way(false)] }))).toBe(false);
  });
  it("is never true for an installed game or a store game", () => {
    expect(folderGone(game(1, { store: "manual" }))).toBe(false);
    expect(folderGone(game(1, { installed: false }))).toBe(false);
  });
});

describe("jumpBackIn", () => {
  const status = (s: GameStatus) => ({ status: s });
  it("leaves out uninstalled, completed, abandoned and never-played games", () => {
    const games = [
      game(1),
      game(2, { installed: false }),
      game(3, status("completed")),
      game(4, status("abandoned")),
      game(5, { last_played: null }),
      game(6, status("playing")),
      game(7, { installed: false, offline_drive: "E:" }),
    ];
    expect(jumpBackIn(games).map((g) => g.id)).toEqual([7, 6, 1]);
  });
});
