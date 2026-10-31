import { useMemo } from "react";
import type { Game, SmartRole } from "../lib/types";
import type { Smart, SmartNode } from "../lib/smart";
import { Art } from "./Art";

/** What a card's children are called. */
const INNER: Record<SmartRole, [string, string]> = {
  series: ["series", "series"],
  developer: ["studio", "studios"],
  publisher: ["label", "labels"],
};

/**
 * Every series, developer or publisher in the library as a card with a few
 * of its covers. Groups (a franchise, a parent company) say what they hold;
 * a search looks inside them too.
 */
export function SmartBrowser({
  role,
  smart,
  games,
  search,
  looking,
  onOpen,
}: {
  role: SmartRole;
  smart: Smart;
  games: Game[];
  search: string;
  /** Wikidata is being asked right now. */
  looking: boolean;
  onOpen: (id: string) => void;
}) {
  const byId = useMemo(() => new Map(games.map((g) => [g.id, g])), [games]);
  const tree = useMemo(() => smart.tree(role), [smart, role]);
  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return tree;
    const all: SmartNode[] = [];
    const walk = (ns: SmartNode[]) => ns.forEach((n) => (all.push(n), walk(n.children)));
    walk(tree);
    return [...new Map(all.filter((n) => n.label.toLowerCase().includes(q)).map((n) => [n.id, n])).values()];
  }, [tree, search]);

  if (tree.length === 0 && looking) {
    // The shape of what's coming, so the wait reads as loading, not as empty.
    return (
      <>
        <p className="note smart-looking">Looking up your games on Wikidata. They appear here as answers come in.</p>
        <div className="smart-grid" aria-hidden>
          {Array.from({ length: 6 }, (_, i) => (
            <span key={i} className="smart-card skeleton">
              <span className="smart-covers n3">
                <span className="smart-cover" />
                <span className="smart-cover" />
                <span className="smart-cover" />
              </span>
              <b />
              <small />
            </span>
          ))}
        </div>
      </>
    );
  }
  if (tree.length === 0) {
    return (
      <div className="empty">
        <h3>Nothing to browse yet</h3>
        After each sync Hearth looks up your games' series, developers and publishers on Wikidata, a couple of hundred
        at a time. A big library fills in over a few starts. Fix any game by hand from its menu, under Series and
        studios.
      </div>
    );
  }
  if (shown.length === 0) {
    return (
      <div className="empty">
        <h3>No {role === "series" ? "series" : role === "developer" ? "developers" : "publishers"} match</h3>
        Try a different search.
      </div>
    );
  }

  return (
    <div className="smart-grid">
      {shown.map((n) => {
        // The most played games make the best-known covers.
        const covers = [...smart.games(role, n.id)]
          .map((id) => byId.get(id))
          .filter((g): g is Game => !!g)
          .sort((a, b) => b.playtime_secs - a.playtime_secs)
          .slice(0, 3);
        const [one, many] = INNER[role];
        return (
          <button key={n.id} className="smart-card" data-nav onClick={() => onOpen(n.id)} title={`Show ${n.label}`}>
            <span className={`smart-covers n${covers.length}`} aria-hidden>
              {covers.map((g) => (
                <span key={g.id} className="smart-cover">
                  <Art src={g.cover} title={g.title} />
                </span>
              ))}
            </span>
            <b>{n.label}</b>
            <small>
              {n.count} {n.count === 1 ? "game" : "games"}
              {n.children.length > 0 && ` · ${n.children.length} ${n.children.length === 1 ? one : many}`}
            </small>
          </button>
        );
      })}
    </div>
  );
}
