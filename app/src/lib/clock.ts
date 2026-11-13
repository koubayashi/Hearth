import type { NowPlaying } from "./types";
import { useEffect, useState } from "react";

/** Unix seconds now, refreshed every `everyMs` while `ticking`: for times
 *  that grow while you watch, like a game's "Playing now" time. Nothing
 *  runs while not ticking. */
export function useNowSecs(ticking: boolean, everyMs = 30_000): number {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    if (!ticking) return;
    setNow(Math.floor(Date.now() / 1000));
    const t = setInterval(() => setNow(Math.floor(Date.now() / 1000)), everyMs);
    return () => clearInterval(t);
  }, [ticking, everyMs]);
  return now;
}

/** Seconds of the session still running, or 0 (nothing running, or paused).
 *  Earlier parts of a paused sitting are already in the game's playtime. */
export const liveSecs = (p: NowPlaying | null | undefined, now: number) => (p?.started_at ? Math.max(0, now - p.started_at) : 0);

/** The time on screen while a game runs: this sitting so far, pauses left out. */
export const sittingSecs = (p: NowPlaying | null | undefined, now: number) => (p ? p.before_secs + liveSecs(p, now) : 0);
