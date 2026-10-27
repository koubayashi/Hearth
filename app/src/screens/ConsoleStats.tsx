import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { computeStats, hours } from "../lib/stats";
import { formatPlaytime } from "../lib/format";
import type { Game, PlaySession } from "../lib/types";
import { Art } from "../components/Art";
import { possessive, useGamerTag } from "../lib/gamertag";

/**
 * Stats for the couch: the same numbers as the desk Stats page, sized for a
 * TV and read at a glance. Nothing to operate; B goes back to the games.
 */
export function ConsoleStats({ version, onArt }: { version: number; onArt: (src: string | null) => void }) {
  const name = useGamerTag();
  const [data, setData] = useState<{ games: Game[]; sessions: PlaySession[] } | null>(null);
  useEffect(() => {
    Promise.all([api.listGames({ install: "all", stores: [], search: null, hidden_only: false }), api.playSessions()])
      .then(([games, sessions]) => setData({ games, sessions }))
      .catch(() => setData({ games: [], sessions: [] }));
  }, [version]);
  const s = useMemo(() => (data ? computeStats(data.games, data.sessions) : null), [data]);

  // The most played game's art fills the screen behind the numbers.
  const top = s?.top[0]?.game;
  useEffect(() => onArt(top?.hero ?? top?.cover ?? null), [top, onArt]);

  if (!s) return <div className="cx-stats" />;
  const total = Math.round(s.totalSecs / 3600);
  const figures = [
    { label: "This week", value: hours(s.thisWeekSecs) },
    { label: "This month", value: hours(s.thisMonthSecs) },
    ...(s.year ? [{ label: `In ${s.year.year}`, value: hours(s.year.secs) }] : []),
    ...(s.longest ? [{ label: "Longest session", value: formatPlaytime(s.longest.secs) }] : []),
  ];

  return (
    <div className="cx-stats">
      <header className="cx-stats-head">
        <p className="cx-kicker">{name ? `${possessive(name)} playtime` : "All your playtime"}</p>
        <h1 className="cx-total">
          {s.totalSecs === 0 ? "No playtime yet" : total === 0 ? "Under an hour" : `${total.toLocaleString()} ${total === 1 ? "hour" : "hours"}`}
        </h1>
        <p className="cx-sub">
          {s.played} of {s.games} games played · {s.installed} installed
        </p>
      </header>

      <dl className="cx-figures">
        {figures.map((f) => (
          <div key={f.label}>
            <dt>{f.label}</dt>
            <dd>{f.value}</dd>
          </div>
        ))}
      </dl>

      {s.top.length > 0 && (
        <section className="cx-most" aria-label="Most played">
          <h2>Most played</h2>
          <ol>
            {s.top.slice(0, 5).map(({ game, secs }, i) => (
              <li key={game.id}>
                <span className="cx-most-art">
                  <Art src={game.cover ?? game.hero} title={game.title} />
                  <span className="cx-rank">{i + 1}</span>
                </span>
                <b>{game.title}</b>
                <span>{hours(secs)}</span>
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  );
}
