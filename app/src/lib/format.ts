/** "41h 12m", "45m", or "Never played". */
export function formatPlaytime(secs: number): string {
  if (secs < 60) return secs === 0 ? "Never played" : "Under a minute";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h === 0 ? `${m}m` : m === 0 ? `${h}h` : `${h}h ${m}m`;
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
