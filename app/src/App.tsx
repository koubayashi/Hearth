import { useCallback, useEffect, useState } from "react";
import { api } from "./lib/api";
import type { Game, GameStatus, NowPlaying, Via } from "./lib/types";
import { chosenWay } from "./lib/ways";
import { STORE_LABEL } from "./lib/types";
import { Rail, type Route } from "./components/Rail";
import { TitleBar } from "./components/TitleBar";
import { Toast, type ToastMsg } from "./components/Toast";
import { Library } from "./screens/Library";
import { Collections } from "./screens/Collections";
import { Settings } from "./screens/Settings";
import { Stats } from "./screens/Stats";
import { legacy } from "./components/Art";
import { setPrefs, usePrefs } from "./lib/prefs";
import { ListView } from "./screens/ListView";
import { Console } from "./screens/Console";

export function App() {
  const [route, setRoute] = useState<Route>("library");
  // Automatic backups run in the background, window open or not: give them
  // the current appearance settings to include.
  const prefs = usePrefs();
  useEffect(() => {
    api.savePrefsSnapshot(JSON.stringify(prefs)).catch(() => {});
  }, [prefs]);
  const [focused, setFocused] = useState<Game | null>(null);
  const [version, setVersion] = useState(0);
  const [toast, setToast] = useState<ToastMsg | null>(null);
  const [playing, setPlaying] = useState<NowPlaying[]>([]);
  // While the cover size slider moves, the library shows behind Settings.
  // It stays mounted (hidden) while Settings is open, so it is ready at once.
  const [peek, setPeek] = useState(false);
  // Where Stats sent you: a game to show in the library, or a status shelf.
  const [revealId, setRevealId] = useState<number | null>(null);
  const [startStatus, setStartStatus] = useState<GameStatus | undefined>(undefined);
  const openGame = (g: Game) => {
    setFocused(g);
    setRevealId(g.id);
    setRoute("library");
  };
  const openStatus = (status: GameStatus) => {
    setStartStatus(status);
    setFocused(null);
    setRoute("collections");
  };
  // A controller's Start button opens the console from anywhere.
  useEffect(() => {
    if (prefs.consoleMode) return;
    const onKey = (e: KeyboardEvent) => e.key === "GamepadStart" && setPrefs({ consoleMode: true });
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [prefs.consoleMode]);
  const [counts, setCounts] = useState<Partial<Record<Route, number>>>({});

  const refresh = () => setVersion((v) => v + 1);
  const say = (text: string, action?: ToastMsg["action"]) => setToast({ id: Date.now(), text, action });
  const clearToast = useCallback(() => setToast(null), []);

  useEffect(() => {
    api.gameCounts().then(setCounts);
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
    const store = STORE_LABEL[g.entries[0]?.store ?? "manual"];
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
    const offs = [
      api.onLibraryChanged(refresh),
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
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, []);

  const libraryLike = route === "library" || route === "hidden";
  // Pages that show the selected game's art behind them.
  const showsArt = libraryLike || route === "collections";

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
        <Toast toast={toast} onDone={clearToast} />
      </>
    );
  }

  return (
    <>
      <Backdrop src={showsArt ? (focused?.hero ?? null) : null} />
      <TitleBar />
      <div className="app">
        <Rail route={route} counts={counts} onConsole={() => setPrefs({ consoleMode: true })} onNavigate={(r) => (setRoute(r), setFocused(null), setPeek(false), setRevealId(null), setStartStatus(undefined))} />
        <main className={`main${peek && route === "settings" ? " peeking" : ""}`}>
          {libraryLike && prefs.libraryView === "list" && (
            <ListView
              key={route}
              hiddenView={route === "hidden"}
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
          {route === "collections" && (
            <Collections
              key={startStatus ?? "collections"}
              initialStatus={startStatus}
              version={version}
              focused={focused}
              playing={playing}
              onFocusGame={setFocused}
              onPrimary={primary}
              onToggleHidden={toggleHidden}
              onToggleFavorite={toggleFavorite}
            />
          )}
          {route === "stats" && <Stats version={version} onOpenGame={openGame} onOpenStatus={openStatus} />}
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
          {route === "settings" && <Settings onPeek={setPeek} />}
        </main>
      </div>
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
