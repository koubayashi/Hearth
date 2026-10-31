import { describe, expect, it } from "vitest";
import { genresOf, narrowGames, releaseValue, sortGames, type InfoOf } from "./arrange";
import type { Game, GameDetails } from "./types";

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

const details = (release_date: string | null, genres: string[] = []): GameDetails => ({
  steam_app_id: 1,
  release_date,
  developers: [],
  publishers: [],
  genres,
  features: [],
  metacritic: null,
  metacritic_url: null,
  recommendations: null,
});

const titles = (gs: Game[]) => gs.map((g) => g.title);

describe("sortGames", () => {
  const none: InfoOf = () => undefined;

  it("orders names naturally, ignoring case", () => {
    const gs = [game(1, "max payne 3"), game(2, "Max Payne 10"), game(3, "Alan Wake")];
    expect(titles(sortGames(gs, "title", none))).toEqual(["Alan Wake", "max payne 3", "Max Payne 10"]);
  });

  it("puts games without a value last, in name order", () => {
    const gs = [game(1, "B"), game(2, "A"), game(3, "C", { last_played: 100 }), game(4, "D", { last_played: 200 })];
    expect(titles(sortGames(gs, "last_played", none))).toEqual(["D", "C", "A", "B"]);
  });

  it("sorts by release date from store details", () => {
    const info: InfoOf = (id) => ({ details: details({ 1: "31 May, 2024", 2: "2001", 3: null }[id] ?? null), bytes: null });
    const gs = [game(1, "New"), game(2, "Old"), game(3, "Unknown")];
    expect(titles(sortGames(gs, "release", info))).toEqual(["New", "Old", "Unknown"]);
  });

  it("only sizes installed games", () => {
    const info: InfoOf = (id) => ({ details: null, bytes: id * 100 });
    const gs = [game(1, "Small"), game(2, "Gone", { installed: false }), game(3, "Big")];
    expect(titles(sortGames(gs, "size", info))).toEqual(["Big", "Small", "Gone"]);
  });
});

describe("releaseValue", () => {
  it("reads Steam's date styles and bare years", () => {
    expect(releaseValue("May 31, 2024")).toBe(releaseValue("31 May, 2024"));
    expect(releaseValue("2019")).toBe(Date.UTC(2019, 0, 1));
    expect(releaseValue("Coming soon")).toBeNull();
  });
});

describe("narrowGames", () => {
  const info: InfoOf = (id) => ({ details: details(null, id === 1 ? ["Action", "RPG"] : ["Racing"]), bytes: null });
  const gs = [game(1, "Witcher", { favorite: true }), game(2, "F1 24")];

  it("keeps favourites", () => {
    expect(titles(narrowGames(gs, { favorites: true, genres: [] }, info))).toEqual(["Witcher"]);
  });

  it("keeps games with any chosen genre", () => {
    expect(titles(narrowGames(gs, { favorites: false, genres: ["Racing", "Puzzle"] }, info))).toEqual(["F1 24"]);
  });

  it("lists genres by how common they are", () => {
    expect(genresOf([...gs, game(3, "Forza")], info)).toEqual(["Racing", "Action", "RPG"]);
  });
});
