import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Game } from "../lib/types";
import { STORE_LABEL } from "../lib/types";
import { usePrefs } from "../lib/prefs";
import { formatPlaytime } from "../lib/format";
import { Art } from "./Art";
import { StoreBadges } from "./StoreIcon";

const GAP = 22;
/** Title and playtime lines under each cover, plus the space above the next row. */
const CAPTION = 44;
const ROW_GAP = 26;

/** Virtualized cover grid: only the rows on screen exist, so 2,000+ games scroll at 60fps. */
export function CoverGrid({
  games,
  focusedId,
  scrollRef,
  onFocusGame,
  onActivate,
  reveal,
}: {
  /** Scroll this game into view; the nonce makes repeat picks of one game work. */
  reveal?: { id: number; nonce: number } | null;
  games: Game[];
  focusedId: number | null;
  scrollRef: RefObject<HTMLElement | null>;
  onFocusGame: (g: Game) => void;
  onActivate: (g: Game) => void;
}) {
  const { showStoreIcons, coverSize: MIN } = usePrefs();
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [offset, setOffset] = useState(0);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    // The parent's ref may not be attached yet when this grid mounts in the
    // same render (the Hidden view), so find the scroller from the DOM too.
    const scroller = () => scrollRef.current ?? el.closest<HTMLElement>(".scroll");
    // Distance from the top of the scroll content; changes whenever a section
    // above this grid (Jump back in, another grid) grows or shrinks.
    const measure = () => {
      setWidth(el.clientWidth);
      const s = scroller();
      if (s) setOffset(el.getBoundingClientRect().top - s.getBoundingClientRect().top + s.scrollTop);
    };
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    const page = el.closest(".page");
    if (page) ro.observe(page);
    measure();
    return () => ro.disconnect();
  }, [scrollRef]);

  const cols = Math.max(1, Math.floor((width + GAP) / (MIN + GAP)));
  const cell = width > 0 ? (width - GAP * (cols - 1)) / cols : MIN;
  const rowH = cell * 1.5 + CAPTION + ROW_GAP;
  const rows = Math.ceil(games.length / cols);

  const v = useVirtualizer({
    count: rows,
    getScrollElement: () => scrollRef.current ?? ref.current?.closest<HTMLElement>(".scroll") ?? null,
    estimateSize: () => rowH,
    overscan: 3,
    scrollMargin: offset,
  });
  // Column count or cell size changed: every row height is stale.
  useLayoutEffect(() => v.measure(), [rowH, v]);

  useLayoutEffect(() => {
    if (!reveal) return;
    const i = games.findIndex((g) => g.id === reveal.id);
    if (i < 0) return;
    v.scrollToIndex(Math.floor(i / cols), { align: "center" });
    // Focus once the row exists, so arrows and A/Enter continue from there.
    // A long jump renders the row a frame or two later: retry briefly.
    let tries = 0;
    const focus = () => {
      const el = document.querySelector<HTMLElement>(`.cover[data-game="${reveal.id}"]`);
      if (el) el.focus({ preventScroll: true });
      else if (tries++ < 10) setTimeout(focus, 40);
    };
    requestAnimationFrame(focus);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reveal]);

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
              aria-label={`${g.title}, ${g.entries.map((e) => STORE_LABEL[e.store]).join(" and ")}${g.installed ? "" : ", not installed"}`}
              onFocus={() => onFocusGame(g)}
              onClick={() => onFocusGame(g)}
              onDoubleClick={() => onActivate(g)}
              onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), onActivate(g))}
            >
              <span className="cover-art">
                <Art src={g.cover} title={g.title} />
                {showStoreIcons && <StoreBadges stores={g.entries.map((e) => e.store)} />}
              </span>
              <span className="cover-title">{g.title}</span>
              <span className="cover-meta">
                {g.playtime_secs > 0 ? formatPlaytime(g.playtime_secs) : "Never played"}
              </span>
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
