import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Collection, Game, GameStatus } from "../lib/types";
import { STATUSES, STORE_LABEL } from "../lib/types";
import { Icon, type IconName } from "./Icon";

export type PanelAction = "artwork" | "match";

/** Right-click (or Menu key / controller Y) menu for a game. */
export function GameMenu({
  game,
  x,
  y,
  onClose,
  onPrimary,
  onToggleFavorite,
  onToggleHidden,
  onPanel,
}: {
  game: Game;
  x: number;
  y: number;
  onClose: () => void;
  onPrimary: (g: Game) => void;
  onToggleFavorite: (g: Game) => void;
  onToggleHidden: (g: Game) => void;
  /** Opens a picker in the game panel. */
  onPanel: (action: PanelAction) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const [view, setView] = useState<"main" | "collections">("main");

  // Keep the menu on screen near the pointer.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - r.height - 8)),
    });
    el.querySelector<HTMLElement>("button, input")?.focus();
  }, [x, y, view]);

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

  const store = STORE_LABEL[game.entries[0]?.store ?? "manual"];
  const run = (f: () => void) => () => {
    onClose();
    f();
  };
  const item = (icon: IconName, label: string, f: () => void) => (
    <button role="menuitem" onClick={run(f)}>
      <Icon name={icon} />
      {label}
    </button>
  );
  const setStatus = (s: GameStatus | null) => api.setStatus(game.id, s);

  return (
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
      {view === "collections" ? (
        <CollectionsView game={game} onBack={() => setView("main")} />
      ) : (
        <>
          {item(game.installed ? "play" : "download", game.installed ? "Play" : `Install from ${store}`, () => onPrimary(game))}
          {item("star", game.favorite ? "Remove from favourites" : "Add to favourites", () => onToggleFavorite(game))}
          <hr />
          <div className="menu-label">Status</div>
          {STATUSES.map((s) => (
            <button
              key={s.id}
              role="menuitemradio"
              aria-checked={game.status === s.id}
              onClick={run(() => setStatus(game.status === s.id ? null : s.id))}
            >
              <span className={`status-dot ${s.id}`} />
              {s.label}
              {game.status === s.id && <Icon name="check" size={15} />}
            </button>
          ))}
          <button role="menuitem" onClick={() => setView("collections")}>
            <Icon name="collections" />
            Add to collection…
          </button>
          <hr />
          {item("library", "Change artwork…", () => onPanel("artwork"))}
          {item("search", "Find on Steam store…", () => onPanel("match"))}
          {item("external", "Open store page", () => api.openStorePage(game.id))}
          {item("external", "Open on Backloggd", () => api.openBackloggd(game.id))}
          {game.installed && item("folder", "Open install folder", () => api.openInstallFolder(game.id))}
          <hr />
          {item(game.hidden ? "eye" : "hidden", game.hidden ? "Show in library" : "Hide from library", () => onToggleHidden(game))}
        </>
      )}
    </div>
  );
}

/** Tick the collections a game belongs to, or start a new one. */
function CollectionsView({ game, onBack }: { game: Game; onBack: () => void }) {
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
      <button role="menuitem" onClick={onBack}>
        <Icon name="collections" />
        Back
      </button>
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
