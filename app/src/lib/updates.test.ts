import { describe, expect, it } from "vitest";
import { boldRuns, parseNotes } from "./updates";

describe("parseNotes", () => {
  it("reads a changelog section", () => {
    const md = [
      "A big one: storage and history.",
      "",
      "### New",
      "",
      "- **Storage.** The bottom of Stats now shows how full each drive is,",
      "  with games marked separately.",
      "- **Uninstall.** Always through whoever installed it:",
      "  - Steam games: Steam's own confirmation opens.",
      "  - GOG games: the game's own uninstaller",
      "    runs.",
      "",
      "  Hearth notices when the game is gone.",
      "",
      "### Fixed",
      "",
      "- A blank cover.",
    ].join("\n");
    expect(parseNotes(md)).toEqual([
      { kind: "p", text: "A big one: storage and history." },
      { kind: "h", text: "New" },
      {
        kind: "ul",
        items: [
          { text: "**Storage.** The bottom of Stats now shows how full each drive is, with games marked separately.", sub: [] },
          {
            text: "**Uninstall.** Always through whoever installed it: Hearth notices when the game is gone.",
            sub: ["Steam games: Steam's own confirmation opens.", "GOG games: the game's own uninstaller runs."],
          },
        ],
      },
      { kind: "h", text: "Fixed" },
      { kind: "ul", items: [{ text: "A blank cover.", sub: [] }] },
    ]);
  });

  it("handles Windows line endings and nothing at all", () => {
    expect(parseNotes("- One\r\n- Two\r\n")).toEqual([{ kind: "ul", items: [{ text: "One", sub: [] }, { text: "Two", sub: [] }] }]);
    expect(parseNotes("")).toEqual([]);
  });
});

describe("boldRuns", () => {
  it("puts bold text at odd places", () => {
    expect(boldRuns("**F5 refreshes**, like Settings")).toEqual(["", "F5 refreshes", ", like Settings"]);
    expect(boldRuns("plain")).toEqual(["plain"]);
  });
});
