import { useCallback, useEffect, useState } from "react";
import { api } from "./lib/api";
import type { Game, NowPlaying } from "./lib/types";
import { STORE_LABEL } from "./lib/types";
import { Rail, type Route } from "./components/Rail";
import { TitleBar } from "./components/TitleBar";
import { Toast, type ToastMsg } from "./components/Toast";
import { Library } from "./screens/Library";
import { Settings } from "./screens/Settings";
import { Soon } from "./screens/Soon";
import { legacy } from "./components/Art";

export function App() {
  const [route, setRoute] = useState<Route>("library");
  const [focused, setFocused] = useState<Game | null>(null);
  const [version, setVersion] = useState(0);
  const [toast, setToast] = useState<ToastMsg | null>(null);
  const [playing, setPlaying] = useState<NowPlaying[]>([]);
  const [counts, setCounts] = useState<Partial<Record<Route, number>>>({});

  const refresh = () => setVersion((v) => v + 1);
  const say = (text: string, action?: ToastMsg["action"]) => setToast({ id: Date.now(), text, action });
  const clearToast = useCallback(() => setToast(null), []);

  useEffect(() => {
    const q = { install: "all" as const, stores: [], search: null };
    Promise.all([
      api.listGames({ ...q, hidden_only: false }),
      api.listGames({ ...q, hidden_only: true }),
    ]).then(([lib, hid]) => setCounts({ library: lib.length, hidden: hid.length }));
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
  const primary = async (g: Game) => {
    const store = STORE_LABEL[g.entries[0]?.store ?? "manual"];
    if (api.demo) {
      say(`Demo library: ${g.installed ? "launching" : "installing"} works in the desktop app`);
      return;
    }
    try {
      await api.launch(g.id);
      say(g.installed ? `Starting ${g.title}` : `Opening ${store} to install ${g.title}`);
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

  return (
    <>
      <Backdrop src={libraryLike ? (focused?.hero ?? null) : null} />
      <TitleBar />
      <div className="app">
        <Rail route={route} counts={counts} onNavigate={(r) => (setRoute(r), setFocused(null))} />
        <main className="main">
          {libraryLike && (
            <Library
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
          {route === "collections" && (
            <Soon title="Collections" text="Group games your way, like Backlog or Co-op night, plus smart collections that fill themselves. Coming soon." />
          )}
          {route === "stats" && (
            <Soon title="Stats" text="Your playtime by week and month, top games, and a yearly recap. Coming soon." />
          )}
          {route === "settings" && <Settings />}
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
