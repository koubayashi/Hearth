import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { computeStats, hours, type Stats as StatsData, type Week } from "../lib/stats";
import { formatPlaytime } from "../lib/format";
import { STATUSES, STORE_LABEL, type Game, type GameStatus, type PlaySession } from "../lib/types";
import { StoreIcon } from "../components/StoreIcon";
import { possessive, useGamerTag } from "../lib/gamertag";

/** Playtime and library numbers: store totals for all-time, Hearth's own sessions for anything dated. */
export function Stats({
  version,
  onOpenGame,
  onOpenStatus,
}: {
  version: number;
  /** Show this game in the library. */
  onOpenGame: (g: Game) => void;
  /** Show this status's shelf in Collections. */
  onOpenStatus: (status: GameStatus) => void;
}) {
  const [data, setData] = useState<{ games: Game[]; sessions: PlaySession[] } | null>(null);
  const name = useGamerTag();

  useEffect(() => {
    Promise.all([
      api.listGames({ install: "all", stores: [], search: null, hidden_only: false }),
      api.playSessions(),
    ]).then(([games, sessions]) => setData({ games, sessions }));
  }, [version]);

  const s = useMemo(() => (data ? computeStats(data.games, data.sessions) : null), [data]);
  if (!s) return <div className="scroll" />;

  const podium = s.top.slice(0, 3);
  const rest = s.top.slice(3);
  const stores = s.stores.filter((st) => st.secs > 0);

  return (
    <div className="scroll stats-scroll">
      <div className="stats">
        <h1>{name ? `${possessive(name)} stats` : "Stats"}</h1>

        <Headline s={s} onOpenGame={onOpenGame} />

        <section>
          <h2>
            Activity <small>last {s.weeks.length} weeks</small>
          </h2>
          <Activity s={s} />
        </section>

        {podium.length > 0 && (
          <section>
            <h2>Most played</h2>
            <ol className="podium">
              {podium.map(({ game, secs }, i) => (
                <li key={game.id}>
                  <button className="podium-item" data-nav onClick={() => onOpenGame(game)} title={`Show ${game.title} in the library`}>
                    {game.cover ? <img src={game.cover} alt="" loading="lazy" /> : <span className="podium-art" />}
                    <span className="podium-text">
                      <span className="rank">{i + 1}</span>
                      <b>{game.title}</b>
                      <span>{formatPlaytime(secs)}</span>
                    </span>
                  </button>
                </li>
              ))}
            </ol>
            {rest.length > 0 && (
              <ol className="ranked" start={4} style={{ ["--rows" as string]: Math.ceil(rest.length / 2) }}>
                {rest.map(({ game, secs }, i) => (
                  <li key={game.id}>
                    <button data-nav onClick={() => onOpenGame(game)} title={`Show ${game.title} in the library`}>
                      <span className="rank">{i + 4}</span>
                      <span className="name">{game.title}</span>
                      <span className="value">{hours(secs)}</span>
                    </button>
                  </li>
                ))}
              </ol>
            )}
          </section>
        )}

        <div className="stats-pair">
          {stores.length > 0 && (
            <section>
              <h2>By store</h2>
              <ul className="figures">
                {stores.map((st) => (
                  <li key={st.store}>
                    <span className="figure-label">
                      <StoreIcon store={st.store} size={15} />
                      {STORE_LABEL[st.store]}
                    </span>
                    <b>{hours(st.secs)}</b>
                    <small>
                      {st.games} {st.games === 1 ? "game" : "games"}
                    </small>
                  </li>
                ))}
              </ul>
            </section>
          )}

          <section>
            <h2>Library</h2>
            <p className="library-line">
              <b>{s.played}</b> of {s.games} games played
            </p>
            <ul className="figures statuses">
              {STATUSES.map((st) => (
                <li key={st.id}>
                  <button data-nav onClick={() => onOpenStatus(st.id)} title={`Show ${st.label} in Collections`}>
                    <span className="figure-label">
                      <span className={`status-dot ${st.id}`} />
                      {st.label}
                    </span>
                    <b>{s.statuses[st.id]}</b>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        </div>
      </div>
    </div>
  );
}

/**
 * The headline: the totals in one sentence, over the art of the game you
 * have played most, which is also a way into it.
 */
function Headline({ s, onOpenGame }: { s: StatsData; onOpenGame: (g: Game) => void }) {
  const top = s.top[0];
  const art = top?.game.hero ?? top?.game.cover ?? null;
  const h = Math.round(s.totalSecs / 3600);
  const total = s.totalSecs === 0 ? "No playtime yet" : h === 0 ? "Under an hour" : `${h.toLocaleString()} ${h === 1 ? "hour" : "hours"}`;
  return (
    <header className={`stats-hero${art ? " has-art" : ""}`}>
      {art && <img className="stats-hero-art" src={art} alt="" />}
      <div className="stats-hero-text">
        <p className="stats-hero-total">
          {total}
          <span> across {s.played} {s.played === 1 ? "game" : "games"}</span>
        </p>
        <p className="stats-hero-sub">
          {s.installed} installed
          {s.sessions > 0 && `, ${hours(s.thisWeekSecs)} played this week`}
        </p>
      </div>
      {top && (
        <button className="stats-hero-game" data-nav onClick={() => onOpenGame(top.game)}>
          Most played <b>{top.game.title}</b>
          <span>{formatPlaytime(top.secs)}</span>
        </button>
      )}
    </header>
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
            {w.secs > 0 && (
              <span style={{ height: `${Math.max((w.secs / max) * 100, 2)}%`, ["--i" as string]: i }} />
            )}
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
