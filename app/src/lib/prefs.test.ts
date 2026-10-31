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

describe("highlight presets", () => {
  it("work on every tone's ground, dark and light, with white text on them", async () => {
    const { ACCENT_PRESETS } = await import("../components/AccentPicker");
    for (const { hex, name } of ACCENT_PRESETS) {
      for (const t of TONES) {
        for (const mode of ["dark", "light"] as const) {
          expect(contrast(hex, t[mode].base), `${name} on ${t.id} ${mode}`).toBeGreaterThanOrEqual(3);
        }
      }
      expect(inkFor(hex), name).toBe("#ffffff");
      expect(contrast(hex, "#ffffff"), name).toBeGreaterThanOrEqual(4.5);
    }
  });
});
