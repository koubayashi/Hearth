import type { Game } from "./types";

/** A drive as the backend reports it. */
export interface DriveSpace {
  root: string;
  label: string | null;
  total: number;
  free: number;
}

export interface StoredGame {
  game: Game;
  path: string;
  /** null while not measured yet. */
  bytes: number | null;
}

export interface DriveUse {
  root: string;
  label: string | null;
  /** null when the drive isn't connected (or can't be read). */
  space: { total: number; free: number } | null;
  /** Largest first; unmeasured last, in name order. */
  games: StoredGame[];
  /** What the measured games add up to. */
  gameBytes: number;
}

/** "D:" from "d:\Games\Foo", "\\nas\games" from "\\nas\games\Foo"; null for a relative path. */
export function driveRoot(path: string): string | null {
  const p = path.replace(/\//g, "\\");
  const letter = p.match(/^([a-zA-Z]):/);
  if (letter) return `${letter[1].toUpperCase()}:`;
  const unc = p.match(/^\\\\([^\\]+)\\([^\\]+)/);
  if (unc) return `\\\\${unc[1]}\\${unc[2]}`.toLowerCase();
  return null;
}

/** Where a game's install lives: its first installed store entry, or the
 *  entry on a drive that isn't connected. Matches what the backend measures. */
export function installPath(g: Game): string | null {
  const e = g.entries.find((x) => x.installed && x.install_dir) ?? g.entries.find((x) => x.offline && x.install_dir);
  return e?.install_dir ?? null;
}

const bySize = (a: StoredGame, b: StoredGame) =>
  a.bytes === null && b.bytes === null
    ? a.game.title.localeCompare(b.game.title)
    : a.bytes === null
      ? 1
      : b.bytes === null
        ? -1
        : b.bytes - a.bytes || a.game.title.localeCompare(b.game.title);

/**
 * Installed games grouped by drive. Drives with games come first, fullest of
 * games first; local drives with no games follow in letter order, so there's
 * still a view of where there's room to install.
 */
export function groupByDrive(games: StoredGame[], drives: DriveSpace[]): DriveUse[] {
  const byRoot = new Map<string, DriveUse>();
  const key = (r: string) => r.toLowerCase();
  for (const d of drives) {
    byRoot.set(key(d.root), { root: d.root, label: d.label, space: { total: d.total, free: d.free }, games: [], gameBytes: 0 });
  }
  for (const sg of games) {
    const root = driveRoot(sg.path);
    if (!root) continue;
    let use = byRoot.get(key(root));
    if (!use) byRoot.set(key(root), (use = { root, label: null, space: null, games: [], gameBytes: 0 }));
    use.games.push(sg);
    use.gameBytes += sg.bytes ?? 0;
  }
  const all = [...byRoot.values()];
  for (const u of all) u.games.sort(bySize);
  const withGames = all.filter((u) => u.games.length > 0).sort((a, b) => b.gameBytes - a.gameBytes || a.root.localeCompare(b.root));
  const empty = all.filter((u) => u.games.length === 0).sort((a, b) => a.root.localeCompare(b.root));
  return [...withGames, ...empty];
}

/** How long since a game was last played, as a cleanup hint: "Not played in
 *  8 months", "Never played". Nothing for a game played in the last 3 months. */
export function idleHint(g: Game, nowMs = Date.now()): string | null {
  if (g.last_played === null) return g.playtime_secs > 0 ? null : "Never played";
  const days = (nowMs / 1000 - g.last_played) / 86400;
  if (days < 90) return null;
  const months = Math.max(3, Math.floor(days / 30.44));
  if (months < 12) return `Not played in ${months} months`;
  const years = Math.floor(days / 365.25);
  return years === 1 ? "Not played in a year" : `Not played in ${years} years`;
}
