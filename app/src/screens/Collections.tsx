import { useEffect, useMemo, useState, type ComponentProps } from "react";
import { api } from "../lib/api";
import type { Collection, Game, GameStatus, SmartRole } from "../lib/types";
import { STATUSES } from "../lib/types";
import { shelfKey, type Shelf } from "../lib/nav";
import { buildSmart, loadSmart, ROLE_LABEL, useSmartIndex, useSmartLooking } from "../lib/smart";
import { Icon } from "../components/Icon";
import { SmartBrowser } from "../components/SmartBrowser";
import { Library } from "./Library";

type LibraryProps = Omit<ComponentProps<typeof Library>, "hiddenView" | "scope">;

const ROLES: SmartRole[] = ["series", "developer", "publisher"];

const EMPTY: Record<GameStatus, string> = {
  playing: "Right-click a game and choose Playing to keep what you're in the middle of here.",
  backlog: "Right-click a game and choose Backlog for the ones you mean to get to.",
  completed: "Right-click a game and choose Completed when you finish it.",
  abandoned: "Right-click a game and choose Abandoned for the ones you've put down for good.",
};

/** Statuses, the user's own collections, and series and studios, each a shelf of the library. */
export function Collections({ initialShelf, ...props }: LibraryProps & { initialShelf?: Shelf }) {
  const [shelf, setShelf] = useState<Shelf>(initialShelf ?? { kind: "status", status: "playing" });
  const [collections, setCollections] = useState<Collection[]>([]);
  const [counts, setCounts] = useState<Record<GameStatus, number>>({ playing: 0, backlog: 0, completed: 0, abandoned: 0 });
  const [favoriteCount, setFavoriteCount] = useState(0);
  const [naming, setNaming] = useState<{ id: number | null; name: string } | null>(null);
  const [allGames, setAllGames] = useState<Game[]>([]);
  const index = useSmartIndex();
  const looking = useSmartLooking();
  const smart = useMemo(() => buildSmart(index, new Set(allGames.map((g) => g.id))), [index, allGames]);

  useEffect(() => {
    api.listCollections().then(setCollections);
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then((games: Game[]) => {
      const c = { playing: 0, backlog: 0, completed: 0, abandoned: 0 };
      games.forEach((g) => g.status && c[g.status]++);
      setFavoriteCount(games.filter((g) => g.favorite).length);
      setCounts(c);
      setAllGames(games);
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
      <div className="shelf-row" role="tablist" aria-label="Progress">
        <span className="shelf-label" aria-hidden>Progress</span>
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
        <span className="shelf-label" aria-hidden>Collections</span>
        <button
          role="tab"
          data-nav
          className="shelf"
          aria-selected={shelf.kind === "favorites"}
          onClick={() => setShelf({ kind: "favorites" })}
        >
          <Icon name="star" size={15} />
          Favourites
          <small>{favoriteCount}</small>
        </button>
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
      <div className="shelf-row" role="tablist" aria-label="Browse by">
        <span className="shelf-label" aria-hidden>Browse</span>
        {ROLES.map((r) => (
          <button
            key={r}
            role="tab"
            data-nav
            className="shelf"
            aria-selected={(shelf.kind === "browse" || shelf.kind === "entity") && shelf.role === r}
            onClick={() => setShelf({ kind: "browse", role: r })}
          >
            {ROLE_LABEL[r].many}
            {/* While Wikidata is being asked, an empty shelf says so instead of "0". */}
            <small className={looking && smart.tree(r).length === 0 ? "pending" : undefined}>
              {looking && smart.tree(r).length === 0 ? "…" : smart.tree(r).length}
            </small>
          </button>
        ))}
      </div>
      {shelf.kind === "entity" && <EntityTrail shelf={shelf} smart={smart} onShelf={setShelf} />}
    </div>
  );

  const scope = () => {
    switch (shelf.kind) {
      case "status":
        return { status: shelf.status, title: statusLabel, header, emptyText: EMPTY[shelf.status] };
      case "favorites":
        return { favorites: true, title: "Favourites", header, emptyText: "Star a game from its panel or right-click menu to keep it here." };
      case "collection":
        return {
          collection: shelf.id,
          title: current?.name ?? "Collection",
          header,
          emptyText: "Right-click any game and choose Add to collection… to put it here.",
        };
      case "browse":
        return {
          title: ROLE_LABEL[shelf.role].many,
          header,
          emptyText: "",
          browse: (search: string) => (
            <SmartBrowser
              role={shelf.role}
              smart={smart}
              games={allGames}
              search={search}
              looking={looking}
              onOpen={(id) => setShelf({ kind: "entity", role: shelf.role, id })}
            />
          ),
        };
      case "entity":
        return {
          ids: smart.games(shelf.role, shelf.id),
          title: smart.label(shelf.id),
          header,
          emptyText: "No games here any more. Fix a game's series and studios from its menu.",
        };
    }
  };

  return <Library key={shelfKey(shelf)} {...props} hiddenView={false} scope={scope()} />;
}

/**
 * Where a series or company shelf sits: back to all of them, what it's part
 * of (Ubisoft for Ubisoft Montreal), and what's part of it, each a shelf.
 */
function EntityTrail({
  shelf,
  smart,
  onShelf,
}: {
  shelf: Extract<Shelf, { kind: "entity" }>;
  smart: ReturnType<typeof buildSmart>;
  onShelf: (s: Shelf) => void;
}) {
  const find = (nodes: ReturnType<typeof smart.tree>): ReturnType<typeof smart.tree>[number] | undefined => {
    for (const n of nodes) {
      if (n.id === shelf.id) return n;
      const inner = find(n.children);
      if (inner) return inner;
    }
  };
  const node = find(smart.tree(shelf.role));
  const parents = smart.ancestors(shelf.id).filter((a) => smart.games(shelf.role, a).size > 0).reverse();
  const open = (id: string) => onShelf({ kind: "entity", role: shelf.role, id });
  return (
    <div className="shelf-row entity-trail">
      <span className="shelf-label" aria-hidden />
      <button className="link" data-nav onClick={() => onShelf({ kind: "browse", role: shelf.role })}>
        All {ROLE_LABEL[shelf.role].many.toLowerCase()}
      </button>
      {parents.map((p) => (
        <span key={p} className="trail-step">
          <Icon name="chevronRight" size={13} />
          <button className="link" data-nav onClick={() => open(p)}>
            {smart.label(p)}
          </button>
        </span>
      ))}
      <span className="trail-step">
        <Icon name="chevronRight" size={13} />
        <b>{smart.label(shelf.id)}</b>
      </span>
      <PartOf shelf={shelf} smart={smart} />
      {node && node.children.length > 0 && (
        <span className="trail-children">
          {node.children.map((c) => (
            <button key={c.id} className="chip" data-nav onClick={() => open(c.id)}>
              {c.label}
              <small>{c.count}</small>
            </button>
          ))}
        </span>
      )}
    </div>
  );
}

/**
 * Fix where a company or series sits, where Wikidata is wrong or silent:
 * EA Romania names no owner there. What's set here outlasts every refresh.
 */
function PartOf({ shelf, smart }: { shelf: Extract<Shelf, { kind: "entity" }>; smart: ReturnType<typeof buildSmart> }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const direct = smart.parents(shelf.id);
  // Candidates: what the same shelves list (companies for studios, series
  // for series), never the entity itself or anything inside it.
  const options = useMemo(() => {
    const roles: SmartRole[] = shelf.role === "series" ? ["series"] : ["developer", "publisher"];
    const all = new Map<string, string>();
    const inside = new Set<string>();
    const walk = (nodes: ReturnType<typeof smart.tree>, under: boolean) => {
      for (const n of nodes) {
        const mine = under || n.id === shelf.id;
        if (mine) inside.add(n.id);
        all.set(n.id, n.label);
        walk(n.children, mine);
      }
    };
    roles.forEach((r) => walk(smart.tree(r), false));
    return [...all].filter(([id]) => !inside.has(id) && !direct.includes(id)).map(([id, label]) => ({ id, label }));
  }, [smart, shelf, direct]);
  const q = text.trim().toLowerCase();
  const matches = q ? options.filter((o) => o.label.toLowerCase().includes(q)).slice(0, 6) : [];
  const set = async (parent: string, on: boolean) => {
    setProblem(null);
    try {
      await api.setEntityParent(shelf.id, parent, on);
      await loadSmart();
      setText("");
    } catch (e) {
      setProblem(String(e));
    }
  };

  if (!open) {
    return (
      <button className="link part-of-toggle" data-nav onClick={() => setOpen(true)} title="Wikidata wrong or silent? Say what this is part of">
        Part of…
      </button>
    );
  }
  return (
    <span className="part-of">
      {direct.map((p) => (
        <span key={p} className="tag smart-chip">
          Part of {smart.label(p)}
          <button className="chip-x" data-nav aria-label={`${smart.label(shelf.id)} is not part of ${smart.label(p)}`} title="Not part of this" onClick={() => set(p, false)}>
            <Icon name="close" size={11} />
          </button>
        </span>
      ))}
      <span className="part-of-field">
        <input
          className="text-input"
          data-nav
          autoFocus
          value={text}
          placeholder={shelf.role === "series" ? "Part of which series?" : "Part of which company?"}
          aria-label="Part of"
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setOpen(false);
            if (e.key === "Enter" && matches[0]) set(matches[0].id, true);
          }}
        />
        {matches.length > 0 && (
          <span className="smart-suggest" role="listbox">
            {matches.map((m) => (
              <button key={m.id} role="option" aria-selected={false} data-nav onClick={() => set(m.id, true)}>
                {m.label}
              </button>
            ))}
          </span>
        )}
      </span>
      <button className="link" data-nav onClick={() => setOpen(false)}>
        Done
      </button>
      {problem && <span className="note error">{problem}</span>}
    </span>
  );
}
