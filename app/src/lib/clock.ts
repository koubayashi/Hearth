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

/** Seconds of the session still running, or 0. */
export const liveSecs = (playingSince: number | null, now: number) => (playingSince ? Math.max(0, now - playingSince) : 0);
