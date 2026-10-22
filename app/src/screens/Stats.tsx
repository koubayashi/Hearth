import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { computeStats, hours, type Stats as StatsData, type Week } from "../lib/stats";
import { formatPlaytime } from "../lib/format";
import { STATUSES, STORE_LABEL, type Game, type PlaySession } from "../lib/types";
import { StoreIcon } from "../components/StoreIcon";

/** Playtime and library numbers: store totals for all-time, Hearth's own sessions for anything dated. */
export function Stats({ version }: { version: number }) {
  const [data, setData] = useState<{ games: Game[]; sessions: PlaySession[] } | null>(null);

  useEffect(() => {
    Promise.all([
      api.listGames({ install: "all", stores: [], search: null, hidden_only: false }),
      api.playSessions(),
    ]).then(([games, sessions]) => setData({ games, sessions }));
  }, [version]);

  const s = useMemo(() => (data ? computeStats(data.games, data.sessions) : null), [data]);
  if (!s) return <div className="scroll" />;

  return (
    <div className="scroll stats-scroll">
      <div className="stats">
        <h1>Stats</h1>

        <div className="stat-tiles glass">
          <Tile value={hours(s.totalSecs)} label="Total playtime" />
          <Tile value={`${s.played}`} unit={`of ${s.games}`} label="Games played" />
          <Tile value={`${s.installed}`} label="Installed" />
          <Tile value={hours(s.thisWeekSecs)} label="This week" />
        </div>

        <section>
          <h2>
            Activity <small>last {s.weeks.length} weeks</small>
          </h2>
          <Activity s={s} />
        </section>

        {s.top.length > 0 && (
          <section>
            <h2>Most played</h2>
            <ol className="bar-list">
              {s.top.map(({ game, secs }, i) => (
                <li key={game.id} className="bar-row">
                  <span className="rank">{i + 1}</span>
                  {game.cover ? <img className="thumb" src={game.cover} alt="" loading="lazy" /> : <span className="thumb" />}
                  <span className="name" title={game.title}>
                    {game.title}
                  </span>
                  <Bar value={secs} max={s.top[0].secs} />
                  <span className="value" title={formatPlaytime(secs)}>
                    {hours(secs)}
                  </span>
                </li>
              ))}
            </ol>
          </section>
        )}

        <div className="stats-pair">
          <section>
            <h2>By store</h2>
            <ol className="bar-list compact">
              {s.stores.map((st) => (
                <li key={st.store} className="bar-row">
                  <span className="store-mark">
                    <StoreIcon store={st.store} size={16} />
                  </span>
                  <span className="name">
                    {STORE_LABEL[st.store]} <small>{st.games} {st.games === 1 ? "game" : "games"}</small>
                  </span>
                  <Bar value={st.secs} max={s.stores[0].secs} />
                  <span className="value">{hours(st.secs)}</span>
                </li>
              ))}
            </ol>
          </section>

          <section>
            <h2>Library</h2>
            <div className="split" role="img" aria-label={`${s.played} played, ${s.games - s.played} never played`}>
              <span style={{ flexGrow: s.played }} />
              <span style={{ flexGrow: s.games - s.played }} />
            </div>
            <p className="split-legend">
              <span><i className="key played" />{s.played} played</span>
              <span><i className="key" />{s.games - s.played} never played</span>
            </p>
            <dl className="status-list">
              {STATUSES.map((st) => (
                <div key={st.id}>
                  <dt>{st.label}</dt>
                  <dd>{s.statuses[st.id]}</dd>
                </div>
              ))}
            </dl>
          </section>
        </div>
      </div>
    </div>
  );
}

function Tile({ value, unit, label }: { value: string; unit?: string; label: string }) {
  return (
    <div className="tile">
      <b>
        {value}
        {unit && <small> {unit}</small>}
      </b>
      <span>{label}</span>
    </div>
  );
}

function Bar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  return (
    <span className="bar">
      {value > 0 && <span style={{ width: `${Math.max(pct, 1)}%` }} />}
    </span>
  );
}

const weekLabel = (w: Week) =>
  new Date(w.start * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short" });

/** Weekly playtime from recorded sessions, with a per-week hover readout. */
function Activity({ s }: { s: StatsData }) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(...s.weeks.map((w) => w.secs));

  if (s.sessions === 0) {
    return (
      <div className="activity glass empty-activity">
        <p>
          <b>Nothing recorded yet.</b> Hearth times every session from here on, whether you start a game from Hearth,
          its store or its folder. Your weeks fill in from the next time you play.
        </p>
      </div>
    );
  }

  const shown = hover === null ? null : s.weeks[hover];
  // A month label under the first week that starts in that month. The
  // leading partial month is skipped when the next label would crowd it.
  const monthOf = (i: number) => new Date(s.weeks[i].start * 1000).getMonth();
  const month = (w: Week, i: number) => {
    const starts = i === 0 || monthOf(i - 1) !== monthOf(i);
    const crowded = i === 0 && [1, 2].some((j) => j < s.weeks.length && monthOf(j) !== monthOf(0));
    return starts && !crowded ? new Date(w.start * 1000).toLocaleDateString(undefined, { month: "short" }) : "";
  };

  return (
    <div className="activity glass">
      <div className="activity-head">
        {shown ? (
          <>
            <b>{formatPlaytime(shown.secs)}</b>
            <span>
              Week of {weekLabel(shown)}
              {shown.games > 0 && ` · ${shown.games} ${shown.games === 1 ? "game" : "games"}`}
            </span>
          </>
        ) : (
          <>
            <b>{hours(s.thisMonthSecs)}</b>
            <span>
              this month
              {s.year && ` · ${hours(s.year.secs)} in ${s.year.year} across ${s.year.games} ${s.year.games === 1 ? "game" : "games"}`}
              {s.year?.top && `, most of it ${s.year.top.title}`}
            </span>
          </>
        )}
      </div>
      <div className="weeks" onMouseLeave={() => setHover(null)}>
        <span className="peak">{hours(max)}</span>
        {s.weeks.map((w, i) => (
          <div
            key={w.start}
            className="week"
            data-on={hover === i || undefined}
            onMouseEnter={() => setHover(i)}
            aria-label={`Week of ${weekLabel(w)}: ${formatPlaytime(w.secs)}`}
          >
            {w.secs > 0 && <span style={{ height: `${Math.max((w.secs / max) * 100, 2)}%` }} />}
            <small>{month(w, i)}</small>
          </div>
        ))}
      </div>
      {s.longest && s.longest.game && (
        <p className="activity-foot">
          Longest session: {formatPlaytime(s.longest.secs)} of {s.longest.game.title} · {s.sessions}{" "}
          {s.sessions === 1 ? "session" : "sessions"} recorded
        </p>
      )}
    </div>
  );
}
