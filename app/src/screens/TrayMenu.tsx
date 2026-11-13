import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { useNowSecs } from "../lib/clock";
import { formatLastPlayed } from "../lib/format";
import type { Game, LibraryQuery, NowPlaying } from "../lib/types";
import { readyVersion, useUpdate } from "../lib/updates";
import { Art } from "../components/Art";
import { Icon } from "../components/Icon";
import { PauseButton, SittingText } from "../components/Timing";

/** Recent games in the menu, as in the Windows menu it replaces. */
const RECENT = 5;
const ALL: LibraryQuery = { install: "all", stores: [], search: null, hidden_only: false };

/**
 * The tray icon's right-click menu, in its own small window (tray.rs): the
 * update when one waits, each game being timed with Pause and stop timing,
 * recent games to start, Open and Quit. It works like a real menu: arrow
 * keys move, Enter acts, Esc or a click elsewhere closes it.
 */
export function TrayMenu() {
  const [games, setGames] = useState<Game[]>([]);
  const [playing, setPlaying] = useState<NowPlaying[]>([]);
  // Bumped when the menu is asked for: measure, then place and show it.
  const [opened, setOpened] = useState(0);
  const update = useUpdate();
  const ref = useRef<HTMLDivElement>(null);
  const now = useNowSecs(playing.length > 0, 15_000);

  const load = useCallback(async () => {
    const [g, p] = await Promise.all([api.listGames(ALL), api.nowPlaying()]);
    setGames(g);
    setPlaying(p);
    setOpened((n) => n + 1);
  }, []);

  useEffect(() => {
    void load();
    const offs = [
      api.onTrayOpen(() => void load()),
      api.onNowPlaying((p) => setPlaying(p as NowPlaying[])),
      api.onLibraryChanged(() => void api.listGames(ALL).then(setGames)),
    ];
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, [load]);

  // Opened: size to the contents and show at the pointer (without focus;
  // the keys arrive from Rust, see `key` below).
  useLayoutEffect(() => {
    if (!opened || !ref.current) return;
    void api.trayMenuFit(Math.ceil(ref.current.getBoundingClientRect().height), true);
  }, [opened]);
  // Contents changed height while it's open (a pause note, an update, the
  // Recent list): resize only.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => void api.trayMenuFit(Math.ceil(el.getBoundingClientRect().height), false));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // A hidden game being timed isn't in the library list; fetch it on its own
  // so it still gets its card with Pause and Stop.
  const [hiddenTimed, setHiddenTimed] = useState<Game[]>([]);
  const missing = playing
    .map((p) => p.game_id)
    .filter((id) => !games.some((g) => g.id === id))
    .join(",");
  useEffect(() => {
    if (!missing) return setHiddenTimed([]);
    let live = true;
    Promise.all(missing.split(",").map((id) => api.getGame(Number(id))))
      .then((gs) => live && setHiddenTimed(gs.filter((g): g is Game => !!g)))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [missing]);

  const byId = new Map([...games, ...hiddenTimed].map((g) => [g.id, g]));
  const timed = playing.map((p) => ({ p, g: byId.get(p.game_id) })).filter((x): x is { p: NowPlaying; g: Game } => !!x.g);
  const recent = games
    .filter((g) => g.installed && g.last_played !== null && !g.hidden && !playing.some((p) => p.game_id === g.id))
    .sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0))
    .slice(0, RECENT);
  const pausedOnes = timed.filter(({ p }) => p.paused);
  const version = readyVersion(update);
  const restarting = update?.phase === "installing";

  const close = () => void api.trayMenuHide();
  const play = (g: Game) => {
    close();
    api.launch(g.id).catch(() => void api.openHearth());
  };

  // The menu never takes focus (so the hidden-icons panel it opened from
  // stays open, as for any app's menu). Rust hands it the menu keys while it
  // shows, and the row they reach is marked here instead of focused.
  const key = useCallback((k: string) => {
    const items = [...(ref.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    const at = items.findIndex((b) => b.hasAttribute("data-kbd"));
    const go = (i: number) => {
      items.forEach((b) => b.removeAttribute("data-kbd"));
      items[(i + items.length) % items.length]?.setAttribute("data-kbd", "");
    };
    switch (k) {
      case "Escape":
        void api.trayMenuHide();
        break;
      case "Enter":
        items[at]?.click();
        break;
      case "ArrowDown":
        go(at + 1);
        break;
      case "ArrowUp":
        go(at < 0 ? items.length - 1 : at - 1);
        break;
      case "Home":
        go(0);
        break;
      case "End":
        go(items.length - 1);
        break;
    }
  }, []);
  useEffect(() => {
    const off = api.onTrayKey((k) => key(String(k)));
    return () => void off.then((f) => f());
  }, [key]);
  // Opened again, or the pointer is used: no row marked by the keys.
  const clearKeys = () => ref.current?.querySelectorAll("[data-kbd]").forEach((b) => b.removeAttribute("data-kbd"));
  useEffect(clearKeys, [opened]);

  const onKey = (e: React.KeyboardEvent) => {
    if (!["Escape", "Enter", "ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    e.preventDefault();
    key(e.key);
  };

  return (
    <div ref={ref} className="tray" role="menu" aria-label="Hearth" tabIndex={-1} onKeyDown={onKey} onPointerMove={clearKeys}>
      {version && (
        <>
          <div className="tray-row tray-update">
            <span className="tray-dot" aria-hidden />
            <span className="t">Hearth {version} is ready</span>
            <button className="btn sm btn-primary" role="menuitem" disabled={restarting} onClick={() => void api.installUpdate()}>
              {restarting ? "Restarting…" : "Restart"}
            </button>
          </div>
          <div className="tray-sep" />
        </>
      )}

      {timed.length > 0 && (
        <>
          {timed.map(({ p, g }) => (
            <div key={g.id} className={`tray-card${p.paused ? " paused" : ""}`}>
              {g.hero && <Art src={g.hero} title="" className="tray-card-art" />}
              <span className="tray-thumb">
                <Art src={g.cover} title="" />
              </span>
              <span className="two">
                <span className="t">{g.title}</span>
                <SittingText session={p} now={now} />
              </span>
              <span className="acts">
                <PauseButton session={p} />
                <button
                  className="btn sm icon"
                  role="menuitem"
                  aria-label={`Stop timing ${g.title}`}
                  title="Stop timing (the game keeps running)"
                  onClick={() => void api.stopTiming(g.id)}
                >
                  <Icon name="stop" size={11} />
                </button>
              </span>
            </div>
          ))}
          {pausedOnes.length > 0 && (
            <p className="tray-note">
              {pausedOnes.length === 1 ? `Picks up when you're back in ${pausedOnes[0].g.title}.` : "Each picks up when you're back in it."}
            </p>
          )}
          <div className="tray-sep" />
        </>
      )}

      {recent.length > 0 ? (
        <>
          <div className="tray-cap">Recent</div>
          {recent.map((g) => (
            <button key={g.id} className="tray-row tray-game" role="menuitem" title={g.title} onClick={() => play(g)}>
              <span className="tray-thumb">
                <Art src={g.cover} title="" />
              </span>
              <span className="t">{g.title}</span>
              <span className="m when">{formatLastPlayed(g.last_played)}</span>
              <span className="m go">Play</span>
            </button>
          ))}
        </>
      ) : (
        timed.length === 0 && <p className="tray-note">Games you play show up here, ready to start again.</p>
      )}

      <div className="tray-sep" />
      <button className="tray-row" role="menuitem" onClick={() => void api.openHearth()}>
        <span className="ic">
          <Icon name="flame" size={16} />
        </span>
        <span className="t">Open Hearth</span>
      </button>
      <button className="tray-row" role="menuitem" onClick={() => void api.quitHearth()}>
        <span className="ic">
          <Icon name="power" size={15} />
        </span>
        <span className="t">Quit Hearth</span>
        {timed.length > 0 && <span className="m">Stops timing</span>}
      </button>
    </div>
  );
}
