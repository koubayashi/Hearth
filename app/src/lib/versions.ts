// Versions of one game (GTA V and GTA V Enhanced): what each is called,
// which one Play uses, and which games look like versions of each other.

import type { Game, StoreEntry } from "./types";

export interface Version {
  /** "Enhanced", "Special Edition", or "Original" for the plain one. */
  label: string;
  /** The store's full name for it. */
  title: string;
  /** The same version may be owned on several stores. */
  entries: StoreEntry[];
  installed: boolean;
}

const words = (s: string) =>
  s
    .replace(/[™®©]/g, "")
    .split(/[\s:\-–—_,.]+/)
    .filter(Boolean);
const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** A game's versions, when it has more than one; [] for an ordinary game. */
export function versionsOf(g: Game): Version[] {
  const byTitle = new Map<string, StoreEntry[]>();
  for (const e of g.entries) {
    const t = e.title ?? g.title;
    const key = words(t).join(" ").toLowerCase();
    byTitle.set(key, [...(byTitle.get(key) ?? []), e]);
  }
  if (byTitle.size < 2) return [];
  const titles = [...byTitle.values()].map((es) => es[0].title ?? g.title);
  // The words every version starts with ("Grand Theft Auto V") aren't part
  // of any version's name; what's left is ("Enhanced"), or "Original".
  const split = titles.map(words);
  let common = 0;
  while (split.every((w) => common < w.length && same(w[common], split[0][common]))) common++;
  return [...byTitle.values()].map((entries, i) => {
    const rest = split[i].slice(common).join(" ");
    return {
      label: rest ? titleCase(rest) : "Original",
      title: titles[i],
      entries,
      installed: entries.some((e) => e.installed && !e.offline),
    };
  });
}

/** "REMASTERED" → "Remastered"; "Special Edition" stays. */
function titleCase(s: string) {
  return s === s.toUpperCase() ? s.toLowerCase().replace(/\b\w/g, (c) => c.toUpperCase()) : s;
}

/** The version Play starts: the installed one played last; with none
 *  installed, the newer version (not the original) to install. */
export function playVersion(g: Game): Version | null {
  const vs = versionsOf(g);
  if (vs.length === 0) return null;
  const lastPlayed = (v: Version) => Math.max(0, ...v.entries.map((e) => e.last_played ?? 0));
  const installed = vs.filter((v) => v.installed);
  if (installed.length > 0) return installed.reduce((a, b) => (lastPlayed(b) > lastPlayed(a) ? b : a));
  return vs.find((v) => v.label !== "Original") ?? vs[0];
}

/** The store entry Hearth launches or installs a version through. */
export const versionEntry = (v: Version) =>
  v.entries.find((e) => e.installed && !e.offline) ?? v.entries.find((e) => e.store !== "manual") ?? v.entries[0];

// Words that mark the same game rebuilt, not a new game. "Remake" isn't
// here: a remake (Resident Evil 2, 2019) is a game of its own.
const EDITION =
  /\s*[:\-–—]?\s*(enhanced( edition)?|remastered|remaster|legacy|special edition|definitive edition|anniversary edition|game of the year edition|goty( edition)?|complete edition|director'?s cut|next gen|redux|hd)\s*$/i;

/** The title with its edition words taken off, compared loosely. */
export const baseTitle = (title: string) => {
  let t = title.replace(/[™®©]/g, "").trim();
  for (let s = t.replace(EDITION, ""); s !== t && s.length > 0; s = t.replace(EDITION, "")) t = s;
  return words(t).join("").toLowerCase();
};

/** Key for a suggestion the user said no to. */
export const pairKey = (a: number, b: number) => (a < b ? `${a}-${b}` : `${b}-${a}`);

/**
 * Games that look like versions of each other: the same title once edition
 * words ("Enhanced", "Remastered", "Special Edition") are taken off, where
 * the titles do differ. Hearth only suggests; the user combines. Each is
 * [keep, other]: the game keeps the plain title's card.
 */
export function suggestCombines(games: Game[], dismissed: string[]): [Game, Game][] {
  const groups = new Map<string, Game[]>();
  for (const g of games) {
    const key = baseTitle(g.title);
    if (key) groups.set(key, [...(groups.get(key) ?? []), g]);
  }
  const out: [Game, Game][] = [];
  for (const group of groups.values()) {
    if (group.length < 2) continue;
    const plainFirst = [...group].sort((a, b) => a.title.length - b.title.length || a.added_at - b.added_at);
    const keep = plainFirst[0];
    for (const other of plainFirst.slice(1)) {
      if (!dismissed.includes(pairKey(keep.id, other.id))) out.push([keep, other]);
    }
  }
  return out;
}
