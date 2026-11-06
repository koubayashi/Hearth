import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { savedInfo } from "../lib/gameInfo";
import { formatBytes } from "../lib/format";
import { driveRoot } from "../lib/storage";
import { versionEntry, versionsOf } from "../lib/versions";
import type { Game } from "../lib/types";
import { Icon } from "./Icon";

// Any menu or page can ask to uninstall a game; App shows the one dialog.
let opener: ((g: Game) => void) | null = null;
export const openUninstall = (g: Game) => opener?.(g);

type Plan = { store: string; path: string };
type Outcome = { kind: "started"; by: string } | { kind: "elsewhere"; app: string; steps: string } | { kind: "recycled" };

/** What each route does, said before anything happens. */
function explain(store: string, title: string, size: string) {
  switch (store) {
    case "steam":
      return { text: `Steam asks you to confirm, then removes ${title}${size}.`, go: "Uninstall in Steam", danger: false };
    case "gog":
      return { text: `${title}'s own uninstaller runs and removes it${size}.`, go: "Uninstall", danger: false };
    case "epic":
      return { text: `Epic doesn't let other apps uninstall its games. Hearth opens the Epic Games Launcher and shows you where to do it.`, go: "Open Epic Games Launcher", danger: false };
    case "xbox":
      return { text: `Xbox and Game Pass games are removed from Windows' Installed apps. Hearth opens it and shows you where.`, go: "Open Installed apps", danger: false };
    default:
      return { text: `The game's folder goes to the Recycle Bin${size}. You can restore it from there if you change your mind.`, go: "Move to Recycle Bin", danger: true };
  }
}

/** Mounted once by App. Confirms, then uninstalls through whoever installed the game. */
export function UninstallHost({ say }: { say: (text: string) => void }) {
  const [game, setGame] = useState<Game | null>(null);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [next, setNext] = useState<Extract<Outcome, { kind: "elsewhere" }> | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const goRef = useRef<HTMLButtonElement>(null);
  // With two versions installed (GTA V and Enhanced), which one goes.
  const [entry, setEntry] = useState<number | undefined>(undefined);

  const ask = (g: Game, e: number | undefined) => {
    setEntry(e);
    setPlan(null);
    setProblem(null);
    api.uninstallPlan(g.id, e).then(setPlan, (err) => setProblem(String(err)));
  };
  useEffect(() => {
    opener = (g) => {
      setGame(g);
      setNext(null);
      const installed = versionsOf(g).filter((v) => v.installed);
      ask(g, installed.length > 1 ? versionEntry(installed[0]).id : undefined);
    };
    return () => {
      opener = null;
    };
  }, []);
  useEffect(() => {
    if (plan) goRef.current?.focus();
  }, [plan]);

  if (!game) return null;
  const close = () => setGame(null);
  const installedVersions = versionsOf(game).filter((v) => v.installed);
  // The saved size is one folder's: only say it when there's one install.
  const bytes = installedVersions.length > 1 ? null : (savedInfo(game.id)?.install?.bytes ?? null);
  const drive = plan ? driveRoot(plan.path) : null;
  const size = bytes != null ? `, freeing ${formatBytes(bytes)}${drive ? ` on ${drive}` : ""}` : "";
  const how = plan ? explain(plan.store, game.title, size) : null;

  const go = async () => {
    setBusy(true);
    setProblem(null);
    try {
      const out = (await api.uninstallGame(game.id, entry)) as Outcome;
      if (out.kind === "elsewhere") return setNext(out);
      say(out.kind === "recycled" ? `${game.title} is in the Recycle Bin` : `${out.by} is uninstalling ${game.title}`);
      close();
    } catch (e) {
      setProblem(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="art-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div
        className="ways-dialog uninstall-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Uninstall ${game.title}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
          }
        }}
      >
        <div className="match-head">
          <span>{next ? `Finish in ${next.app}` : `Uninstall ${game.title}?`}</span>
          <button className="icon-btn small" data-nav aria-label="Close" onClick={close}>
            <Icon name="close" size={16} />
          </button>
        </div>

        {next ? (
          <p className="uninstall-text">{next.steps} Hearth notices when it's gone.</p>
        ) : (
          <>
            {installedVersions.length > 1 && (
              <div className="seg uninstall-versions" role="radiogroup" aria-label="Which version">
                {installedVersions.map((v) => {
                  const id = versionEntry(v).id;
                  return (
                    <button key={id} role="radio" aria-checked={entry === id} data-nav onClick={() => ask(game, id)}>
                      {v.label}
                    </button>
                  );
                })}
              </div>
            )}
            {how && <p className="uninstall-text">{how.text}</p>}
            {plan && <p className="uninstall-path" title={plan.path}>{plan.path}</p>}
          </>
        )}

        {problem && (
          <p className="note error" role="alert">
            {problem}
          </p>
        )}
        <div className="way-actions">
          {next ? (
            <button className="btn btn-primary" data-nav onClick={close} ref={goRef}>
              Done
            </button>
          ) : (
            <>
              <button className="btn" data-nav onClick={close}>
                Cancel
              </button>
              {how && (
                <button ref={goRef} className={`btn ${how.danger ? "btn-danger" : "btn-primary"}`} data-nav disabled={busy} onClick={() => void go()}>
                  {how.go}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
