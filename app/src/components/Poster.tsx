import { forwardRef, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, GameDetails, InstallInfo } from "../lib/types";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { formatBytes, formatLastPlayed, formatPlaytime, releaseYear } from "../lib/format";
import { usePrefs } from "../lib/prefs";
import { Art } from "./Art";
import { Icon } from "./Icon";
import { StoreIcon } from "./StoreIcon";
import { StoreMatch } from "./StoreMatch";
import { ArtworkPicker } from "./ArtworkPicker";

/** Store-page details and install info for a game, loaded when it's selected. */
function useGameInfo(game: Game | null, nonce: number) {
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
    onPrimary: (g: Game) => void;
    onToggleHidden: (g: Game) => void;
    onToggleFavorite: (g: Game) => void;
    onBack: () => void;
    /** Open a picker from outside (the right-click menu). */
    request?: { action: "artwork" | "match"; nonce: number } | null;
  }
>(function Poster({ game, playingSince, onPrimary, onToggleHidden, onToggleFavorite, onBack, request }, primaryRef) {
  const [menu, setMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const [matching, setMatching] = useState(false);
  const [picking, setPicking] = useState(false);
  const [nonce, setNonce] = useState(0);
  const { details, install } = useGameInfo(game, nonce);
  const { showScores } = usePrefs();

  useEffect(() => {
    setMenu(false);
    setMatching(false);
    setPicking(false);
  }, [game?.id]);
  useEffect(() => {
    if (!request) return;
    setPicking(request.action === "artwork");
    setMatching(request.action === "match");
  }, [request]);
  useEffect(() => {
    if (!menu) return;
    menuRef.current?.querySelector("button")?.focus();
    const close = (e: MouseEvent) => !menuRef.current?.contains(e.target as Node) && setMenu(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [menu]);

  if (!game) {
    return (
      <aside className="poster">
        <p className="poster-empty">Select a game to see it here.</p>
      </aside>
    );
  }

  const installStore = STORE_LABEL[game.entries[0]?.store ?? "manual"];
  const year = releaseYear(details?.release_date ?? null);
  const studio = details?.developers[0];
  const byline = [year, studio].filter(Boolean).join(" · ");

  return (
    <aside
      className="poster"
      aria-label={`${game.title} details`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          if (menu) setMenu(false);
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
      {/* A blurred copy fills the panel; the cover itself keeps its true shape. */}
      <div className="poster-art" key={`bg-${game.id}`} aria-hidden>
        <Art src={game.cover ?? game.hero} title="" />
      </div>
      <div className="poster-cover" key={game.id}>
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

        {(game.status || (details && ((showScores && details.metacritic != null) || details.genres.length > 0))) && (
          <div className="facts">
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
          <dd>{formatPlaytime(game.playtime_secs)}</dd>
          <dt>Last played</dt>
          <dd>{formatLastPlayed(game.last_played)}</dd>
          <dt>Status</dt>
          <dd className={playingSince ? "playing" : undefined}>
            {playingSince
              ? `Playing now · ${formatPlaytime(Math.max(60, Math.floor(Date.now() / 1000) - playingSince))}`
              : game.installed
                ? "Ready to play"
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

        <div className="actions">
          <button ref={primaryRef} className="primary" data-nav onClick={() => onPrimary(game)}>
            <Icon name={game.installed ? "play" : "download"} size={16} />
            {game.installed ? "Play" : `Install from ${installStore}`}
          </button>
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
            className="icon-btn"
            data-nav
            aria-label="More options"
            aria-haspopup="menu"
            aria-expanded={menu}
            onClick={() => setMenu((m) => !m)}
          >
            <Icon name="more" />
          </button>
          {menu && (
            <div className="menu" role="menu" ref={menuRef}>
              <button role="menuitem" onClick={() => (setMenu(false), api.openStorePage(game.id))}>
                <Icon name="external" />
                Open store page
              </button>
              {game.installed && (
                <button role="menuitem" onClick={() => (setMenu(false), api.openInstallFolder(game.id))}>
                  <Icon name="folder" />
                  Open install folder
                </button>
              )}
              <button role="menuitem" onClick={() => (setMenu(false), setPicking(true))}>
                <Icon name="library" />
                Change artwork…
              </button>
              <button role="menuitem" onClick={() => (setMenu(false), setMatching(true))}>
                <Icon name="search" />
                Find on Steam store…
              </button>
              <button role="menuitem" onClick={() => onToggleHidden(game)}>
                <Icon name={game.hidden ? "eye" : "hidden"} />
                {game.hidden ? "Show in library" : "Hide from library"}
              </button>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
});
