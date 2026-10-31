import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { SORTS, type Narrow, type SortKey } from "../lib/arrange";
import { Icon } from "./Icon";

/** Genres listed before "More genres": the long tail stays out of the way. */
const GENRES_SHOWN = 8;

/**
 * Sort order and the filters that don't earn a place in the toolbar: genres,
 * and the way to hidden games. Opens under its button.
 */
export function ArrangeMenu({
  anchor,
  sort,
  narrow,
  genres,
  hiddenCount,
  onSort,
  onNarrow,
  onShowHidden,
  onClose,
}: {
  anchor: DOMRect;
  sort: SortKey;
  narrow: Narrow;
  /** Genres in the library, most common first. */
  genres: string[];
  hiddenCount: number;
  onSort: (s: SortKey) => void;
  onNarrow: (n: Narrow) => void;
  /** Absent where hidden games can't be opened from (the hidden view itself). */
  onShowHidden?: () => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x: anchor.left, y: anchor.bottom + 6 });
  const [allGenres, setAllGenres] = useState(false);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.max(8, Math.min(anchor.right - r.width, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(anchor.bottom + 6, window.innerHeight - r.height - 8)),
    });
  }, [anchor, allGenres]);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
  }, []);
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

  const toggleGenre = (g: string) =>
    onNarrow({ ...narrow, genres: narrow.genres.includes(g) ? narrow.genres.filter((x) => x !== g) : [...narrow.genres, g] });
  // Chosen genres stay listed even when they're in the long tail.
  const listed = allGenres ? genres : [...new Set([...genres.slice(0, GENRES_SHOWN), ...narrow.genres])];
  const check = (on: boolean) => <span className="check-box">{on && <Icon name="check" size={13} />}</span>;

  return createPortal(
    <div
      ref={ref}
      className="menu context-menu arrange-menu"
      role="menu"
      aria-label="Sort and filter"
      style={{ left: pos.x, top: pos.y }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        e.preventDefault();
        e.stopPropagation();
        const items = [...(ref.current?.querySelectorAll<HTMLElement>("button") ?? [])];
        const i = items.indexOf(document.activeElement as HTMLElement);
        const next = e.key === "ArrowDown" ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
        items[next]?.focus();
      }}
    >
      <div className="menu-label">Sort by</div>
      {SORTS.map((s) => (
        <button key={s.id} role="menuitemradio" aria-checked={sort === s.id} onClick={() => onSort(s.id)}>
          {check(sort === s.id)}
          {s.label}
        </button>
      ))}
      {genres.length > 0 && (
        <>
          <hr />
          <div className="menu-label">Genre</div>
          {listed.map((g) => (
            <button key={g} role="menuitemcheckbox" aria-checked={narrow.genres.includes(g)} onClick={() => toggleGenre(g)}>
              {check(narrow.genres.includes(g))}
              {g}
            </button>
          ))}
          {!allGenres && genres.length > listed.length && (
            <button role="menuitem" onClick={() => setAllGenres(true)}>
              <Icon name="chevronDown" size={15} />
              {genres.length - listed.length} more genres
            </button>
          )}
        </>
      )}
      {(narrow.favorites || narrow.genres.length > 0) && (
        <button role="menuitem" onClick={() => onNarrow({ favorites: false, genres: [] })}>
          <Icon name="close" size={15} />
          Clear filters
        </button>
      )}
      {onShowHidden && (
        <>
          <hr />
          <button
            role="menuitem"
            onClick={() => {
              onClose();
              onShowHidden();
            }}
          >
            <Icon name="hidden" />
            Hidden games
            <small className="menu-count">{hiddenCount}</small>
          </button>
        </>
      )}
    </div>,
    document.body,
  );
}
