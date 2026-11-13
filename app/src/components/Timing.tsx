import { api } from "../lib/api";
import { sittingSecs } from "../lib/clock";
import { formatPlaytime } from "../lib/format";
import type { NowPlaying } from "../lib/types";
import { Icon } from "./Icon";

/** "Playing now · 1h 12m" with a stopwatch while counting; "Paused · 1h 12m"
 *  (or "Idle", when Hearth noticed you'd gone) with a pause mark while not.
 *  The stopwatch says this is the time being counted now, not a total. */
export function SittingText({ session, now }: { session: NowPlaying; now: number }) {
  const secs = sittingSecs(session, now);
  if (session.paused) {
    return (
      <span className="sitting paused" title="Not counting. Timing picks up when you're back in the game.">
        <Icon name="pause" size={12} />
        {session.paused === "away" ? "Idle" : "Paused"}
        {secs >= 60 && ` · ${formatPlaytime(secs)}`}
      </span>
    );
  }
  return (
    <span className="sitting live">
      <Icon name="stopwatch" size={15} />
      {secs < 60 ? "Playing now" : `Playing now · ${formatPlaytime(secs)}`}
    </span>
  );
}

/** Pause or Resume for the game being timed. */
export function PauseButton({ session, className = "btn sm" }: { session: NowPlaying; className?: string }) {
  const paused = !!session.paused;
  return (
    <button
      className={`${className}${paused ? " btn-primary" : ""}`}
      data-nav
      title={paused ? undefined : "Stop counting for now. Timing picks up when you're back in the game."}
      onClick={() => void (paused ? api.resumeTiming(session.game_id) : api.pauseTiming(session.game_id))}
    >
      <Icon name={paused ? "play" : "pause"} size={12} />
      {paused ? "Resume" : "Pause"}
    </button>
  );
}
