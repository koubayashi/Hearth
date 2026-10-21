import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Game } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { Art } from "./Art";

const MIN = 150;
const GAP = 18;

/** Virtualized cover grid: only the rows on screen exist, so 2,000+ games scroll at 60fps. */
export function CoverGrid({
  games,
  focusedId,
  scrollRef,
  onFocusGame,
  onActivate,
}: {
  games: Game[];
  focusedId: number | null;
  scrollRef: RefObject<HTMLElement | null>;
  onFocusGame: (g: Game) => void;
  onActivate: (g: Game) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [offset, setOffset] = useState(0);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      setWidth(el.clientWidth);
      setOffset(el.offsetTop);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const cols = Math.max(1, Math.floor((width + GAP) / (MIN + GAP)));
  const cell = width > 0 ? (width - GAP * (cols - 1)) / cols : MIN;
  const rowH = cell * 1.5 + GAP;
  const rows = Math.ceil(games.length / cols);

  const v = useVirtualizer({
    count: rows,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowH,
    overscan: 3,
    scrollMargin: offset,
  });
  // Column count or cell size changed: every row height is stale.
  useLayoutEffect(() => v.measure(), [rowH, v]);

  return (
    <div ref={ref} style={{ position: "relative", height: v.getTotalSize() }}>
      {v.getVirtualItems().map((row) => (
        <div
          key={row.key}
          className="grid-row"
          style={{
            top: row.start - offset,
            gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
          }}
        >
          {games.slice(row.index * cols, row.index * cols + cols).map((g) => (
            <button
              key={g.id}
              data-nav
              data-game={g.id}
              className={`cover ${g.installed ? "" : "out"} ${g.id === focusedId ? "lit" : ""}`}
              aria-label={`${g.title}, ${STORE_LABEL[g.entries[0]?.store ?? "manual"]}${g.installed ? "" : ", not installed"}`}
              onFocus={() => onFocusGame(g)}
              onClick={() => onFocusGame(g)}
              onDoubleClick={() => onActivate(g)}
              onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), onActivate(g))}
            >
              <Art src={g.cover} title={g.title} />
              {!g.installed && <span className="badge">Not installed</span>}
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
