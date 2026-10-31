import type { GameStatus, SmartRole } from "./types";

/** One shelf of the Collections page. */
export type Shelf =
  | { kind: "favorites" }
  | { kind: "status"; status: GameStatus }
  | { kind: "collection"; id: number }
  /** Every series, developer or publisher in the library, as cards. */
  | { kind: "browse"; role: SmartRole }
  /** The games of one series or company. */
  | { kind: "entity"; role: SmartRole; id: string };

export const shelfKey = (s: Shelf) =>
  s.kind === "status" ? `s-${s.status}` : s.kind === "collection" ? `c-${s.id}` : s.kind === "browse" ? `b-${s.role}` : s.kind === "entity" ? `e-${s.role}-${s.id}` : "favorites";

// Links deep in a panel ("Series: Max Payne") open a shelf without every
// screen passing a callback down: App listens here.
let handler: ((s: Shelf) => void) | null = null;

export function openShelf(s: Shelf) {
  handler?.(s);
}

export function onOpenShelf(h: (s: Shelf) => void) {
  handler = h;
  return () => {
    if (handler === h) handler = null;
  };
}
