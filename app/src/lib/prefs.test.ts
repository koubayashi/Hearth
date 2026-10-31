import { describe, expect, it } from "vitest";
import { TONES, contrast, inkFor } from "./prefs";

describe("tones", () => {
  it("keep the highlight visible on their ground in both modes", () => {
    for (const t of TONES) {
      for (const mode of ["dark", "light"] as const) {
        // Focus rings and selections: non-text contrast, WCAG 3:1.
        expect(contrast(t[mode].accent, t[mode].base), `${t.id} ${mode}`).toBeGreaterThanOrEqual(3);
        // The Play button's label on the highlight: text contrast, 4.5:1.
        expect(contrast(t[mode].accent, inkFor(t[mode].accent)), `${t.id} ${mode} button`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("measures contrast like WCAG", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21);
    expect(contrast("#777777", "#777777")).toBe(1);
  });
});
