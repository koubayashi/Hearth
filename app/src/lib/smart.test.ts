import { describe, expect, it } from "vitest";
import { buildSmart } from "./smart";
import type { SmartIndex, SmartRole } from "./types";

// The cases Hearth has to get right, as Wikidata describes them.
const labels: Record<string, string> = {
  maxpayne: "Max Payne", gta: "Grand Theft Auto", f1: "F1", f1m: "F1 Manager",
  acs: "Assassin's Creed (series)", acf: "Assassin's Creed (franchise)",
  msm: "Marvel's Spider-Man", tasm: "The Amazing Spider-Man", spidey: "Spider-Man",
  remedy: "Remedy Entertainment", rockstar: "Rockstar Games", rnorth: "Rockstar North", rvan: "Rockstar Vancouver",
  taketwo: "Take-Two Interactive", infogrames: "Infogrames", ubisoft: "Ubisoft", umtl: "Ubisoft Montreal",
  uquebec: "Ubisoft Quebec", codemasters: "Codemasters", ea: "Electronic Arts", frontier: "Frontier Developments",
};

const games: [number, string, [SmartRole, string][]][] = [
  [1, "Max Payne", [["series", "maxpayne"], ["developer", "remedy"], ["publisher", "rockstar"]]],
  [2, "Max Payne 2", [["series", "maxpayne"], ["developer", "remedy"], ["publisher", "rockstar"]]],
  [3, "Max Payne 3", [["series", "maxpayne"], ["developer", "rvan"], ["publisher", "rockstar"]]],
  [4, "GTA V", [["series", "gta"], ["developer", "rnorth"], ["publisher", "rockstar"], ["publisher", "taketwo"]]],
  [5, "GTA: San Andreas", [["series", "gta"], ["developer", "rnorth"], ["publisher", "rockstar"]]],
  [6, "F1 24", [["series", "f1"], ["developer", "codemasters"], ["publisher", "ea"]]],
  [7, "F1 Manager 2024", [["series", "f1m"], ["developer", "frontier"], ["publisher", "frontier"]]],
  [8, "AC Origins", [["series", "acs"], ["developer", "umtl"], ["publisher", "ubisoft"]]],
  [9, "AC Odyssey", [["series", "acs"], ["developer", "uquebec"], ["publisher", "ubisoft"]]],
  [10, "Spider-Man Remastered", [["series", "msm"]]],
  [11, "The Amazing Spider-Man", [["series", "tasm"]]],
  [12, "A hidden game", [["series", "gta"]]],
];

const index: SmartIndex = {
  entities: Object.entries(labels).map(([id, label]) => ({ id, label })),
  parents: [
    { child: "acs", parent: "acf" },
    { child: "msm", parent: "spidey" },
    { child: "tasm", parent: "spidey" },
    { child: "rnorth", parent: "rockstar" },
    { child: "rnorth", parent: "taketwo" },
    { child: "rnorth", parent: "infogrames" }, // an old owner, still listed
    { child: "rvan", parent: "rockstar" },
    { child: "rockstar", parent: "taketwo" },
    { child: "umtl", parent: "ubisoft" },
    { child: "uquebec", parent: "ubisoft" },
    { child: "codemasters", parent: "ea" },
  ],
  links: games.flatMap(([game_id, , ls]) => ls.map(([role, entity]) => ({ game_id, role, entity, by_user: false }))),
};

const visible = new Set(games.map(([id]) => id).filter((id) => id !== 12));
const smart = buildSmart(index, visible);
const ids = (s: Set<number>) => [...s].sort((a, b) => a - b);

describe("buildSmart", () => {
  it("keeps a series together across studios", () => {
    expect(ids(smart.games("series", "maxpayne"))).toEqual([1, 2, 3]);
  });

  it("tells similar names apart", () => {
    expect(ids(smart.games("series", "f1"))).toEqual([6]);
    expect(ids(smart.games("series", "f1m"))).toEqual([7]);
  });

  it("puts sibling series under their shared franchise", () => {
    expect(ids(smart.games("series", "spidey"))).toEqual([10, 11]);
    const spidey = smart.tree("series").find((n) => n.id === "spidey")!;
    expect(spidey.children.map((c) => c.id).sort()).toEqual(["msm", "tasm"]);
  });

  it("counts a parent company's studios as the parent", () => {
    expect(ids(smart.games("developer", "ubisoft"))).toEqual([8, 9]);
    expect(ids(smart.games("developer", "taketwo"))).toEqual([3, 4, 5]);
  });

  it("leaves out hidden games", () => {
    expect(ids(smart.games("series", "gta"))).toEqual([4, 5]);
  });

  it("doesn't let an old owner gather a studio", () => {
    const roots = smart.tree("developer").map((n) => n.id);
    expect(roots).not.toContain("infogrames");
    expect(smart.ancestors("rnorth")).not.toContain("infogrames");
  });

  it("nests studios once, under the nearest owner", () => {
    const taketwo = smart.tree("developer").find((n) => n.id === "taketwo")!;
    expect(taketwo.children.map((c) => c.id)).toEqual(["rockstar"]);
    expect(taketwo.children[0].children.map((c) => c.id).sort()).toEqual(["rnorth", "rvan"]);
  });

  it("lists a game's own links, without the owners they imply", () => {
    // Take-Two owns Rockstar Games: naming both says nothing more.
    expect(smart.of(4).publisher.map((l) => l.label)).toEqual(["Rockstar Games"]);
  });
});

describe("a game made by many studios of one owner", () => {
  const studios = ["Rockstar North", "Rockstar Leeds", "Rockstar London", "Rockstar Toronto"];
  const index: SmartIndex = {
    entities: [{ id: "rs", label: "Rockstar Games" }, ...studios.map((s) => ({ id: s, label: s })), { id: "x", label: "Other Studio" }],
    parents: studios.map((s) => ({ child: s, parent: "rs" })),
    links: [
      ...studios.map((s) => ({ game_id: 1, role: "developer" as const, entity: s, by_user: false })),
      { game_id: 1, role: "developer", entity: "x", by_user: false },
      { game_id: 1, role: "publisher", entity: "rs", by_user: false },
    ],
  };
  const smart = buildSmart(index, new Set([1]));

  it("names the owner once in the panel", () => {
    expect(smart.of(1).developer.map((l) => l.label)).toEqual(["Other Studio", "Rockstar Games · 4 studios"]);
  });

  it("still lists every studio for editing", () => {
    expect(smart.of(1, "all").developer).toHaveLength(5);
  });
});

// What Wikidata leaves unsaid, from a real library.
describe("buildSmart with gaps in Wikidata", () => {
  const index: SmartIndex = {
    entities: [
      { id: "forza", label: "Forza" },
      { id: "fh", label: "Forza Horizon" },
      { id: "ww", label: "Wizarding World" },
      { id: "hl", label: "Hogwarts Legacy" },
      { id: "hp", label: "Harry Potter" },
      { id: "acf", label: "Assassin's Creed" },
      { id: "acs", label: "Assassin's Creed" },
    ],
    parents: [
      { child: "hp", parent: "ww" },
      { child: "acs", parent: "acf" },
    ],
    links: [
      // Forza Horizon 4 names both series; Wikidata doesn't say one holds the other.
      { game_id: 1, role: "series", entity: "fh", by_user: false },
      { game_id: 1, role: "series", entity: "forza", by_user: false },
      // Hogwarts Legacy: its own series, and the franchise it belongs to.
      { game_id: 2, role: "series", entity: "hl", by_user: false },
      { game_id: 2, role: "franchise", entity: "ww", by_user: false },
      { game_id: 2, role: "series", entity: "hp", by_user: false },
      { game_id: 3, role: "series", entity: "acs", by_user: false },
      { game_id: 3, role: "franchise", entity: "acf", by_user: false },
    ],
  };
  const smart = buildSmart(index, new Set([1, 2, 3]));
  const roots = smart.tree("series");

  it("puts a series inside the one whose name it starts with", () => {
    expect(roots.map((n) => n.id)).not.toContain("fh");
    expect(roots.find((n) => n.id === "forza")?.children.map((c) => c.id)).toEqual(["fh"]);
  });

  it("puts a game's series inside its franchise", () => {
    expect(roots.map((n) => n.id)).not.toContain("hl");
    expect(roots.find((n) => n.id === "ww")?.children.map((c) => c.id).sort()).toEqual(["hl", "hp"]);
  });

  it("shows a franchise and its same-named series as one card", () => {
    const ac = roots.find((n) => n.label === "Assassin's Creed")!;
    expect(ac.children).toEqual([]);
    expect(roots.filter((n) => n.label === "Assassin's Creed")).toHaveLength(1);
  });

  it("names only the most specific series in a game's panel", () => {
    expect(smart.of(1).series.map((l) => l.label)).toEqual(["Forza Horizon"]);
    expect(smart.of(3).series.map((l) => l.label)).toEqual(["Assassin's Creed"]);
  });
});
