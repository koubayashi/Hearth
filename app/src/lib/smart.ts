import { useSyncExternalStore } from "react";
import { api } from "./api";
import type { SmartIndex, SmartRole } from "./types";

/** A series or company on a "browse by" shelf, with what's part of it. */
export interface SmartNode {
  id: string;
  label: string;
  /** Games in it or in anything part of it. */
  count: number;
  children: SmartNode[];
}

export interface SmartLink {
  id: string;
  label: string;
  byUser: boolean;
}

export const ROLE_LABEL: Record<SmartRole, { one: string; many: string }> = {
  series: { one: "Series", many: "Series" },
  developer: { one: "Developer", many: "Developers" },
  publisher: { one: "Publisher", many: "Publishers" },
};

export interface Smart {
  label: (id: string) => string;
  /** Games linked in this role to this entity or anything part of it. */
  games: (role: SmartRole, id: string) => Set<number>;
  /** Top-level series or companies for a role, biggest first. */
  tree: (role: SmartRole) => SmartNode[];
  /** The groups an entity sits in, nearest first (Ubisoft Montreal → Ubisoft). */
  ancestors: (id: string) => string[];
  /** The groups an entity sits in directly, as the shelves show them. */
  parents: (id: string) => string[];
  /** A game's links, by role: tidied for its panel, or every one (for editing them). */
  of: (gameId: number, mode?: "panel" | "all") => Record<SmartRole, SmartLink[]>;
}

const ROLES: SmartRole[] = ["series", "developer", "publisher"];

/**
 * Builds the shelves from the index, for the games in `visible` (the library
 * without hidden games).
 *
 * Wikidata knows every owner a studio ever had and every umbrella a series
 * sits under. Only groupings that mean something in this library are kept: a
 * parent groups its children when it is credited on a game here itself, or
 * gathers two or more of them. So Ubisoft gathers Ubisoft Montreal, but an
 * old owner nobody here has a game from doesn't swallow Rockstar North.
 */
export function buildSmart(index: SmartIndex, visible: Set<number>): Smart {
  const labels = new Map(index.entities.map((e) => [e.id, e.label]));
  const raw = index.links.filter((l) => visible.has(l.game_id) && labels.has(l.entity));
  // A franchise shows as a series: the broadest one, holding the others.
  const links = raw.map((l) => ({ ...l, role: (l.role === "franchise" ? "series" : l.role) as SmartRole }));
  const parents = [...index.parents, ...inferParents(raw, index.parents, labels)];

  // Series and companies are grouped separately: a company never gathers a
  // series, nor the other way round.
  const creditedAs = (series: boolean) => new Set(links.filter((l) => (l.role === "series") === series).map((l) => l.entity));

  const keptParents = new Map<string, string[]>();
  for (const series of [true, false]) {
    const credited = creditedAs(series);
    const other = creditedAs(!series);
    const edges = parents.filter((p) => p.child !== p.parent && labels.has(p.parent) && !other.has(p.parent));
    const useful = new Set(credited);
    for (let grew = true; grew; ) {
      grew = false;
      const gathers = new Map<string, number>();
      for (const e of edges) if (useful.has(e.child) && !useful.has(e.parent)) gathers.set(e.parent, (gathers.get(e.parent) ?? 0) + 1);
      for (const [p, n] of gathers) {
        if (n >= 2) {
          useful.add(p);
          grew = true;
        }
      }
    }
    for (const e of edges) {
      if (!useful.has(e.child) || !useful.has(e.parent)) continue;
      const list = keptParents.get(e.child) ?? [];
      if (!list.includes(e.parent)) keptParents.set(e.child, [...list, e.parent]);
    }
  }

  const ancestorsOf = (id: string): string[] => {
    const out: string[] = [];
    const queue = [...(keptParents.get(id) ?? [])];
    while (queue.length) {
      const p = queue.shift()!;
      if (p === id || out.includes(p)) continue;
      out.push(p);
      queue.push(...(keptParents.get(p) ?? []));
    }
    return out;
  };
  // A studio listed under both Rockstar Games and Take-Two (which owns
  // Rockstar Games) only needs the nearer: the other is implied.
  const parentsOf = new Map<string, string[]>();
  for (const [child, ps] of keptParents) {
    parentsOf.set(
      child,
      ps.filter((p) => !ps.some((q) => q !== p && ancestorsOf(q).includes(p))),
    );
  }
  const childrenOf = new Map<string, string[]>();
  for (const [child, ps] of parentsOf) for (const p of ps) childrenOf.set(p, [...(childrenOf.get(p) ?? []), child]);

  const within = (id: string): Set<string> => {
    const out = new Set<string>();
    const stack = [id];
    while (stack.length) {
      const x = stack.pop()!;
      if (out.has(x)) continue;
      out.add(x);
      stack.push(...(childrenOf.get(x) ?? []));
    }
    return out;
  };

  // Game ids per "role entity", so a shelf's count doesn't scan every link.
  const direct = new Map<string, number[]>();
  for (const l of links) {
    const key = `${l.role} ${l.entity}`;
    direct.set(key, [...(direct.get(key) ?? []), l.game_id]);
  }
  const cache = new Map<string, Set<number>>();
  const games = (role: SmartRole, id: string) => {
    const key = `${role} ${id}`;
    let found = cache.get(key);
    if (!found) {
      found = new Set([...within(id)].flatMap((e) => direct.get(`${role} ${e}`) ?? []));
      cache.set(key, found);
    }
    return found;
  };

  const tree = (role: SmartRole): SmartNode[] => {
    const shown = new Set<string>();
    for (const l of links) {
      if (l.role !== role) continue;
      shown.add(l.entity);
      for (const a of ancestorsOf(l.entity)) shown.add(a);
    }
    const node = (id: string, path: Set<string>): SmartNode => {
      const label = labels.get(id) ?? id;
      const kids = (childrenOf.get(id) ?? []).filter((c) => shown.has(c) && !path.has(c)).map((c) => node(c, new Set(path).add(c)));
      // A franchise and its series often share a name (Assassin's Creed):
      // one card, holding what the series holds.
      const children = kids.flatMap((k) => (sameName(k.label, label) ? k.children : [k]));
      return { id, label, count: games(role, id).size, children: sorted(children) };
    };
    const roots = [...shown].filter((id) => !(parentsOf.get(id) ?? []).some((p) => shown.has(p)));
    return sorted(roots.map((id) => node(id, new Set([id]))));
  };

  const of = (gameId: number, mode: "panel" | "all" = "panel") => {
    const out = { series: [], developer: [], publisher: [] } as Record<SmartRole, SmartLink[]>;
    for (const l of links) if (l.game_id === gameId) out[l.role].push({ id: l.entity, label: labels.get(l.entity)!, byUser: l.by_user });
    for (const r of ROLES) {
      out[r] = mode === "all"
        ? out[r].sort((a, b) => a.label.localeCompare(b.label))
        : gatherSiblings(mostSpecific(out[r], (id) => ancestorsOf(id)), (id) => parentsOf.get(id) ?? [], labels);
    }
    return out;
  };

  return { label: (id) => labels.get(id) ?? id, games, tree, ancestors: ancestorsOf, parents: (id) => parentsOf.get(id) ?? [], of };
}

const sorted = (nodes: SmartNode[]) =>
  nodes.filter((n) => n.count > 0).sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));

const words = (s: string) => s.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
const sameName = (a: string, b: string) => words(a).join(" ") === words(b).join(" ");

/**
 * "Is part of" steps Wikidata leaves out, read from how a game is linked:
 * - a game's series sits inside the game's franchise (Hogwarts Legacy inside
 *   Wizarding World);
 * - of two series on one game, the one whose name the other starts with
 *   holds it (Forza holds Forza Horizon).
 * Never where Wikidata already links the two, either way round.
 */
export function inferParents(
  links: SmartIndex["links"],
  known: SmartIndex["parents"],
  labels: Map<string, string>,
): SmartIndex["parents"] {
  const up = new Map<string, string[]>();
  for (const p of known) up.set(p.child, [...(up.get(p.child) ?? []), p.parent]);
  const reaches = (from: string, to: string) => {
    const seen = new Set<string>();
    const stack = [from];
    while (stack.length) {
      const x = stack.pop()!;
      if (x === to) return true;
      if (seen.has(x)) continue;
      seen.add(x);
      stack.push(...(up.get(x) ?? []));
    }
    return false;
  };
  const byGame = new Map<number, { series: string[]; franchise: string[] }>();
  for (const l of links) {
    if (l.role !== "series" && l.role !== "franchise") continue;
    const g = byGame.get(l.game_id) ?? { series: [], franchise: [] };
    g[l.role].push(l.entity);
    byGame.set(l.game_id, g);
  }
  const out: SmartIndex["parents"] = [];
  const add = (child: string, parent: string) => {
    if (child === parent || reaches(child, parent) || reaches(parent, child)) return;
    out.push({ child, parent });
    up.set(child, [...(up.get(child) ?? []), parent]);
  };
  for (const { series, franchise } of byGame.values()) {
    for (const s of series) for (const f of franchise) add(s, f);
    for (const a of series) {
      for (const b of series) {
        const wa = words(labels.get(a) ?? "");
        const wb = words(labels.get(b) ?? "");
        if (wb.length > 0 && wa.length > wb.length && wb.every((w, i) => wa[i] === w)) add(a, b);
      }
    }
  }
  return out;
}

/** Studios gathered under their owner once there are this many of them. */
const GATHER_AT = 3;

/**
 * Three or more studios of one owner (GTA V's six Rockstar studios) read as
 * that owner, once: "Rockstar Games · 6 studios", opening its shelf.
 */
function gatherSiblings(list: SmartLink[], parentsOf: (id: string) => string[], labels: Map<string, string>): SmartLink[] {
  const byParent = new Map<string, SmartLink[]>();
  for (const l of list) for (const p of parentsOf(l.id)) byParent.set(p, [...(byParent.get(p) ?? []), l]);
  const out: SmartLink[] = [];
  const done = new Set<string>();
  for (const l of list) {
    if (done.has(l.id)) continue;
    const group = parentsOf(l.id)
      .map((p) => [p, byParent.get(p) ?? []] as const)
      .find(([, kids]) => kids.length >= GATHER_AT);
    if (!group) {
      out.push(l);
      continue;
    }
    const [parent, kids] = group;
    kids.forEach((k) => done.add(k.id));
    out.push({ id: parent, label: `${labels.get(parent) ?? parent} · ${kids.length} studios`, byUser: kids.some((k) => k.byUser) });
  }
  return out;
}

/** A game's links without the broader ones another link already implies,
 *  and without two of the same name: "Assassin's Creed", once. */
function mostSpecific(list: SmartLink[], ancestors: (id: string) => string[]): SmartLink[] {
  const implied = new Set(list.flatMap((l) => ancestors(l.id)));
  const kept: SmartLink[] = [];
  for (const l of list) {
    if (implied.has(l.id) || kept.some((k) => sameName(k.label, l.label))) continue;
    kept.push(l);
  }
  return kept.sort((a, b) => a.label.localeCompare(b.label));
}

// The index is read once per library change and shared by every screen.
const EMPTY: SmartIndex = { entities: [], parents: [], links: [] };
let current: SmartIndex = EMPTY;
const listeners = new Set<() => void>();

export async function loadSmart() {
  try {
    current = await api.smartIndex();
    listeners.forEach((l) => l());
  } catch {
    // Shelves stay as they were.
  }
}

export function useSmartIndex(): SmartIndex {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => current,
  );
}

// The panel's links use the same grouping as the shelves, built once per index.
const built = new WeakMap<SmartIndex, Smart>();

/** One game's series, developers and publishers: for its panel, the most
 *  specific ones (Forza Horizon, not also Forza); for editing, all of them. */
export function useGameLinks(gameId: number | null, mode: "panel" | "all" = "panel"): Record<SmartRole, SmartLink[]> {
  const index = useSmartIndex();
  if (gameId === null) return { series: [], developer: [], publisher: [] };
  let smart = built.get(index);
  if (!smart) {
    smart = buildSmart(index, new Set(index.links.map((l) => l.game_id)));
    built.set(index, smart);
  }
  return smart.of(gameId, mode);
}

// Whether Hearth is asking Wikidata right now, so shelves can say "looking
// up" instead of showing zero.
let looking = false;
const lookingListeners = new Set<() => void>();
// Each batch Wikidata answers shows at once.
void api.onSmartChanged(() => void loadSmart());
void api.onSmartStatus((on) => {
  looking = on === true;
  lookingListeners.forEach((l) => l());
});

export function useSmartLooking(): boolean {
  return useSyncExternalStore(
    (l) => (lookingListeners.add(l), () => lookingListeners.delete(l)),
    () => looking,
  );
}
