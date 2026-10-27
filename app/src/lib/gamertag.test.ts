import { describe, expect, it } from "vitest";
import { pickName, possessive } from "./gamertag";
import type { Accounts } from "./types";

const both: Accounts = { steam: "ExamplePersona", epic: "EpicExample", sgdb: false, steam_auth: null, xbox: null, gog: null };
const epicOnly: Accounts = { ...both, steam: null };

describe("pickName", () => {
  it("picks Steam, then Epic, when left to itself", () => {
    expect(pickName(both, { nameSource: "auto", customName: "" })?.name).toBe("ExamplePersona");
    expect(pickName(epicOnly, { nameSource: "auto", customName: "" })?.name).toBe("EpicExample");
  });

  it("uses the chosen store", () => {
    expect(pickName(both, { nameSource: "epic", customName: "" })?.name).toBe("EpicExample");
  });

  it("falls back when the chosen store has no name", () => {
    expect(pickName(epicOnly, { nameSource: "steam", customName: "" })).toEqual({ source: "epic", name: "EpicExample" });
    expect(pickName(both, { nameSource: "custom", customName: "  " })?.source).toBe("steam");
  });

  it("uses your own name, trimmed", () => {
    expect(pickName(both, { nameSource: "custom", customName: " Koushik " })?.name).toBe("Koushik");
  });

  it("shows nothing when turned off or nothing is known", () => {
    expect(pickName(both, { nameSource: "off", customName: "Koushik" })).toBeNull();
    expect(pickName(null, { nameSource: "auto", customName: "Koushik" })).toBeNull();
  });
});

describe("possessive", () => {
  it("adds an apostrophe s, or only an apostrophe after s", () => {
    expect(possessive("Koushik")).toBe("Koushik’s");
    expect(possessive("James")).toBe("James’");
  });
});
