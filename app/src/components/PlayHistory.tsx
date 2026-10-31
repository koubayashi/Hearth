import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { formatPlaytime } from "../lib/format";
import { gameHistory, LONG_SESSION } from "../lib/history";
import { hours } from "../lib/stats";
import type { Game, PlaySession } from "../lib/types";
import { Icon } from "./Icon";

const day = (unix: number) =>
  new Date(unix * 1000).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
const time = (unix: number) => new Date(unix * 1000).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
const monthName = (unix: number) => {
  const d = new Date(unix * 1000);
  return d.toLocaleDateString(undefined, d.getFullYear() === new Date().getFullYear() ? { month: "long" } : { month: "long", year: "numeric" });
};

/**
 * Every session Hearth timed for one game: when, how long, by month, and a
 * strip of the last six months. A wrong one (a game left running overnight)
 * can be taken out.
 */
export function PlayHistory({ game, onClose }: { game: Game; onClose: () => void }) {
  const [sessions, setSessions] = useState<PlaySession[] | null>(null);
  // The session whose Remove was pressed once and now asks to be sure.
  const [confirming, setConfirming] = useState<number | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    api.playSessions().then(setSessions, (e) => setProblem(String(e)));
  }, []);
  const h = useMemo(() => (sessions ? gameHistory(sessions, game.id) : null), [sessions, game.id]);

  const remove = async (id: number) => {
    setProblem(null);
    try {
      await api.deletePlaySession(id);
      setSessions((all) => all?.filter((s) => s.id !== id) ?? null);
      setConfirming(null);
    } catch (e) {
      setProblem(String(e));
    }
  };

  // Stores keep their own totals; say how the two figures relate when they differ.
  const before = h ? game.playtime_secs - h.secs : 0;
  const peak = h ? Math.max(...h.weeks) : 0;

  return (
    <div className="art-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        className="ways-dialog history-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Play history of ${game.title}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            if (confirming !== null) setConfirming(null);
            else onClose();
          }
        }}
      >
        <div className="match-head">
          <span>Play history · {game.title}</span>
          <button className="icon-btn small" data-nav aria-label="Close" onClick={onClose}>
            <Icon name="close" size={16} />
          </button>
        </div>

        {h && h.count === 0 && (
          <p className="note ways-intro">
            <b>No sessions recorded yet.</b> Hearth times every session from now on, whether you start {game.title} from
            Hearth, its store or its folder.
            {game.playtime_secs > 0 && ` The ${formatPlaytime(game.playtime_secs)} shown in the library is the store's own total, which has no dates.`}
          </p>
        )}

        {h && h.count > 0 && (
          <>
            <div className="history-figures">
              <div>
                <span>Timed by Hearth</span>
                <b>{formatPlaytime(h.secs)}</b>
              </div>
              <div>
                <span>Sessions</span>
                <b>{h.count}</b>
              </div>
              <div>
                <span>Average</span>
                <b>{formatPlaytime(Math.round(h.secs / h.count))}</b>
              </div>
              {h.longest && (
                <div>
                  <span>Longest</span>
                  <b>{formatPlaytime(h.longest.ended_at - h.longest.started_at)}</b>
                </div>
              )}
            </div>
            {before >= 3600 && (
              <p className="note">
                {formatPlaytime(game.playtime_secs)} in all: the store counted {hours(before)} before Hearth started timing,
                on {day(h.first!)}.
              </p>
            )}

            {peak > 0 && (
              <div className="history-weeks" aria-label="Playtime per week, last 26 weeks">
                {h.weeks.map((secs, i) => (
                  <span key={i} title={secs ? formatPlaytime(secs) : undefined}>
                    {secs > 0 && <i style={{ height: `${Math.max((secs / peak) * 100, 6)}%` }} />}
                  </span>
                ))}
                <small>26 weeks ago</small>
                <small>This week</small>
              </div>
            )}

            <div className="history-months">
              {h.months.map((m) => (
                <section key={m.key}>
                  <h3>
                    {monthName(m.start)} <span>{formatPlaytime(m.secs)}</span>
                  </h3>
                  <ul>
                    {m.sessions.map((s) => {
                      const secs = s.ended_at - s.started_at;
                      return (
                        <li key={s.id} className={secs >= LONG_SESSION ? "long" : undefined}>
                          <span className="when">{day(s.started_at)}</span>
                          <span className="span">
                            {time(s.started_at)}–{time(s.ended_at)}
                            {secs >= LONG_SESSION && <em> · left running?</em>}
                          </span>
                          <b>{formatPlaytime(secs)}</b>
                          {confirming === s.id ? (
                            <button className="btn history-remove" data-nav autoFocus onClick={() => remove(s.id)}>
                              Remove
                            </button>
                          ) : (
                            <button
                              className="icon-btn small"
                              data-nav
                              aria-label={`Remove the session of ${day(s.started_at)}`}
                              title="Remove this session"
                              onClick={() => setConfirming(s.id)}
                            >
                              <Icon name="close" size={14} />
                            </button>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                </section>
              ))}
            </div>
          </>
        )}

        {problem && (
          <p className="note error" role="alert">
            {problem}
          </p>
        )}
        <div className="way-actions">
          <button className="btn btn-primary" data-nav onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
