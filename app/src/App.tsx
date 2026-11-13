import { useCallback, useEffect, useState } from "react";
import { preloadGameInfo } from "./lib/gameInfo";
import { loadSmart } from "./lib/smart";
import { onOpenShelf, shelfKey, type Shelf } from "./lib/nav";
import { api } from "./lib/api";
import type { Game, GameStatus, NowPlaying, Via } from "./lib/types";
import { chosenWay, defaultWay, storeName } from "./lib/ways";
import { playVersion, versionEntry } from "./lib/versions";
import { Rail, type Route } from "./components/Rail";
import { TitleBar } from "./components/TitleBar";
import { Toast, type ToastMsg } from "./components/Toast";
import { Library } from "./screens/Library";
import { Collections } from "./screens/Collections";
import { Settings } from "./screens/Settings";
import { Stats } from "./screens/Stats";
import { Storage } from "./screens/Storage";
import { UninstallHost } from "./components/Uninstall";
import { loadVersionSuggestions, VersionsHost } from "./components/Versions";
import { legacy } from "./components/Art";
import { setPrefs, usePrefs } from "./lib/prefs";
import { ListView } from "./screens/ListView";
import { Console } from "./screens/Console";
import { markSeen, useUpdate } from "./lib/updates";

let unsavedPrefs: string | null = null;
function savePrefs() {
  if (unsavedPrefs === null) return;
  api.savePrefsSnapshot(unsavedPrefs).catch(() => {});
  unsavedPrefs = null;
}

export function App() {
  const [route, setRoute] = useState<Route>("library");
  // Automatic backups run in the background, window open or not: give them
  // the current appearance settings to include.
  const prefs = usePrefs();
  // Once the settings rest: dragging the accent picker changes them dozens
  // of times a second. A change still waiting is written on the way out.
  useEffect(() => {
    unsavedPrefs = JSON.stringify(prefs);
    const timer = setTimeout(savePrefs, 500);
    return () => clearTimeout(timer);
  }, [prefs]);
  useEffect(() => {
    window.addEventListener("beforeunload", savePrefs);
    return () => {
      window.removeEventListener("beforeunload", savePrefs);
      savePrefs();
    };
  }, []);
  const [focused, setFocused] = useState<Game | null>(null);
  const [version, setVersion] = useState(0);
  const [toast, setToast] = useState<ToastMsg | null>(null);
  const [playing, setPlaying] = useState<NowPlaying[]>([]);
  // While the cover size slider moves, the library shows behind Settings.
  // It stays mounted (hidden) while Settings is open, so it is ready at once.
  const [peek, setPeek] = useState(false);
  // Where you were sent: a game to show in the library (from Stats), or a
  // shelf of Collections (a status from Stats, a series from a game's panel).
  const [revealId, setRevealId] = useState<number | null>(null);
  const [startShelf, setStartShelf] = useState<Shelf | undefined>(undefined);
  const openGame = (g: Game) => {
    setFocused(g);
    setRevealId(g.id);
    setRoute("library");
  };
  const openStatus = (status: GameStatus) => {
    setStartShelf({ kind: "status", status });
    setFocused(null);
    setRoute("collections");
  };
  // The game you came from stays in the panel.
  useEffect(
    () =>
      onOpenShelf((s) => {
        setStartShelf(s);
        setRoute("collections");
      }),
    [],
  );
  // F5 refreshes everything, as Settings' "Refresh library" does. (Coming
  // back to the window already rescans what's installed: Rust watches that.)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "F5") return;
      e.preventDefault();
      if (e.repeat) return;
      say("Refreshing your library…");
      api.syncNow().catch((err) => say(`Couldn't refresh: ${err}`));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  // A controller's Start button opens the console from anywhere. The first
  // time, say how to get back, in case it was pressed by accident.
  useEffect(() => {
    if (prefs.consoleMode) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "GamepadStart") return;
      if (!prefs.consoleTipSeen) say("Console view. Press Start again to go back to the desktop.");
      setPrefs({ consoleMode: true, consoleTipSeen: true });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [prefs.consoleMode, prefs.consoleTipSeen]);
  // The taskbar jump list's Console task, or one of its games that couldn't
  // start: waiting when Hearth opened from it, else announced (jumplist.rs).
  useEffect(() => {
    const take = () =>
      api
        .takeJumpRequest()
        .then((r) => {
          if (r.console) setPrefs({ consoleMode: true });
          if (r.error) say(r.error);
        })
        .catch(() => {});
    void take();
    const off = api.onJumpList(() => void take());
    return () => void off.then((f) => f());
  }, []);
  // Escape leaves the hidden view (back to the library) and Storage (back to
  // Stats), once nothing nearer used it: the game panel, a menu or a dialog,
  // or a search box clearing itself.
  useEffect(() => {
    const parent: Partial<Record<Route, Route>> = { hidden: "library", storage: "stats" };
    const back = parent[route];
    if (!back) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      if ((e.target as HTMLElement | null)?.closest("input, textarea")) return;
      if (document.querySelector('[role="menu"], [role="dialog"]')) return;
      setRoute(back);
      setFocused(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [route]);
  const [counts, setCounts] = useState<Partial<Record<Route, number>>>({});

  const refresh = () => setVersion((v) => v + 1);
  const say = (text: string, action?: ToastMsg["action"]) => setToast({ id: Date.now(), text, action });
  const clearToast = useCallback(() => setToast(null), []);

  // After an update, once: "Updated to Hearth 0.x", until closed. Its notes
  // open in Settings › Updates. (Started in the tray, the interface isn't
  // loaded at all, so this waits for the window to open.)
  const updatedTo = useUpdate()?.updated_to ?? null;
  const [settingsAt, setSettingsAt] = useState<"updates" | undefined>(undefined);
  useEffect(() => {
    if (!updatedTo) return;
    setToast({
      id: Date.now(),
      text: `Updated to Hearth ${updatedTo}`,
      persist: true,
      onDismiss: markSeen,
      action: {
        label: "What's new",
        run: () => {
          setSettingsAt("updates");
          setRoute("settings");
          setFocused(null);
          setPrefs({ consoleMode: false });
        },
      },
    });
  }, [updatedTo]);

  useEffect(() => {
    api.gameCounts().then(setCounts);
    // Every game's panel details, ready before a game is selected.
    void preloadGameInfo();
    void loadSmart();
    void loadVersionSuggestions();
  }, [version]);

  const toggleHidden = async (g: Game) => {
    const wasHidden = g.hidden;
    await api.setHidden(g.id, !wasHidden);
    refresh();
    say(wasHidden ? `${g.title} is back in your library` : `${g.title} hidden`, {
      label: "Undo",
      run: () => api.setHidden(g.id, wasHidden).then(refresh),
    });
  };
  const toggleFavorite = async (g: Game) => {
    await api.setFavorite(g.id, !g.favorite);
    refresh();
  };
  const primary = async (g: Game, via?: Via) => {
    // Every Play button starts the version the card shows, not one Rust
    // would have to guess.
    const shown = !via && !defaultWay(g) ? playVersion(g) : null;
    if (shown) via = { kind: "entry", id: versionEntry(shown).id };
    const store = storeName(g);
    const way = chosenWay(g, via);
    if (api.demo) {
      say(way ? `Demo library: would start ${g.title} with ${way.name}` : `Demo library: ${g.installed ? "launching" : "installing"} works in the desktop app`);
      return;
    }
    try {
      await api.launch(g.id, via);
      say(way ? `Starting ${g.title} with ${way.name}` : g.installed ? `Starting ${g.title}` : `Opening ${store} to install ${g.title}`);
    } catch (e) {
      say(`Couldn't start ${g.title}: ${e}`);
    }
  };

  useEffect(() => {
    // A burst of changes (a sync's steps, background art and details) reloads
    // the library once, not once per event.
    let pending: ReturnType<typeof setTimeout> | undefined;
    const offs = [
      api.onLibraryChanged(() => {
        pending ??= setTimeout(() => {
          pending = undefined;
          refresh();
        }, 250);
      }),
      api.onSyncError((msg) => say(String(msg))),
      api.onNowPlaying((p) => {
        const next = p as NowPlaying[];
        setPlaying((prev) => {
          const started = next.find((n) => !prev.some((x) => x.game_id === n.game_id));
          if (started) say(`Timing ${started.title}`);
          return next;
        });
      }),
    ];
    api.nowPlaying().then(setPlaying);
    return () => {
      clearTimeout(pending);
      offs.forEach((o) => void o.then((f) => f()));
    };
  }, []);

  const libraryLike = route === "library" || route === "hidden";
  const leaveHidden = () => (setRoute("library"), setFocused(null));
  // Pages that show the selected game's art behind them.
  const showsArt = route === "library" || route === "collections";

  if (prefs.consoleMode) {
    return (
      <>
        <Console
          version={version}
          playing={playing}
          onPrimary={primary}
          onToggleHidden={toggleHidden}
          onToggleFavorite={toggleFavorite}
          onExit={() => setPrefs({ consoleMode: false })}
          onOpenDesktopSettings={() => {
            setRoute("settings");
            setPrefs({ consoleMode: false });
          }}
        />
        <UninstallHost say={say} />
        <VersionsHost say={say} />
        <Toast toast={toast} onDone={clearToast} />
      </>
    );
  }

  return (
    <>
      <Backdrop src={showsArt ? (focused?.hero ?? null) : null} />
      <TitleBar />
      <div className="app">
        <Rail route={route} counts={counts} onConsole={() => setPrefs({ consoleMode: true })} onNavigate={(r) => (setRoute(r), setFocused(null), setPeek(false), setRevealId(null), setStartShelf(undefined), setSettingsAt(undefined))} />
        <main className={`main${peek && route === "settings" ? " peeking" : ""}`}>
          {libraryLike && prefs.libraryView === "list" && (
            <ListView
              key={route}
              hiddenView={route === "hidden"}
              onLeaveHidden={leaveHidden}
              revealId={route === "library" ? revealId : null}
              version={version}
              focused={focused}
              playing={playing}
              onFocusGame={setFocused}
              onPrimary={primary}
              onToggleHidden={toggleHidden}
              onToggleFavorite={toggleFavorite}
            />
          )}
          {libraryLike && prefs.libraryView !== "list" && (
            <Library
              key={route}
              hiddenView={route === "hidden"}
              onLeaveHidden={leaveHidden}
              revealId={route === "library" ? revealId : null}
              hiddenCount={counts.hidden ?? 0}
              onShowHidden={() => (setRoute("hidden"), setFocused(null))}
              version={version}
              focused={focused}
              playing={playing}
              onFocusGame={setFocused}
              onPrimary={primary}
              onToggleHidden={toggleHidden}
              onToggleFavorite={toggleFavorite}
            />
          )}
          {route === "collections" && (
            <Collections
              key={startShelf ? shelfKey(startShelf) : "collections"}
              initialShelf={startShelf}
              version={version}
              focused={focused}
              playing={playing}
              onFocusGame={setFocused}
              onPrimary={primary}
              onToggleHidden={toggleHidden}
              onToggleFavorite={toggleFavorite}
            />
          )}
          {route === "stats" && <Stats version={version} onOpenGame={openGame} onOpenStatus={openStatus} onOpenStorage={() => setRoute("storage")} />}
          {route === "storage" && <Storage version={version} onOpenGame={openGame} say={say} />}
          {route === "settings" && (
            <div className="peek-library" aria-hidden inert>
              <Library
                key="peek"
                hiddenView={false}
                version={version}
                focused={focused}
                playing={playing}
                onFocusGame={setFocused}
                onPrimary={primary}
                onToggleHidden={toggleHidden}
                onToggleFavorite={toggleFavorite}
              />
            </div>
          )}
          {route === "settings" && (
            <Settings key={settingsAt ?? "settings"} startAt={settingsAt} onPeek={setPeek} hiddenCount={counts.hidden ?? 0} onShowHidden={() => (setRoute("hidden"), setFocused(null), setPeek(false))} />
          )}
        </main>
      </div>
      <UninstallHost say={say} />
      <VersionsHost say={say} />
      <Toast toast={toast} onDone={clearToast} />
    </>
  );
}

/** Cross-fades between the focused game's hero art. */
function Backdrop({ src }: { src: string | null }) {
  const [layers, setLayers] = useState<{ src: string; shown: boolean }[]>([]);
  useEffect(() => {
    if (!src) return setLayers([]);
    setLayers((cur) => [...cur.slice(-1), { src, shown: false }]);
  }, [src]);
  return (
    <div className="backdrop" aria-hidden>
      {layers.map((l, i) => (
        <img
          key={l.src + i}
          src={l.src}
          alt=""
          style={l.shown ? undefined : { opacity: 0 }}
          onLoad={() => setLayers((cur) => cur.map((x) => (x === l ? { ...x, shown: true } : x)))}
          onError={(e) => {
            const img = e.currentTarget;
            const alt = legacy(l.src);
            if (img.src !== alt) img.src = alt;
          }}
        />
      ))}
    </div>
  );
}
