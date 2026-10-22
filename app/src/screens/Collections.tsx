import { useEffect, useState, type ComponentProps } from "react";
import { api } from "../lib/api";
import type { Collection, Game, GameStatus } from "../lib/types";
import { STATUSES } from "../lib/types";
import { Icon } from "../components/Icon";
import { Library } from "./Library";

type Shelf = { kind: "status"; status: GameStatus } | { kind: "collection"; id: number };
type LibraryProps = Omit<ComponentProps<typeof Library>, "hiddenView" | "scope">;

const EMPTY: Record<GameStatus, string> = {
  playing: "Right-click a game and choose Playing to keep what you're in the middle of here.",
  backlog: "Right-click a game and choose Backlog for the ones you mean to get to.",
  completed: "Right-click a game and choose Completed when you finish it.",
  abandoned: "Right-click a game and choose Abandoned for the ones you've put down for good.",
};

/** Statuses and the user's own collections, each a shelf of the library. */
export function Collections(props: LibraryProps) {
  const [shelf, setShelf] = useState<Shelf>({ kind: "status", status: "playing" });
  const [collections, setCollections] = useState<Collection[]>([]);
  const [counts, setCounts] = useState<Record<GameStatus, number>>({ playing: 0, backlog: 0, completed: 0, abandoned: 0 });
  const [naming, setNaming] = useState<{ id: number | null; name: string } | null>(null);

  useEffect(() => {
    api.listCollections().then(setCollections);
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then((games: Game[]) => {
      const c = { playing: 0, backlog: 0, completed: 0, abandoned: 0 };
      games.forEach((g) => g.status && c[g.status]++);
      setCounts(c);
    });
  }, [props.version]);

  // A deleted collection falls back to the first shelf.
  useEffect(() => {
    if (shelf.kind === "collection" && collections.length > 0 && !collections.some((c) => c.id === shelf.id)) {
      setShelf({ kind: "status", status: "playing" });
    }
  }, [collections, shelf]);

  const save = async () => {
    if (!naming?.name.trim()) return setNaming(null);
    if (naming.id === null) {
      const id = await api.createCollection(naming.name);
      setShelf({ kind: "collection", id });
    } else {
      await api.renameCollection(naming.id, naming.name);
    }
    setNaming(null);
    setCollections(await api.listCollections());
  };

  const current = shelf.kind === "collection" ? collections.find((c) => c.id === shelf.id) : null;
  const statusLabel = shelf.kind === "status" ? STATUSES.find((s) => s.id === shelf.status)!.label : "";

  const header = (
    <div className="shelves">
      <div className="shelf-row" role="tablist" aria-label="Statuses">
        {STATUSES.map((s) => (
          <button
            key={s.id}
            role="tab"
            data-nav
            className="shelf"
            aria-selected={shelf.kind === "status" && shelf.status === s.id}
            onClick={() => setShelf({ kind: "status", status: s.id })}
          >
            <span className={`status-dot ${s.id}`} />
            {s.label}
            <small>{counts[s.id]}</small>
          </button>
        ))}
      </div>
      <div className="shelf-row" role="tablist" aria-label="Your collections">
        {collections.map((c) => (
          <button
            key={c.id}
            role="tab"
            data-nav
            className="shelf"
            aria-selected={shelf.kind === "collection" && shelf.id === c.id}
            onClick={() => setShelf({ kind: "collection", id: c.id })}
          >
            <Icon name="collections" size={15} />
            {c.name}
            <small>{c.count}</small>
          </button>
        ))}
        {naming ? (
          <form
            className="shelf-form"
            onSubmit={(e) => {
              e.preventDefault();
              save();
            }}
          >
            <input
              className="text-input"
              autoFocus
              data-nav
              placeholder="Collection name"
              aria-label="Collection name"
              value={naming.name}
              onChange={(e) => setNaming({ ...naming, name: e.target.value })}
              onKeyDown={(e) => e.key === "Escape" && setNaming(null)}
              onBlur={save}
            />
          </form>
        ) : (
          <button className="shelf add" data-nav onClick={() => setNaming({ id: null, name: "" })}>
            + New collection
          </button>
        )}
        {current && !naming && (
          <span className="shelf-actions">
            <button className="link" data-nav onClick={() => setNaming({ id: current.id, name: current.name })}>
              Rename
            </button>
            <button
              className="link"
              data-nav
              onClick={async () => {
                await api.deleteCollection(current.id);
                setCollections(await api.listCollections());
              }}
            >
              Delete collection
            </button>
          </span>
        )}
      </div>
    </div>
  );

  return (
    <Library
      key={shelf.kind === "status" ? `s-${shelf.status}` : `c-${shelf.id}`}
      {...props}
      hiddenView={false}
      scope={
        shelf.kind === "status"
          ? { status: shelf.status, title: statusLabel, header, emptyText: EMPTY[shelf.status] }
          : {
              collection: shelf.id,
              title: current?.name ?? "Collection",
              header,
              emptyText: "Right-click any game and choose Add to collection… to put it here.",
            }
      }
    />
  );
}
