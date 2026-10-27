import { forwardRef, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, GameDetails, InstallInfo, Via } from "../lib/types";
import { defaultWay } from "../lib/ways";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { formatBytes, formatGameLastPlayed, formatGamePlaytime, formatPlaytime, releaseYear } from "../lib/format";
import { usePrefs } from "../lib/prefs";
import { Art } from "./Art";
import { Icon } from "./Icon";
import { StoreIcon } from "./StoreIcon";
import { StoreMatch } from "./StoreMatch";
import { ArtworkPicker } from "./ArtworkPicker";
import { GameMenu, type PanelAction } from "./GameMenu";
import { WaysToPlay } from "./WaysToPlay";

/** Store-page details and install info for a game, loaded when it's selected. */
export function useGameInfo(game: Game | null, nonce: number) {
  const [details, setDetails] = useState<GameDetails | null>(null);
  const [install, setInstall] = useState<InstallInfo | null>(null);
  const id = game?.id ?? null;
  const installed = game?.installed ?? false;

  useEffect(() => {
    setDetails(null);
    setInstall(null);
    if (id === null) return;
    let live = true;
    // A short pause so scrolling through covers doesn't fire a lookup per cover.
    const t = setTimeout(() => {
      api.gameDetails(id).then((d) => live && setDetails(d)).catch(() => {});
      if (installed) api.installInfo(id, false).then((i) => live && setInstall(i)).catch(() => {});
    }, 180);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [id, installed, nonce]);

  return { details, install };
}

function scoreTone(score: number) {
  return score >= 75 ? "good" : score >= 50 ? "mixed" : "bad";
}

/** The selected game as a tall framed poster beside the library. */
export const Poster = forwardRef<
  HTMLButtonElement,
  {
    game: Game | null;
    /** Unix seconds when the tracker saw this game start, if it is running now. */
    playingSince: number | null;
    onPrimary: (g: Game, via?: Via) => void;
    onToggleHidden: (g: Game) => void;
    onToggleFavorite: (g: Game) => void;
    onBack: () => void;
    /** Open a picker from outside (the right-click menu). */
    request?: { action: PanelAction; nonce: number } | null;
  }
>(function Poster({ game, playingSince, onPrimary, onToggleHidden, onToggleFavorite, onBack, request }, primaryRef) {
  // Where the "More options" button sits while its menu is open.
  const [menu, setMenu] = useState<DOMRect | null>(null);
  const moreRef = useRef<HTMLButtonElement>(null);
  const [matching, setMatching] = useState(false);
  const [picking, setPicking] = useState(false);
  const [ways, setWays] = useState(false);
  // The arrow beside Play: its menu of ways to play, anchored to it.
  const [playMenu, setPlayMenu] = useState<DOMRect | null>(null);
  const waysRef = useRef<HTMLButtonElement>(null);
  const [nonce, setNonce] = useState(0);
  const { details, install } = useGameInfo(game, nonce);
  const { showScores, blurPosterArt } = usePrefs();

  useEffect(() => {
    setMenu(null);
    setPlayMenu(null);
    setMatching(false);
    setPicking(false);
    setWays(false);
  }, [game?.id]);
  useEffect(() => {
    if (!request) return;
    setPicking(request.action === "artwork");
    setMatching(request.action === "match");
    setWays(request.action === "ways");
  }, [request]);

  // Only while the library loads: the frame holds its place, quietly.
  if (!game) return <aside className="poster" aria-hidden />;

  const installStore = STORE_LABEL[game.entries[0]?.store ?? "manual"];
  const year = releaseYear(details?.release_date ?? null);
  const studio = details?.developers[0];
  const byline = [year, studio].filter(Boolean).join(" · ");

  return (
    <aside
      className={`poster${game.hero ? " has-hero" : ""}${blurPosterArt ? " soft-art" : ""}`}
      aria-label={`${game.title} details`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          if (menu) setMenu(null);
          else onBack();
        }
      }}
    >
      {picking && (
        <ArtworkPicker
          game={game}
          onClose={() => setPicking(false)}
          onDone={() => {
            setPicking(false);
            setNonce((n) => n + 1);
          }}
        />
      )}
      {ways && <WaysToPlay game={game} onClose={() => setWays(false)} />}
      {matching && (
        <StoreMatch
          game={game}
          onClose={() => setMatching(false)}
          onDone={() => {
            setMatching(false);
            setNonce((n) => n + 1);
          }}
        />
      )}
      <div className="poster-cover" key={game.id}>
        {/* A blurred copy right behind the cover: a soft halo in its colours,
            wherever the cover sits, instead of a haze over the whole panel. */}
        {/* The wide hero art fills whatever height is spare above the cover;
            without one, a soft halo of the cover's colours instead. */}
        {game.hero ? (
          <div className="poster-hero" aria-hidden>
            <Art src={game.hero} title="" />
          </div>
        ) : (
          <div className="cover-halo" aria-hidden>
            <Art src={game.cover ?? game.hero} title="" />
          </div>
        )}
        <Art src={game.cover ?? game.hero} title={game.title} />
      </div>
      <div className="poster-body">
        {/* Cover art already carries the game's logo, so the title stays plain text. */}
        <h1 className="poster-title">{game.title}</h1>
        <div className="where">
          {[...new Set(game.entries.map((e) => e.store))].map((s) => (
            <span key={s}>
              <StoreIcon store={s} size={15} />
              {STORE_LABEL[s]}
            </span>
          ))}
          {byline && <span className="byline">{byline}</span>}
        </div>

        {(game.hidden || game.status || (details && ((showScores && details.metacritic != null) || details.genres.length > 0))) && (
          <div className="facts">
            {game.hidden && (
              <span className="tag hidden-tag">
                <Icon name="hidden" size={13} />
                Hidden
              </span>
            )}
            {game.status && (
              <span className="tag status-chip">
                <span className={`status-dot ${game.status}`} />
                {STATUSES.find((s) => s.id === game.status)?.label}
              </span>
            )}
            {showScores && details?.metacritic != null && (
              <span className={`score ${scoreTone(details.metacritic)}`} title="Metacritic score">
                {details.metacritic}
              </span>
            )}
            {details?.genres.slice(0, 3).map((g) => (
              <span key={g} className="tag">
                {g}
              </span>
            ))}
          </div>
        )}

        <dl>
          <dt>Played</dt>
          <dd>{formatGamePlaytime(game.playtime_secs, game.last_played)}</dd>
          <dt>Last played</dt>
          <dd>{formatGameLastPlayed(game.last_played, game.playtime_secs)}</dd>
          <dt>Status</dt>
          <dd className={playingSince ? "playing" : undefined}>
            {playingSince
              ? `Playing now · ${formatPlaytime(Math.max(60, Math.floor(Date.now() / 1000) - playingSince))}`
              : game.installed
                ? "Ready to play"
                : game.offline_drive
                  ? `Drive ${game.offline_drive} not connected`
                  : "Not installed"}
          </dd>
          {details?.release_date && (
            <>
              <dt>Released</dt>
              <dd>{details.release_date}</dd>
            </>
          )}
          {details && details.publishers.length > 0 && details.publishers[0] !== studio && (
            <>
              <dt>Publisher</dt>
              <dd>{details.publishers[0]}</dd>
            </>
          )}
        </dl>

        {details && details.features.length > 0 && (
          <ul className="features" aria-label="Features">
            {details.features.slice(0, 6).map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        )}

        {install && (
          <button
            className="install-row"
            data-nav
            title={`Open ${install.path}`}
            onClick={() => api.openInstallFolder(game.id)}
          >
            <Icon name="folder" size={15} />
            <span className="path">{install.path}</span>
            {install.bytes != null && <span className="size">{formatBytes(install.bytes)}</span>}
          </button>
        )}

        {game.hidden && (
          <button className="restore" data-nav onClick={() => onToggleHidden(game)}>
            <Icon name="eye" size={16} />
            Show in library
          </button>
        )}
        <div className="actions">
          {game.offline_drive ? (
            // Nothing to start until the drive is back; it updates by itself.
            <button
              ref={primaryRef}
              className="primary"
              data-nav
              disabled
              title={`Connect drive ${game.offline_drive} to play. Hearth notices when it's back.`}
            >
              <Icon name="folder" size={16} />
              Drive not connected
            </button>
          ) : (
            <button ref={primaryRef} className="primary" data-nav onClick={() => onPrimary(game)}>
              <Icon name={game.installed || defaultWay(game) ? "play" : "download"} size={16} />
              {defaultWay(game) ? (
                <>
                  Play <small className="via">{defaultWay(game)!.name}</small>
                </>
              ) : game.installed ? (
                "Play"
              ) : (
                `Install from ${installStore}`
              )}
            </button>
          )}
          {game.launch_options.length > 0 && (
            <button
              ref={waysRef}
              className="icon-btn"
              data-nav
              aria-label="Other ways to play"
              title="Other ways to play"
              aria-haspopup="menu"
              aria-expanded={playMenu !== null}
              onMouseDown={(e) => playMenu && e.stopPropagation()}
              onClick={(e) => {
                const at = e.currentTarget.getBoundingClientRect();
                setPlayMenu((m) => (m ? null : at));
              }}
            >
              <Icon name="chevronDown" />
            </button>
          )}
          {playMenu && (
            <GameMenu
              game={game}
              x={0}
              y={0}
              anchor={playMenu}
              start="play"
              onClose={() => {
                setPlayMenu(null);
                waysRef.current?.focus();
              }}
              onPrimary={onPrimary}
              onToggleFavorite={onToggleFavorite}
              onToggleHidden={onToggleHidden}
              onPanel={(action) => (action === "ways" ? setWays(true) : action === "artwork" ? setPicking(true) : setMatching(true))}
            />
          )}
          <button
            className="icon-btn"
            data-nav
            aria-label={game.favorite ? "Remove from favourites" : "Add to favourites"}
            aria-pressed={game.favorite}
            onClick={() => onToggleFavorite(game)}
          >
            <Icon name="star" />
          </button>
          <button
            ref={moreRef}
            className="icon-btn"
            data-nav
            aria-label="More options"
            aria-haspopup="menu"
            aria-expanded={menu !== null}
            // While open, a press here should close the menu, not reopen it.
            onMouseDown={(e) => menu && e.stopPropagation()}
            onClick={(e) => {
              const at = e.currentTarget.getBoundingClientRect();
              setMenu((m) => (m ? null : at));
            }}
          >
            <Icon name="more" />
          </button>
          {menu && (
            <GameMenu
              game={game}
              x={0}
              y={0}
              anchor={menu}
              onClose={() => {
                setMenu(null);
                moreRef.current?.focus();
              }}
              onPrimary={onPrimary}
              onToggleFavorite={onToggleFavorite}
              onToggleHidden={onToggleHidden}
              onPanel={(action) => (action === "ways" ? setWays(true) : action === "artwork" ? setPicking(true) : setMatching(true))}
            />
          )}
        </div>
      </div>
    </aside>
  );
});
