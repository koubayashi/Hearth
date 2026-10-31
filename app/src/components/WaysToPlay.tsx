import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import type { Game, LaunchOption } from "../lib/types";
import { folderProgram, storeName } from "../lib/ways";
import { Icon } from "./Icon";
import { StoreIcon } from "./StoreIcon";

const PROGRAMS = [
  { name: "Programs, scripts and shortcuts", extensions: ["exe", "bat", "cmd", "lnk"] },
  { name: "All files", extensions: ["*"] },
];

type Draft = { id: number | null; name: string; path: string; args: string; working_dir: string };
const blank: Draft = { id: null, name: "", path: "", args: "", working_dir: "" };

/** "Start Server.cmd" -> "Start Server": a starting name for a picked program. */
const nameFrom = (path: string) => path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "";

/**
 * The ways a game can be played: its store's launch plus programs the user
 * added (a server replacement's script, a mod loader, another exe), and which
 * one the Play button uses.
 */
export function WaysToPlay({ game, onClose }: { game: Game; onClose: () => void }) {
  const [ways, setWays] = useState<LaunchOption[]>(game.launch_options);
  const [draft, setDraft] = useState<Draft | null>(game.launch_options.length === 0 ? blank : null);
  const [problem, setProblem] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const store = storeName(game);
  const storeId = (game.entries.find((e) => e.installed && !e.offline) ?? game.entries[0])?.store ?? "manual";
  const storeIsDefault = !ways.some((w) => w.is_default);
  const [program, setProgram] = useState(folderProgram(game));

  // The library refreshes after each change; keep this list in step with it.
  useEffect(() => {
    setWays(game.launch_options);
    setProgram(folderProgram(game));
  }, [game]);
  const reload = () =>
    api.getGame(game.id).then((g) => {
      if (!g) return;
      setWays(g.launch_options);
      setProgram(folderProgram(g));
    });

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>("input, button")?.focus();
  }, [draft === null]);

  const act = async (f: () => Promise<unknown>) => {
    setProblem(null);
    try {
      await f();
      await reload();
    } catch (e) {
      setProblem(String(e));
    }
  };
  const browse = async () => {
    if (!isTauri() || !draft) return;
    const picked = await openDialog({ title: "Choose a program, script or shortcut", filters: PROGRAMS });
    if (typeof picked !== "string") return;
    setDraft((d) => d && { ...d, path: picked, name: d.name || nameFrom(picked) });
  };
  /** A folder game: start another program from its folder (single player
   * instead of multiplayer, say). */
  const changeProgram = async () => {
    if (!isTauri() || !program) return;
    const picked = await openDialog({ title: "Choose the program Play starts", defaultPath: program.path, filters: PROGRAMS });
    if (typeof picked === "string") void act(() => api.setFolderExe(game.id, picked));
  };
  const browseFolder = async () => {
    if (!isTauri() || !draft) return;
    const picked = await openDialog({ title: "Choose the folder it starts in", directory: true });
    if (typeof picked === "string") setDraft((d) => d && { ...d, working_dir: picked });
  };
  const save = () =>
    draft &&
    act(async () => {
      await api.saveLaunchOption({ ...draft, game_id: game.id, working_dir: draft.working_dir || null });
      setDraft(null);
    });

  return (
    <div className="art-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={ref}
        className="ways-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Ways to play ${game.title}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            if (draft && ways.length > 0) setDraft(null);
            else onClose();
          }
        }}
      >
        <div className="match-head">
          <span>Ways to play {game.title}</span>
          <button className="icon-btn small" data-nav aria-label="Close" onClick={onClose}>
            <Icon name="close" size={16} />
          </button>
        </div>
        <p className="note ways-intro">
          Start {game.title} another way: with mods through a script extender or mod manager, straight from its exe
          without the store's launcher, in DX11 or offline mode, or through any script or tool. Pick which one the Play
          button uses.
        </p>

        <ul className="ways-list">
          <li className="way">
            <span className="way-icon">
              <StoreIcon store={storeId} size={18} />
            </span>
            {program ? (
              <span className="way-text">
                <b>Game folder</b>
                <span title={program.path}>
                  Starts {program.name}
                  {!program.picked && " (picked automatically)"}
                </span>
              </span>
            ) : (
              <span className="way-text">
                <b>{store}</b>
                <span>{game.installed ? `Launches through ${store}` : `Installs through ${store}`}</span>
              </span>
            )}
            {storeIsDefault ? (
              <span className="way-default">Play button</span>
            ) : (
              <button className="btn" data-nav onClick={() => act(() => api.setDefaultLaunch(game.id, null))}>
                Use for Play
              </button>
            )}
            {program && (
              <button className="icon-btn small" data-nav aria-label={`Change the program ${game.title} starts`} title="Change program" onClick={changeProgram}>
                <Icon name="sliders" size={15} />
              </button>
            )}
            {program?.picked && (
              <button
                className="icon-btn small"
                data-nav
                aria-label="Go back to the program Hearth picked"
                title="Go back to the program Hearth picked"
                onClick={() => act(() => api.setFolderExe(game.id, null))}
              >
                <Icon name="close" size={15} />
              </button>
            )}
          </li>
          {ways.map((w) => (
            <li key={w.id} className="way">
              <span className="way-icon">
                <Icon name="play" size={14} />
              </span>
              <span className="way-text">
                <b>{w.name}</b>
                <span title={w.path}>
                  {w.path}
                  {w.args && ` ${w.args}`}
                </span>
              </span>
              {w.is_default ? (
                <span className="way-default">Play button</span>
              ) : (
                <button className="btn" data-nav onClick={() => act(() => api.setDefaultLaunch(game.id, w.id))}>
                  Use for Play
                </button>
              )}
              <button
                className="icon-btn small"
                data-nav
                aria-label={`Edit ${w.name}`}
                title="Edit"
                onClick={() => setDraft({ id: w.id, name: w.name, path: w.path, args: w.args, working_dir: w.working_dir ?? "" })}
              >
                <Icon name="sliders" size={15} />
              </button>
              <button
                className="icon-btn small"
                data-nav
                aria-label={`Remove ${w.name}`}
                title="Remove"
                onClick={() => act(() => api.deleteLaunchOption(w.id))}
              >
                <Icon name="close" size={15} />
              </button>
            </li>
          ))}
        </ul>

        {draft ? (
          <form
            className="way-form"
            onSubmit={(e) => {
              e.preventDefault();
              void save();
            }}
          >
            <h3>{draft.id === null ? "Add a way to play" : `Edit ${draft.name || "way to play"}`}</h3>
            <label className="field">
              <span>Program, script or shortcut</span>
              <span className="field-row">
                <input
                  className="text-input"
                  data-nav
                  value={draft.path}
                  placeholder="C:\Games\Skyrim Special Edition\skse64_loader.exe"
                  onChange={(e) => setDraft({ ...draft, path: e.target.value })}
                />
                {isTauri() && (
                  <button type="button" className="btn" data-nav onClick={browse}>
                    Browse
                  </button>
                )}
              </span>
              <small className="field-hint">A mod loader, a mod manager, the game's own exe, a .bat script or a .lnk shortcut.</small>
            </label>
            <label className="field">
              <span>Name</span>
              <input
                className="text-input"
                data-nav
                value={draft.name}
                placeholder="With mods"
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
              <small className="field-hint">Shown on the Play button, as in "Play · With mods".</small>
            </label>
            <label className="field">
              <span>Arguments (optional)</span>
              <input
                className="text-input"
                data-nav
                value={draft.args}
                placeholder="-skip_launcher"
                onChange={(e) => setDraft({ ...draft, args: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Start in (optional, the program's own folder if empty)</span>
              <span className="field-row">
                <input
                  className="text-input"
                  data-nav
                  value={draft.working_dir}
                  onChange={(e) => setDraft({ ...draft, working_dir: e.target.value })}
                />
                {isTauri() && (
                  <button type="button" className="btn" data-nav onClick={browseFolder}>
                    Browse
                  </button>
                )}
              </span>
            </label>
            <p className="note">
              Playtime counts as usual while the game runs from its install folder, however it was started.
            </p>
            {problem && (
              <p className="note error" role="alert">
                {problem}
              </p>
            )}
            <div className="way-actions">
              {ways.length > 0 && (
                <button type="button" className="btn" data-nav onClick={() => (setDraft(null), setProblem(null))}>
                  Cancel
                </button>
              )}
              <button type="submit" className="btn btn-primary" data-nav>
                {draft.id === null ? "Add" : "Save"}
              </button>
            </div>
          </form>
        ) : (
          <>
            {problem && (
              <p className="note error" role="alert">
                {problem}
              </p>
            )}
            <button className="btn way-add" data-nav onClick={() => setDraft(blank)}>
              <Icon name="play" size={13} />
              Add a way to play
            </button>
          </>
        )}
      </div>
    </div>
  );
}
