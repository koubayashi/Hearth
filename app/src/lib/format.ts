/** "41h 12m", "45m", or "Never played". */
export function formatPlaytime(secs: number): string {
  if (secs < 60) return secs === 0 ? "Never played" : "Under a minute";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h === 0 ? `${m}m` : m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/** A game's playtime. A game with a last-played date but no time (played
 *  before Hearth, found from its save files) hasn't been "never played". */
export function formatGamePlaytime(secs: number, lastPlayed: number | null): string {
  if (secs === 0 && lastPlayed !== null) return "No time recorded";
  return formatPlaytime(secs);
}

/** When a game was last played. A store can report hours without a date;
 *  that game has been played, Hearth just doesn't know when. */
export function formatGameLastPlayed(lastPlayed: number | null, secs: number): string {
  if (lastPlayed === null && secs > 0) return "Not recorded";
  return formatLastPlayed(lastPlayed);
}

/** A relative phrase inside a sentence: "Last week" → "last week". Dates
 *  ("Mar 12, 2024") keep their capitals. */
export function inSentence(phrase: string): string {
  return /^(Just|Yesterday|Last|An|Never|Under|No)\b/.test(phrase) ? phrase[0].toLowerCase() + phrase.slice(1) : phrase;
}

/** Compact hours for tiles: "41h", "<1h", "". */
export function formatHoursShort(secs: number): string {
  if (secs === 0) return "";
  return secs < 3600 ? "<1h" : `${Math.floor(secs / 3600)}h`;
}

/** "Just now", "2 hours ago", "Yesterday", "3 weeks ago", "12 Mar 2024". */
export function formatLastPlayed(unix: number | null, nowMs = Date.now()): string {
  if (unix === null) return "Never played";
  const diff = Math.max(0, nowMs / 1000 - unix);
  const min = 60, hour = 3600, day = 86400;
  if (diff < 5 * min) return "Just now";
  if (diff < hour) return `${Math.floor(diff / min)} minutes ago`;
  if (diff < day) {
    const h = Math.floor(diff / hour);
    return h === 1 ? "An hour ago" : `${h} hours ago`;
  }
  if (diff < 2 * day) return "Yesterday";
  if (diff < 7 * day) return `${Math.floor(diff / day)} days ago`;
  if (diff < 30 * day) {
    const w = Math.floor(diff / (7 * day));
    return w === 1 ? "Last week" : `${w} weeks ago`;
  }
  return new Date(unix * 1000).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/** "86.4 GB", "512 MB". */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

/** The year from a store date like "May 31, 2024" or "2024". */
export function releaseYear(date: string | null): string | null {
  return date?.match(/\b(19|20)\d{2}\b/)?.[0] ?? null;
}
