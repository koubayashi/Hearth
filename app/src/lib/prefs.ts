import { useSyncExternalStore } from "react";
import type { SortKey } from "./arrange";
import type { IdleMode } from "./types";

export type Tone = "charcoal" | "graphite" | "gallery" | "ember" | "rosewood" | "harbor" | "mono";
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
  /** The note on how to leave the console has been shown once. */
  consoleTipSeen: boolean;
  /** "Combine these versions?" pairs the user said are different games. */
  dismissedCombines: string[];
  /** Idle time (the game open while you're away): counted as play, or kept
   *  separate. Read by the tracker from prefs.json. */
  idleTime: IdleMode;
  /** Tell someone who used Hearth before 0.4 about the Charcoal tone, once. */
  charcoalNews: boolean;
}

/** Each tone's ground and accent in both modes, for the swatches (tokens.css has the full sets). */
export const TONES: { id: Tone; label: string; dark: { base: string; accent: string }; light: { base: string; accent: string } }[] = [
  // The default: the app icon's charcoal and vermilion.
  { id: "charcoal", label: "Charcoal", dark: { base: "#111112", accent: "#ff6a48" }, light: { base: "#ecebe8", accent: "#c2381a" } },
  { id: "graphite", label: "Graphite", dark: { base: "#111214", accent: "#b9ccff" }, light: { base: "#e9ecf0", accent: "#2f5bd3" } },
  { id: "gallery", label: "Gallery", dark: { base: "#121b26", accent: "#f5d7a1" }, light: { base: "#e7ecf2", accent: "#8a5d14" } },
  { id: "ember", label: "Ember", dark: { base: "#17110e", accent: "#f2a65a" }, light: { base: "#f0e9e3", accent: "#b04e12" } },
  { id: "rosewood", label: "Rosewood", dark: { base: "#171114", accent: "#eba9b9" }, light: { base: "#efe7ea", accent: "#a3385a" } },
  { id: "harbor", label: "Harbor", dark: { base: "#0d1517", accent: "#8fd5de" }, light: { base: "#e3edef", accent: "#0f6f7c" } },
  { id: "mono", label: "Mono", dark: { base: "#121212", accent: "#ececec" }, light: { base: "#ebebeb", accent: "#1c1c1c" } },
];

const systemDark = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;

/** The tray menu sits on the taskbar, so it takes Windows mode (the taskbar's)
 *  over Hearth's own mode, like Windows' own flyouts; the tone stays Hearth's. */
let taskbarMode: "dark" | "light" | null = null;
export function followTaskbar(light: boolean) {
  taskbarMode = light ? "light" : "dark";
  recolor(() => applyPrefs(current));
}

/** Dark or light as shown now: "system" resolved against Windows. */
export const shownMode = (p: Prefs): "dark" | "light" =>
  taskbarMode ?? (p.mode === "system" ? (systemDark?.matches === false ? "light" : "dark") : p.mode);

const KEY = "hearth.prefs";
const DEFAULTS: Prefs = { tone: "charcoal", mode: "dark", customAccent: null, showRecent: true, showStoreIcons: true, coverSize: 150, showScores: false, blurPosterArt: true, lastBackupAt: null, libraryView: "covers", consoleMode: false, consoleFullscreen: true, consoleHints: true, nameSource: "auto", customName: "", librarySort: "title", consoleTipSeen: false, dismissedCombines: [], idleTime: "separate", charcoalNews: false };

/** Settings as saved, made safe to use: removed options dropped or carried
 *  over. Also for the settings in a restored backup, which may be older. */
export function sanitize(saved: Record<string, unknown>): Partial<Prefs> {
  saved = { ...saved };
  // Before "Idle time" there was a "Don't count time away" switch.
  if (saved.idleTime === undefined && saved.pauseWhenAway === false) saved.idleTime = "count";
  delete saved.pauseWhenAway;
  // A tone that was taken out (Spruce, later "Hearth") falls back to the default.
  if (!TONES.some((t) => t.id === saved.tone)) delete saved.tone;
  return saved as Partial<Prefs>;
}

function load(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    const saved = sanitize(JSON.parse(raw ?? "{}"));
    // Saved by a Hearth from before Charcoal: it keeps its tone (all saves
    // hold one; Graphite was the default) and hears about Charcoal once.
    if (raw && saved.charcoalNews === undefined) saved.charcoalNews = true;
    return { ...DEFAULTS, ...saved };
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

// Another window (the tray menu, or the main window while the menu is open)
// changed the settings: take them, so tone and mode match everywhere.
if (typeof window !== "undefined") {
  window.addEventListener("storage", (e) => {
    if (e.key !== KEY) return;
    current = load();
    applyPrefs(current);
    listeners.forEach((l) => l());
  });
}

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

/** Pick black or white text for a custom accent so the Play button stays
 *  legible: whichever reads better on it (a fixed cut-off put white on
 *  mid-bright colours like vermilion, at under 3:1). */
export function inkFor(hex: string): string {
  return contrast(hex, "#111214") >= contrast(hex, "#ffffff") ? "#111214" : "#ffffff";
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
