import { describe, expect, it } from "vitest";
import { formatHoursShort, formatLastPlayed, formatPlaytime } from "./format";

describe("formatPlaytime", () => {
  it("formats hours and minutes", () => {
    expect(formatPlaytime(0)).toBe("Never played");
    expect(formatPlaytime(45 * 60)).toBe("45m");
    expect(formatPlaytime(3600)).toBe("1h");
    expect(formatPlaytime(41 * 3600 + 12 * 60)).toBe("41h 12m");
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
