import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../lib/api";
import type { Collection, Game, GameStatus, Via } from "../lib/types";
import { defaultWay, folderGone, folderProgram, storeName } from "../lib/ways";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { Icon, type IconName } from "./Icon";
import { openUninstall } from "./Uninstall";
import { openVersions } from "./Versions";
import { versionEntry, versionsOf } from "../lib/versions";

export type PanelAction = "artwork" | "match" | "ways" | "smart" | "history";

/**
 * The one menu for a game: right-click, Menu key or controller Y on a cover,
 * and the poster's "More options" button. Kept short: statuses are one
 * compact picker, and the store and folder links share a submenu.
 */
export function GameMenu({
  game,
  x,
  y,
  anchor,
  onClose,
  onPrimary,
  onToggleFavorite,
  onToggleHidden,
  onPanel,
  start = "main",
}: {
  /** "play" opens straight on the ways to play (the arrow beside Play). */
  start?: "main" | "play";
  game: Game;
  x: number;
  y: number;
  /** Open above this element, right-aligned to it, instead of at x/y. */
  anchor?: DOMRect;
  onClose: () => void;
  onPrimary: (g: Game, via?: Via) => void;
  onToggleFavorite: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  /** Opens a picker in the game panel. */
  onPanel: (action: PanelAction) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const [view, setView] = useState<"main" | "collections" | "open" | "play">(start);

  // Keep the menu on screen near the pointer.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const want = anchor ? { x: anchor.right - r.width, y: anchor.top - r.height - 6 } : { x, y };
    setPos({
      x: Math.max(8, Math.min(want.x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(want.y, window.innerHeight - r.height - 8)),
    });
    el.querySelector<HTMLElement>("button, input")?.focus();
  }, [x, y, anchor, view]);

  useEffect(() => {
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && onClose();
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", esc);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", esc);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const store = storeName(game);
  const run = (f: () => void) => () => {
    onClose();
    f();
  };
  const item = (icon: IconName, label: string, f: () => void, hint?: string) => (
    <button role="menuitem" onClick={run(f)} className={hint ? "has-hint" : undefined}>
      <Icon name={icon} />
      {hint ? (
        <span className="menu-two">
          {label}
          <small>{hint}</small>
        </span>
      ) : (
        label
      )}
    </button>
  );
  const setStatus = (s: GameStatus | null) => api.setStatus(game.id, s);
  const sub = (icon: IconName, label: string, to: "collections" | "open" | "play") => (
    <button role="menuitem" aria-haspopup="menu" onClick={() => setView(to)}>
      <Icon name={icon} />
      {label}
      <Icon name="chevronRight" size={15} className="sub-arrow" />
    </button>
  );
  const back = (
    <button role="menuitem" onClick={() => setView("main")}>
      <Icon name="chevronLeft" />
      Back
    </button>
  );

  // Rendered at the page root so no panel can clip or offset it.
  return createPortal(
    <div
      ref={ref}
      className="menu context-menu"
      role="menu"
      aria-label={`${game.title} options`}
      style={{ left: pos.x, top: pos.y }}
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => {
        // Arrow keys move through the menu, not the library behind it.
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          e.stopPropagation();
          const items = [...(ref.current?.querySelectorAll<HTMLElement>("button, input") ?? [])];
          const i = items.indexOf(document.activeElement as HTMLElement);
          const next = e.key === "ArrowDown" ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
          items[next]?.focus();
        }
      }}
    >
      {view === "play" ? (
        <PlayView game={game} back={start === "play" ? null : back} item={item} run={run} onPrimary={onPrimary} onPanel={onPanel} />
      ) : view === "collections" ? (
        <CollectionsView game={game} back={back} />
      ) : view === "open" ? (
        <>
          {back}
          <hr />
          {item("external", `Open ${store} page`, () => api.openStorePage(game.id))}
          {game.installed && item("folder", "Open install folder", () => api.openInstallFolder(game.id))}
          {item("search", "Find on Steam store…", () => onPanel("match"))}
        </>
      ) : (
        <>
          {game.offline_drive ? (
            <button role="menuitem" disabled>
              <Icon name="folder" />
              Drive {game.offline_drive} not connected
            </button>
          ) : folderGone(game) ? (
            <button role="menuitem" disabled>
              <Icon name="folder" />
              Not installed
            </button>
          ) : (
            item(
              defaultWay(game) || game.installed ? "play" : "download",
              defaultWay(game) ? `Play with ${defaultWay(game)!.name}` : game.installed ? "Play" : `Install from ${store}`,
              () => onPrimary(game),
            )
          )}
          {game.launch_options.length > 0 && sub("play", "Play another way", "play")}
          {item("star", game.favorite ? "Remove from favourites" : "Add to favourites", () => onToggleFavorite(game))}
          <hr />
          {/* Pressing the current status again clears it. */}
          <div className="status-pick" role="group" aria-label="Status">
            {STATUSES.map((s) => (
              <button
                key={s.id}
                role="menuitemradio"
                aria-checked={game.status === s.id}
                onClick={run(() => setStatus(game.status === s.id ? null : s.id))}
              >
                <span className={`status-dot ${s.id}`} />
                {s.label}
              </button>
            ))}
          </div>
          <hr />
          {sub("collections", "Add to collection", "collections")}
          {item("library", "Change artwork…", () => onPanel("artwork"))}
          {item("collections", "Series and studios…", () => onPanel("smart"))}
          {item("stats", "Play history…", () => onPanel("history"))}
          {item("collections", "Versions…", () => openVersions(game), versionsOf(game).length === 0 ? "Combine with a remaster or edition" : undefined)}
          {/* Until a game has one, say what they are for: mods are the usual reason. */}
          {item("sliders", "Ways to play…", () => onPanel("ways"), game.launch_options.length === 0 ? "Mods, tools, other programs" : undefined)}
          {sub("external", "Store and files", "open")}
          <hr />
          {item(game.hidden ? "eye" : "hidden", game.hidden ? "Show in library" : "Hide from library", () => onToggleHidden(game))}
          {game.installed && item("trash", "Uninstall…", () => openUninstall(game))}
        </>
      )}
    </div>,
    document.body,
  );
}

/** Every way to start this game; the one the Play button uses is ticked. */
function PlayView({
  game,
  back,
  item,
  run,
  onPrimary,
  onPanel,
}: {
  game: Game;
  back: React.ReactNode;
  /** Closes the menu, then does the thing. */
  run: (f: () => void) => () => void;
  item: (icon: IconName, label: string, f: () => void, hint?: string) => React.ReactNode;
  onPrimary: (g: Game, via?: Via) => void;
  onPanel: (action: PanelAction) => void;
}) {
  const fallback = defaultWay(game);
  const choice = (key: string, label: string, on: boolean, via: Via, disabled = false) => (
    <button key={key} role="menuitemradio" aria-checked={on} disabled={disabled} onClick={run(() => onPrimary(game, via))}>
      <span className="radio-dot" aria-hidden />
      {label}
    </button>
  );
  const store = storeName(game);
  const program = folderProgram(game);
  const versions = versionsOf(game);
  return (
    <>
      {back}
      {back && <hr />}
      {versions.length > 0 && (
        <>
          <div className="menu-label">Versions</div>
          {versions.map((v) => {
            const e = versionEntry(v);
            return item(
              v.installed ? "play" : "download",
              v.installed ? `Play ${v.label}` : `Install ${v.label} from ${STORE_LABEL[e.store]}`,
              () => onPrimary(game, { kind: "entry", id: e.id }),
            );
          })}
          {item("collections", "Versions…", () => openVersions(game))}
          {game.launch_options.length > 0 && <hr />}
        </>
      )}
      {(versions.length === 0 || game.launch_options.length > 0) && (
        <>
      <div className="menu-label">The Play button starts</div>
      {choice(
        "store",
        game.offline_drive
          ? `${store}: drive ${game.offline_drive} not connected`
          : program
            ? `Play ${program.name}`
            : game.installed
            ? `Play with ${store}`
            : storeName(game) === "Manual"
              ? "Game folder: not installed"
              : `Install from ${store}`,
        fallback === null,
        { kind: "store" },
        !!game.offline_drive || (!game.installed && storeName(game) === "Manual"),
      )}
      {game.launch_options.map((o) => choice(String(o.id), `Play with ${o.name}`, o.is_default, { kind: "option", id: o.id }))}
      <hr />
      {item("sliders", "Ways to play…", () => onPanel("ways"))}
        </>
      )}
    </>
  );
}

/** Tick the collections a game belongs to, or start a new one. */
function CollectionsView({ game, back }: { game: Game; back: React.ReactNode }) {
  const [list, setList] = useState<Collection[] | null>(null);
  const [member, setMember] = useState<number[]>(game.collections);
  const [name, setName] = useState("");

  useEffect(() => {
    api.listCollections().then(setList);
  }, []);

  const toggle = async (c: Collection) => {
    const on = !member.includes(c.id);
    await api.setInCollection(c.id, game.id, on);
    setMember((m) => (on ? [...m, c.id] : m.filter((x) => x !== c.id)));
  };
  const create = async () => {
    if (!name.trim()) return;
    const id = await api.createCollection(name);
    await api.setInCollection(id, game.id, true);
    setMember((m) => [...m, id]);
    setList(await api.listCollections());
    setName("");
  };

  return (
    <>
      {back}
      <hr />
      {list?.length === 0 && <div className="menu-label">No collections yet</div>}
      {list?.map((c) => (
        <button key={c.id} role="menuitemcheckbox" aria-checked={member.includes(c.id)} onClick={() => toggle(c)}>
          <span className="check-box">{member.includes(c.id) && <Icon name="check" size={13} />}</span>
          {c.name}
        </button>
      ))}
      <hr />
      <form
        className="menu-new"
        onSubmit={(e) => {
          e.preventDefault();
          create();
        }}
      >
        <input
          className="text-input"
          placeholder="New collection…"
          aria-label="New collection name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.stopPropagation()}
        />
      </form>
    </>
  );
}
