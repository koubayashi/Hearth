import { describe, expect, it } from "vitest";
import { baseTitle, pairKey, pickEntry, playVersion, suggestCombines, versionsOf } from "./versions";
import type { Game, StoreEntry } from "./types";

let nextId = 1;
const entry = (title: string, extra: Partial<StoreEntry> = {}): StoreEntry => ({
  id: nextId++, game_id: 1, store: "epic", store_game_id: title, installed: false, install_dir: null, hidden: false,
  store_playtime_secs: null, store_baseline_secs: null, launch_exe: null, offline: false, title, last_played: null, ...extra,
});
const game = (id: number, title: string, entries: StoreEntry[] = [entry(title)], added_at = 0): Game => ({
  id, title, favorite: false, hidden: false, installed: entries.some((e) => e.installed), added_at, last_played: null,
  playtime_secs: 0, cover: null, hero: null, logo: null, description: null, status: null, collections: [],
  offline_drive: null, launch_options: [], entries,
});

describe("versionsOf", () => {
  it("names each version by what its title adds", () => {
    const g = game(1, "Grand Theft Auto V", [entry("Grand Theft Auto V"), entry("Grand Theft Auto V Enhanced", { installed: true })]);
    expect(versionsOf(g).map((v) => [v.label, v.installed])).toEqual([["Original", false], ["Enhanced", true]]);
  });
  it("treats the same version on two stores as one", () => {
    const g = game(1, "Skyrim", [entry("The Elder Scrolls V: Skyrim Special Edition"), entry("The Elder Scrolls V: Skyrim Special Edition", { store: "steam" }), entry("The Elder Scrolls V: Skyrim")]);
    const vs = versionsOf(g);
    expect(vs.map((v) => [v.label, v.entries.length])).toEqual([["Special Edition", 2], ["Original", 1]]);
  });
  it("is empty for an ordinary game", () => {
    expect(versionsOf(game(1, "Hades", [entry("Hades"), entry("HADES™", { store: "steam" })]))).toEqual([]);
  });
  it("tidies shouting titles", () => {
    const g = game(1, "DARK SOULS", [entry("DARK SOULS™: REMASTERED"), entry("Dark Souls: Prepare to Die Edition")]);
    expect(versionsOf(g).map((v) => v.label)).toEqual(["Remastered", "Prepare to Die Edition"]);
  });
});

describe("playVersion", () => {
  it("plays the installed version played last, else installs the newer one", () => {
    const both = game(1, "GTA V", [
      entry("GTA V", { installed: true, last_played: 500 }),
      entry("GTA V Enhanced", { installed: true, last_played: 900 }),
    ]);
    expect(playVersion(both)?.label).toBe("Enhanced");
    const none = game(1, "GTA V", [entry("GTA V"), entry("GTA V Enhanced")]);
    expect(playVersion(none)?.label).toBe("Enhanced");
    expect(playVersion(game(1, "Hades"))).toBeNull();
  });
  it("plays the first installed version on a tie, never-played included", () => {
    const never = game(1, "GTA V", [entry("GTA V", { installed: true }), entry("GTA V Enhanced", { installed: true })]);
    expect(playVersion(never)?.label).toBe("Original");
    const tied = game(1, "GTA V", [entry("GTA V", { installed: true, last_played: 7 }), entry("GTA V Enhanced", { installed: true, last_played: 7 })]);
    expect(playVersion(tied)?.label).toBe("Original");
  });
});

describe("pickEntry", () => {
  it("picks the installed entry played last, the first on a tie", () => {
    const a = entry("Hades", { installed: true, last_played: 5 });
    const b = entry("Hades", { installed: true, last_played: 9, store: "steam" });
    const c = entry("Hades", { installed: true, last_played: 9, store: "gog" });
    expect(pickEntry([a, b, c])).toBe(b);
    const x = entry("Hades", { installed: true });
    const y = entry("Hades", { installed: true, store: "steam" });
    expect(pickEntry([x, y])).toBe(x);
  });
  it("skips offline and uninstalled entries, else picks one a store can install from", () => {
    const off = entry("Hades", { installed: true, offline: true, last_played: 99 });
    const on = entry("Hades", { installed: true });
    expect(pickEntry([off, on])).toBe(on);
    const manual = entry("Hades", { store: "manual" });
    const steam = entry("Hades", { store: "steam" });
    expect(pickEntry([manual, steam])).toBe(steam);
  });
});

describe("suggestCombines", () => {
  const gta = game(22, "Grand Theft Auto V");
  const enh = game(38, "Grand Theft Auto V Enhanced");
  it("pairs a game with its enhanced or remastered version, plain title kept", () => {
    const pairs = suggestCombines([enh, gta, game(5, "Hades")], []);
    expect(pairs.map(([k, o]) => [k.id, o.id])).toEqual([[22, 38]]);
  });
  it("leaves remakes and dismissed pairs alone", () => {
    expect(suggestCombines([game(1, "Resident Evil 2"), game(2, "Resident Evil 2 Remake")], [])).toEqual([]);
    expect(suggestCombines([gta, enh], [pairKey(38, 22)])).toEqual([]);
  });
  it("knows common edition words", () => {
    expect(baseTitle("DARK SOULS™: REMASTERED")).toBe(baseTitle("Dark Souls"));
    expect(baseTitle("The Witcher 3: Wild Hunt - Game of the Year Edition")).toBe(baseTitle("The Witcher 3: Wild Hunt"));
    expect(baseTitle("Skyrim Special Edition")).toBe(baseTitle("Skyrim"));
  });
});
