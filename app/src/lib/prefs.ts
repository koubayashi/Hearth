import { useSyncExternalStore } from "react";
import type { SortKey } from "./arrange";

export type Tone = "graphite" | "gallery" | "ember" | "spruce" | "rosewood" | "harbor" | "mono";
/** "system" follows Windows' app mode (Settings > Personalization > Colors). */
export type Mode = "dark" | "light" | "system";
/** How the library is laid out: a cover wall beside the poster, or a list of
 *  titles beside a detail page. */
export type LibraryView = "covers" | "list";
/** Where the console's name comes from. "auto" picks Steam, then Epic, Xbox, GOG. */
export type NameSource = "auto" | "steam" | "epic" | "xbox" | "gog" | "custom" | "off";

export interface Prefs {
  tone: Tone;
  mode: Mode;
  /** Overrides the tone's accent when set, e.g. "#ff7a59". */
  customAccent: string | null;
  showRecent: boolean;
  /** Small store marks on covers. */
  showStoreIcons: boolean;
  /** Smallest cover width in the library grid, in px. */
  coverSize: number;
  /** Metacritic badge in the game panel. */
  showScores: boolean;
  /** Soften the hero art behind the cover in the game panel. */
  blurPosterArt: boolean;
  /** When the last backup was saved (ms), or null. */
  lastBackupAt: number | null;
  libraryView: LibraryView;
  /** The fullscreen, controller-first console. Hearth reopens in it if you left it there. */
  consoleMode: boolean;
  /** The console fills the screen while it's open. */
  consoleFullscreen: boolean;
  /** Controller button hints in the console, once a controller is used. */
  consoleHints: boolean;
  /** Where the name in the console comes from; "off" shows none. */
  nameSource: NameSource;
  /** The name typed in Settings, for nameSource "custom". */
  customName: string;
  /** How games are ordered inside each library section. */
  librarySort: SortKey;
  /** Starred games as the library's first section (and only there). */
  showFavoritesRow: boolean;
  /** The note on how to leave the console has been shown once. */
  consoleTipSeen: boolean;
}

/** Each tone's ground and accent in both modes, for the swatches (tokens.css has the full sets). */
export const TONES: { id: Tone; label: string; dark: { base: string; accent: string }; light: { base: string; accent: string } }[] = [
  { id: "graphite", label: "Graphite", dark: { base: "#111214", accent: "#b9ccff" }, light: { base: "#e9ecf0", accent: "#2f5bd3" } },
  { id: "gallery", label: "Gallery", dark: { base: "#121b26", accent: "#f5d7a1" }, light: { base: "#e7ecf2", accent: "#8a5d14" } },
  { id: "ember", label: "Ember", dark: { base: "#17110e", accent: "#f2a65a" }, light: { base: "#f0e9e3", accent: "#b04e12" } },
  { id: "spruce", label: "Spruce", dark: { base: "#0e1714", accent: "#a8e0c6" }, light: { base: "#e4ece8", accent: "#1c6e51" } },
  { id: "rosewood", label: "Rosewood", dark: { base: "#171114", accent: "#eba9b9" }, light: { base: "#efe7ea", accent: "#a3385a" } },
  { id: "harbor", label: "Harbor", dark: { base: "#0d1517", accent: "#8fd5de" }, light: { base: "#e3edef", accent: "#0f6f7c" } },
  { id: "mono", label: "Mono", dark: { base: "#121212", accent: "#ececec" }, light: { base: "#ebebeb", accent: "#1c1c1c" } },
];

const systemDark = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;

/** Dark or light as shown now: "system" resolved against Windows. */
export const shownMode = (p: Prefs): "dark" | "light" =>
  p.mode === "system" ? (systemDark?.matches === false ? "light" : "dark") : p.mode;

const KEY = "hearth.prefs";
const DEFAULTS: Prefs = { tone: "graphite", mode: "dark", customAccent: null, showRecent: true, showStoreIcons: true, coverSize: 150, showScores: false, blurPosterArt: true, lastBackupAt: null, libraryView: "covers", consoleMode: false, consoleFullscreen: true, consoleHints: true, nameSource: "auto", customName: "", librarySort: "title", showFavoritesRow: true, consoleTipSeen: false };

function load(): Prefs {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return DEFAULTS;
  }
}

let current = load();
const listeners = new Set<() => void>();

export function setPrefs(patch: Partial<Prefs>) {
  const before = current;
  current = { ...current, ...patch };
  try {
    localStorage.setItem(KEY, JSON.stringify(current));
  } catch {
    // Storage unavailable: prefs still apply for this session.
  }
  // Only whole-window changes fade: a highlight dragged in the picker follows the pointer.
  const looks = (p: Prefs) => `${p.tone} ${shownMode(p)}`;
  if (looks(before) !== looks(current)) recolor(() => applyPrefs(current));
  else applyPrefs(current);
  listeners.forEach((l) => l());
}

/** A new tone or mode cross-fades the whole window instead of switching in a
 *  flash (surfaces would otherwise change at different speeds). The same fade
 *  is gentle enough under reduced motion: no movement, only colour. */
function recolor(apply: () => void) {
  if (!document.startViewTransition) return apply();
  document.startViewTransition(apply);
}

// "Match Windows": follow the system when it switches between dark and light.
systemDark?.addEventListener("change", () => {
  if (current.mode === "system") recolor(() => applyPrefs(current));
});

export function usePrefs(): Prefs {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => current,
  );
}

/** WCAG relative luminance of "#rrggbb". */
function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio between two "#rrggbb" colours, 1 to 21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** Pick black or white text for a custom accent so the Play button stays legible. */
export function inkFor(hex: string): string {
  return luminance(hex) > 0.4 ? "#111214" : "#ffffff";
}

/** Like `shownMode`, and re-renders when Windows switches under "Match Windows". */
export function useShownMode(p: Prefs): "dark" | "light" {
  const system = useSyncExternalStore(
    (l) => (systemDark?.addEventListener("change", l), () => systemDark?.removeEventListener("change", l)),
    () => systemDark?.matches ?? true,
  );
  return p.mode === "system" ? (system ? "dark" : "light") : p.mode;
}

export function applyPrefs(p: Prefs = current) {
  const root = document.documentElement;
  root.dataset.tone = p.tone;
  root.dataset.mode = shownMode(p);
  if (p.customAccent) {
    root.style.setProperty("--accent", p.customAccent);
    root.style.setProperty("--accent-ink", inkFor(p.customAccent));
  } else {
    root.style.removeProperty("--accent");
    root.style.removeProperty("--accent-ink");
  }
}
