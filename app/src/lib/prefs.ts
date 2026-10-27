import { useSyncExternalStore } from "react";

export type Tone = "graphite" | "gallery" | "ember" | "spruce";
export type Mode = "dark" | "light";
/** How the library is laid out: a cover wall beside the poster, or a list of
 *  titles beside a detail page. */
export type LibraryView = "covers" | "list";
/** Where the console's name comes from. "auto" picks Steam, then Epic. */
export type NameSource = "auto" | "steam" | "epic" | "custom" | "off";

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
}

export const TONES: { id: Tone; label: string; base: string; accent: string }[] = [
  { id: "graphite", label: "Graphite", base: "#111214", accent: "#b9ccff" },
  { id: "gallery", label: "Gallery blue", base: "#121b26", accent: "#f5d7a1" },
  { id: "ember", label: "Ember", base: "#17110e", accent: "#f2a65a" },
  { id: "spruce", label: "Spruce", base: "#0e1714", accent: "#a8e0c6" },
];

const KEY = "hearth.prefs";
const DEFAULTS: Prefs = { tone: "graphite", mode: "dark", customAccent: null, showRecent: true, showStoreIcons: true, coverSize: 150, showScores: false, blurPosterArt: true, lastBackupAt: null, libraryView: "covers", consoleMode: false, consoleFullscreen: true, consoleHints: true, nameSource: "auto", customName: "" };

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
  current = { ...current, ...patch };
  try {
    localStorage.setItem(KEY, JSON.stringify(current));
  } catch {
    // Storage unavailable: prefs still apply for this session.
  }
  applyPrefs(current);
  listeners.forEach((l) => l());
}

export function usePrefs(): Prefs {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => current,
  );
}

/** Pick black or white text for a custom accent so the Play button stays legible. */
export function inkFor(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum > 0.4 ? "#111214" : "#ffffff";
}

export function applyPrefs(p: Prefs = current) {
  const root = document.documentElement;
  root.dataset.tone = p.tone;
  root.dataset.mode = p.mode;
  if (p.customAccent) {
    root.style.setProperty("--accent", p.customAccent);
    root.style.setProperty("--accent-ink", inkFor(p.customAccent));
  } else {
    root.style.removeProperty("--accent");
    root.style.removeProperty("--accent-ink");
  }
}
