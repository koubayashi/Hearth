import { describe, expect, it } from "vitest";
import { idleSplit } from "./idle";
import type { Game } from "./types";

const game = (extra: Partial<Game>) => ({ playtime_secs: 40 * 3600, idle_secs: 2 * 3600, idle_mode: null, ...extra }) as Game;

describe("idleSplit", () => {
  it("shows active and idle time when idle time is kept separate", () => {
    expect(idleSplit(game({}), "separate")).toBe("38h active · 2h idle");
  });
  it("says nothing when idle time counts as play", () => {
    expect(idleSplit(game({}), "count")).toBeNull();
    expect(idleSplit(game({ idle_mode: "count" }), "separate")).toBeNull();
  });
  it("a game's own choice wins over the setting", () => {
    expect(idleSplit(game({ idle_mode: "separate" }), "count")).toBe("38h active · 2h idle");
  });
  it("leaves out idle time under ten minutes", () => {
    expect(idleSplit(game({ idle_secs: 9 * 60 }), "separate")).toBeNull();
  });
});
