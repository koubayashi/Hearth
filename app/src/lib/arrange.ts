import type { Game, GameDetails } from "./types";
import { formatBytes, formatGamePlaytime, formatLastPlayed, releaseYear } from "./format";

/** How the library orders games inside each section. */
export type SortKey = "title" | "last_played" | "playtime" | "added" | "release" | "size";

export const SORTS: { id: SortKey; label: string; short: string }[] = [
  { id: "title", label: "Name", short: "A to Z" },
  { id: "last_played", label: "Last played", short: "last played first" },
  { id: "playtime", label: "Most played", short: "most played first" },
  { id: "added", label: "Recently added", short: "newest in your library first" },
  { id: "release", label: "Release date", short: "newest releases first" },
  { id: "size", label: "Install size", short: "largest installs first" },
];

/** What's known about a game beyond the library row: store details and its install's size. */
export type InfoOf = (id: number) => { details: GameDetails | null; bytes: number | null } | undefined;

/** A store date ("31 May, 2024", "May 31, 2024", "2024") as a number to sort on; null when unknown. */
export function releaseValue(date: string | null | undefined): number | null {
  if (!date) return null;
  const t = Date.parse(date.replace(",", ""));
  if (!Number.isNaN(t)) return t;
  const y = releaseYear(date);
  return y ? Date.UTC(Number(y), 0, 1) : null;
}

/**
 * Sorted copy. Games with no value for the key (never played, no release date,
 * size unknown) go last, in name order, so a sort never hides them.
 */
export function sortGames(games: Game[], key: SortKey, info: InfoOf): Game[] {
  const byTitle = (a: Game, b: Game) => a.title.localeCompare(b.title, undefined, { sensitivity: "base", numeric: true });
  if (key === "title") return [...games].sort(byTitle);
  const value = (g: Game): number | null => {
    switch (key) {
      case "last_played":
        return g.last_played;
      case "playtime":
        return g.playtime_secs > 0 ? g.playtime_secs : null;
      case "added":
        return g.added_at;
      case "release":
        return releaseValue(info(g.id)?.details?.release_date);
      case "size":
        return g.installed ? (info(g.id)?.bytes ?? null) : null;
    }
  };
  const values = new Map(games.map((g) => [g.id, value(g)]));
  return [...games].sort((a, b) => {
    const va = values.get(a.id) ?? null;
    const vb = values.get(b.id) ?? null;
    if (va === null || vb === null) return va === vb ? byTitle(a, b) : va === null ? 1 : -1;
    return vb - va || byTitle(a, b);
  });
}

/** The line under a cover: the value the library is sorted by, so the order makes sense at a glance. */
export function coverMeta(g: Game, key: SortKey, info: InfoOf): string {
  switch (key) {
    case "last_played":
      return g.last_played === null ? "Never played" : formatLastPlayed(g.last_played);
    case "added":
      return `Added ${new Date(g.added_at * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" })}`;
    case "release":
      return releaseYear(info(g.id)?.details?.release_date ?? null) ?? "Release date unknown";
    case "size": {
      const bytes = info(g.id)?.bytes;
      return g.installed ? (bytes != null ? formatBytes(bytes) : "Size unknown") : "Not installed";
    }
    default:
      return formatGamePlaytime(g.playtime_secs, g.last_played);
  }
}

/** Genres across these games, most common first. */
export function genresOf(games: Game[], info: InfoOf): string[] {
  const count = new Map<string, number>();
  for (const g of games) for (const genre of info(g.id)?.details?.genres ?? []) count.set(genre, (count.get(genre) ?? 0) + 1);
  return [...count].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([genre]) => genre);
}

export interface Narrow {
  favorites: boolean;
  /** Games with any of these genres; empty for all. */
  genres: string[];
}

export const NO_NARROW: Narrow = { favorites: false, genres: [] };

export function narrowGames(games: Game[], n: Narrow, info: InfoOf): Game[] {
  if (!n.favorites && n.genres.length === 0) return games;
  return games.filter(
    (g) =>
      (!n.favorites || g.favorite) &&
      (n.genres.length === 0 || (info(g.id)?.details?.genres ?? []).some((x) => n.genres.includes(x))),
  );
}
