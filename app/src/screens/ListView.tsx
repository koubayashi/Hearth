import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, NowPlaying } from "../lib/types";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { formatBytes, formatLastPlayed, formatPlaytime } from "../lib/format";
import { Art } from "../components/Art";
import { isMenuKey } from "../components/CoverGrid";
import { GameMenu, type PanelAction } from "../components/GameMenu";
import { Icon } from "../components/Icon";
import { useGameInfo } from "../components/Poster";
import { StoreIcon } from "../components/StoreIcon";
import { ArtworkPicker } from "../components/ArtworkPicker";
import { StoreMatch } from "../components/StoreMatch";

type Group = { id: string; title: string; games: Game[] };

/**
 * The library as a list of titles beside a full detail page for the selected
 * game: quicker to scan a big library by name than a wall of covers.
 */
export function ListView({
  hiddenView,
  version,
  focused,
  playing,
  onFocusGame,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
}: {
  hiddenView: boolean;
  version: number;
  focused: Game | null;
  playing: NowPlaying[];
  onFocusGame: (g: Game | null) => void;
  onPrimary: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
}) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [installedOnly, setInstalledOnly] = useState(false);
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<{ game: Game; x: number; y: number; anchor?: DOMRect } | null>(null);
  const [panel, setPanel] = useState<PanelAction | null>(null);
  const [nonce, setNonce] = useState(0);
  const playRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let live = true;
    api
      .listGames({ install: installedOnly ? "installed" : "all", stores: [], search: search || null, hidden_only: hiddenView })
      .then((g) => {
        if (!live) return;
        setGames(g);
        setLoadError(null);
      })
      .catch((e) => live && setLoadError(String(e)));
    return () => {
      live = false;
    };
  }, [search, installedOnly, hiddenView, version]);

  const groups = useMemo<Group[]>(() => {
    const list = games ?? [];
    if (hiddenView) return [{ id: "hidden", title: "Hidden", games: list }];
    return [
      { id: "installed", title: "Installed", games: list.filter((g) => g.installed) },
      { id: "offline", title: "Drive not connected", games: list.filter((g) => !g.installed && g.offline_drive) },
      { id: "rest", title: "Not installed", games: list.filter((g) => !g.installed && !g.offline_drive) },
    ].filter((g) => g.games.length > 0);
  }, [games, hiddenView]);

  // Keep a game selected: the fresh copy of the current one, else the first.
  useEffect(() => {
    if (!games) return;
    const fresh = focused && games.find((g) => g.id === focused.id);
    if (fresh) {
      if (fresh !== focused) onFocusGame(fresh);
      return;
    }
    if (games.length === 0 && search && focused) return;
    onFocusGame(games.find((g) => g.installed) ?? games[0] ?? null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [games]);

  const openMenu = (g: Game, x: number, y: number, anchor?: DOMRect) => {
    onFocusGame(g);
    setMenu({ game: g, x, y, anchor });
  };
  const toggleGroup = (id: string) =>
    setClosed((cur) => {
      const next = new Set(cur);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const backToRow = () => focused && document.querySelector<HTMLElement>(`.lv-row[data-game="${focused.id}"]`)?.focus();

  const count = games?.length ?? 0;

  return (
    <div className="listview">
      {menu && (
        <GameMenu
          game={menu.game}
          x={menu.x}
          y={menu.y}
          anchor={menu.anchor}
          onClose={() => {
            if (menu.anchor) document.querySelector<HTMLElement>(".lv-more")?.focus();
            else document.querySelector<HTMLElement>(`.lv-row[data-game="${menu.game.id}"]`)?.focus();
            setMenu(null);
          }}
          onPrimary={onPrimary}
          onToggleFavorite={onToggleFavorite}
          onToggleHidden={onToggleHidden}
          onPanel={setPanel}
        />
      )}
      {panel === "artwork" && focused && (
        <ArtworkPicker game={focused} onClose={() => setPanel(null)} onDone={() => (setPanel(null), setNonce((n) => n + 1))} />
      )}
      {panel === "match" && focused && (
        <StoreMatch game={focused} onClose={() => setPanel(null)} onDone={() => (setPanel(null), setNonce((n) => n + 1))} />
      )}

      <aside className="lv-side glass" aria-label="Games">
        <div className="lv-tools">
          <label className={`search ${search ? "has-value" : ""}`} title="Search">
            <Icon name="search" size={16} />
            <input
              data-nav
              placeholder={hiddenView ? "Search hidden" : `Search ${count} games`}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setSearch("")}
              aria-label="Search games"
            />
          </label>
          <div className="seg" role="group" aria-label="Install state">
            <button data-nav aria-pressed={!installedOnly} onClick={() => setInstalledOnly(false)}>
              All
            </button>
            <button data-nav aria-pressed={installedOnly} onClick={() => setInstalledOnly(true)}>
              Installed
            </button>
          </div>
        </div>
        <div className="lv-scroll">
          {loadError && (
            <p className="lv-note" role="alert">
              Couldn't load your library. {loadError}
            </p>
          )}
          {games && games.length === 0 && (
            <p className="lv-note">
              {search ? "No games match." : hiddenView ? "Nothing hidden. Games you hide land here." : "Your library is empty. Connect a store in Settings."}
            </p>
          )}
          {groups.map((grp) => {
            const open = !closed.has(grp.id);
            return (
              <section key={grp.id} aria-label={grp.title}>
                <button className="lv-group" data-nav aria-expanded={open} onClick={() => toggleGroup(grp.id)}>
                  <Icon name="chevronRight" size={14} />
                  {grp.title}
                  <small>{grp.games.length}</small>
                </button>
                {open &&
                  grp.games.map((g) => {
                    const isPlaying = playing.some((p) => p.game_id === g.id);
                    return (
                      <button
                        key={g.id}
                        className={`lv-row${g.installed ? "" : " dim"}`}
                        data-nav
                        data-game={g.id}
                        aria-current={focused?.id === g.id ? "true" : undefined}
                        onFocus={() => onFocusGame(g)}
                        onClick={() => onFocusGame(g)}
                        onDoubleClick={() => onPrimary(g)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            onFocusGame(g);
                            playRef.current?.focus();
                          } else if (isMenuKey(e)) {
                            e.preventDefault();
                            const r = e.currentTarget.getBoundingClientRect();
                            openMenu(g, r.left + 40, r.bottom);
                          }
                        }}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          openMenu(g, e.clientX, e.clientY);
                        }}
                      >
                        <span className="lv-thumb">
                          <Art src={g.cover ?? g.hero} title="" />
                        </span>
                        <span className="lv-name">{g.title}</span>
                        {isPlaying ? (
                          <span className="lv-live" title="Playing now" />
                        ) : (
                          g.favorite && <Icon name="star" size={13} className="lv-fav" />
                        )}
                      </button>
                    );
                  })}
              </section>
            );
          })}
        </div>
      </aside>

      {focused ? (
        <Detail
          game={focused}
          nonce={nonce}
          playRef={playRef}
          playingSince={playing.find((p) => p.game_id === focused.id)?.started_at ?? null}
          onPrimary={onPrimary}
          onToggleHidden={onToggleHidden}
          onToggleFavorite={onToggleFavorite}
          onMore={(r) => openMenu(focused, 0, 0, r)}
          onBack={backToRow}
        />
      ) : (
        <div className="lv-detail" />
      )}
    </div>
  );
}

function Detail({
  game,
  nonce,
  playRef,
  playingSince,
  onPrimary,
  onToggleHidden,
  onToggleFavorite,
  onMore,
  onBack,
}: {
  game: Game;
  nonce: number;
  playRef: React.RefObject<HTMLButtonElement | null>;
  playingSince: number | null;
  onPrimary: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onMore: (anchor: DOMRect) => void;
  onBack: () => void;
}) {
  const { details, install } = useGameInfo(game, nonce);
  const [logoFailed, setLogoFailed] = useState(false);
  useEffect(() => setLogoFailed(false), [game.id]);
  const stores = [...new Set(game.entries.map((e) => e.store))];
  const status = STATUSES.find((s) => s.id === game.status)?.label;
  const studio = details?.developers[0];
  const publisher = details?.publishers.find((p) => p !== studio);

  return (
    <section
      className="lv-detail"
      aria-label={`${game.title} details`}
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.defaultPrevented) {
          e.preventDefault();
          onBack();
        }
      }}
    >
      {/* Keyed, so the art fades in for each game; the rest stays put and keeps focus. */}
      <div className={`lv-hero${game.hero ? "" : " no-art"}`} key={game.id}>
        {game.hero ? <Art src={game.hero} title="" /> : <div className="lv-hero-blank" />}
        <div className="lv-hero-title">
          {game.logo && !logoFailed ? (
            <img src={game.logo} alt={game.title} draggable={false} onError={() => setLogoFailed(true)} />
          ) : (
            <h1>{game.title}</h1>
          )}
        </div>
      </div>

      <div className="lv-bar glass">
        {game.offline_drive ? (
          <button ref={playRef} className="lv-play" data-nav disabled title={`Connect drive ${game.offline_drive} to play.`}>
            <Icon name="folder" size={18} />
            Drive not connected
          </button>
        ) : (
          <button ref={playRef} className={`lv-play${game.installed ? "" : " install"}`} data-nav onClick={() => onPrimary(game)}>
            <Icon name={game.installed ? "play" : "download"} size={18} />
            {game.installed ? "Play" : "Install"}
          </button>
        )}
        <div className="lv-stat">
          <span>{playingSince ? "Playing now" : "Last played"}</span>
          <b>
            {playingSince
              ? formatPlaytime(Math.max(60, Math.floor(Date.now() / 1000) - playingSince))
              : formatLastPlayed(game.last_played)}
          </b>
        </div>
        <div className="lv-stat">
          <span>Play time</span>
          <b>{formatPlaytime(game.playtime_secs)}</b>
        </div>
        <span className="spacer" />
        <span className="lv-stores">
          {stores.map((s) => (
            <span key={s} title={STORE_LABEL[s]}>
              <StoreIcon store={s} size={16} />
            </span>
          ))}
        </span>
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
          className="icon-btn lv-more"
          data-nav
          aria-label="More options"
          aria-haspopup="menu"
          onClick={(e) => onMore(e.currentTarget.getBoundingClientRect())}
        >
          <Icon name="more" />
        </button>
      </div>

      <div className="lv-body">
        {game.hidden && (
          <button className="btn" data-nav onClick={() => onToggleHidden(game)}>
            <Icon name="eye" size={16} />
            Show in library
          </button>
        )}
        <dl className="lv-facts">
          {status && (
            <div>
              <dt>Status</dt>
              <dd>
                <span className={`status-dot ${game.status}`} /> {status}
              </dd>
            </div>
          )}
          {details && details.genres.length > 0 && (
            <div>
              <dt>Genre</dt>
              <dd>{details.genres.slice(0, 3).join(", ")}</dd>
            </div>
          )}
          {details?.release_date && (
            <div>
              <dt>Released</dt>
              <dd>{details.release_date}</dd>
            </div>
          )}
          {studio && (
            <div>
              <dt>Developer</dt>
              <dd>{studio}</dd>
            </div>
          )}
          {publisher && (
            <div>
              <dt>Publisher</dt>
              <dd>{publisher}</dd>
            </div>
          )}
          <div>
            <dt>Store</dt>
            <dd>{stores.map((s) => STORE_LABEL[s]).join(", ")}</dd>
          </div>
        </dl>
        {details && details.features.length > 0 && (
          <ul className="features" aria-label="Features">
            {details.features.slice(0, 8).map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        )}
        {install && (
          <button className="install-row" data-nav title={`Open ${install.path}`} onClick={() => api.openInstallFolder(game.id)}>
            <Icon name="folder" size={15} />
            <span className="path">{install.path}</span>
            {install.bytes != null && <span className="size">{formatBytes(install.bytes)}</span>}
          </button>
        )}
      </div>
    </section>
  );
}
