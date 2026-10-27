import { describe, expect, it } from "vitest";
import { formatBytes, formatGameLastPlayed, formatGamePlaytime, formatHoursShort, formatLastPlayed, formatPlaytime, inSentence, releaseYear } from "./format";

describe("formatPlaytime", () => {
  it("formats hours and minutes", () => {
    expect(formatPlaytime(0)).toBe("Never played");
    expect(formatPlaytime(45 * 60)).toBe("45m");
    expect(formatPlaytime(3600)).toBe("1h");
    expect(formatPlaytime(41 * 3600 + 12 * 60)).toBe("41h 12m");
  });
  it("doesn't call a game with a last-played date never played", () => {
    expect(formatGamePlaytime(0, null)).toBe("Never played");
    expect(formatGamePlaytime(0, 1_700_000_000)).toBe("No time recorded");
    expect(formatGamePlaytime(3600, 1_700_000_000)).toBe("1h");
    expect(formatGameLastPlayed(null, 3600)).toBe("Not recorded");
    expect(formatGameLastPlayed(null, 0)).toBe("Never played");
  });
  it("lowercases relative phrases in a sentence, not dates", () => {
    expect(inSentence("Last week")).toBe("last week");
    expect(inSentence("An hour ago")).toBe("an hour ago");
    expect(inSentence("3 days ago")).toBe("3 days ago");
    expect(inSentence("Mar 12, 2024")).toBe("Mar 12, 2024");
  });
  it("formats short hours for tiles", () => {
    expect(formatHoursShort(0)).toBe("");
    expect(formatHoursShort(600)).toBe("<1h");
    expect(formatHoursShort(7300)).toBe("2h");
  });
});

describe("formatLastPlayed", () => {
  const now = 1_700_000_000_000;
  const ago = (s: number) => now / 1000 - s;
  it("uses relative wording for recent sessions", () => {
    expect(formatLastPlayed(null, now)).toBe("Never played");
    expect(formatLastPlayed(ago(60), now)).toBe("Just now");
    expect(formatLastPlayed(ago(2 * 3600), now)).toBe("2 hours ago");
    expect(formatLastPlayed(ago(30 * 3600), now)).toBe("Yesterday");
    expect(formatLastPlayed(ago(3 * 86400), now)).toBe("3 days ago");
    expect(formatLastPlayed(ago(15 * 86400), now)).toBe("2 weeks ago");
  });
});

describe("formatBytes and releaseYear", () => {
  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(86_412_000_000)).toBe("86.4 GB");
    expect(formatBytes(150_000_000_000)).toBe("150 GB");
  });
  it("pulls the year out of store dates", () => {
    expect(releaseYear("May 31, 2024")).toBe("2024");
    expect(releaseYear("Coming soon")).toBeNull();
    expect(releaseYear(null)).toBeNull();
  });
});
