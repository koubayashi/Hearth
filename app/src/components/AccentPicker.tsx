import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/**
 * Highlight colours chosen to work on every tone, dark and light: at least
 * 3:1 against each ground (focus rings, selections) and white text at 4.5:1
 * on them (the Play button). prefs.test.ts checks them.
 */
export const ACCENT_PRESETS = [
  { hex: "#3b72d1", name: "Blue" },
  { hex: "#187f8b", name: "Teal" },
  { hex: "#268459", name: "Green" },
  { hex: "#a06a0d", name: "Amber" },
  { hex: "#bf5518", name: "Orange" },
  { hex: "#d53d32", name: "Red" },
  { hex: "#cc406e", name: "Rose" },
];

type Hsv = [h: number, s: number, v: number];

function hexToHsv(hex: string): Hsv {
  const n = Number.parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => c / 255);
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d) h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [(h * 60 + 360) % 360, max ? d / max : 0, max];
}

function hsvToHex([h, s, v]: Hsv): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round(255 * (v - v * s * Math.max(0, Math.min(k, 4 - k, 1))));
  };
  return "#" + [f(5), f(3), f(1)].map((c) => c.toString(16).padStart(2, "0")).join("");
}

const clamp = (x: number) => Math.min(1, Math.max(0, x));

/**
 * The highlight colour: the tone's own, one of the presets, or any colour from
 * a small panel that opens from the last swatch. Dragging in the panel
 * recolours Hearth as you go.
 */
export function AccentPicker({
  value,
  toneAccent,
  toneLabel,
  onChange,
}: {
  /** The custom highlight, or null for the tone's own. */
  value: string | null;
  toneAccent: string;
  toneLabel: string;
  onChange: (hex: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [closing, setClosing] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const well = useRef<HTMLButtonElement>(null);
  const isPreset = ACCENT_PRESETS.some((p) => p.hex === value);
  const custom = value && !isPreset ? value : null;

  // Closing plays the opening in reverse, and can be undone halfway by opening again.
  const close = (refocus = false) => {
    if (!open) return;
    // Without animations there's no animation end to wait for.
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) setOpen(false);
    else setClosing(true);
    if (refocus) well.current?.focus();
  };
  const toggle = () => (open && !closing ? close() : (setOpen(true), setClosing(false)));

  useEffect(() => {
    if (!open || closing) return;
    const outside = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!wrap.current?.contains(t) && !document.querySelector(".color-panel")?.contains(t)) close();
    };
    window.addEventListener("mousedown", outside);
    return () => window.removeEventListener("mousedown", outside);
  });

  return (
    <div className="accent-picker" ref={wrap}>
      <div className="accent-swatches" role="group" aria-label="Highlight colour">
        <Dot hex={toneAccent} label={`${toneLabel}'s own`} on={!value} onClick={() => onChange(null)} />
        {ACCENT_PRESETS.map((p) => (
          <Dot key={p.hex} hex={p.hex} label={p.name} on={value === p.hex} onClick={() => onChange(p.hex)} />
        ))}
        <button
          ref={well}
          className="accent-dot accent-well"
          data-nav
          aria-label={custom ? `Custom colour ${custom}` : "Pick any colour"}
          title={custom ? `Custom: ${custom}` : "Pick any colour"}
          aria-pressed={!!custom}
          aria-expanded={open && !closing}
          onClick={toggle}
        >
          {custom && <i style={{ background: custom }} />}
        </button>
      </div>
      {/* On the page itself: each settings group is a glass layer of its
          own, so a panel inside one would be drawn under the next. */}
      {open &&
        createPortal(
          <ColorPanel
            anchor={well}
            start={value ?? toneAccent}
            closing={closing}
            onChange={onChange}
            onClose={() => close(true)}
            onClosed={() => (setOpen(false), setClosing(false))}
          />,
          document.body,
        )}
    </div>
  );
}

function Dot({ hex, label, on, onClick }: { hex: string; label: string; on: boolean; onClick: () => void }) {
  return (
    <button className="accent-dot" data-nav aria-label={label} title={label} aria-pressed={on} onClick={onClick}>
      <i style={{ background: hex }} />
    </button>
  );
}

/** Where the panel goes: under the swatch, right edges lined up, or above it
 *  when the window has no room below. Followed while the page scrolls. */
function usePlace(anchor: React.RefObject<HTMLElement | null>, panel: React.RefObject<HTMLElement | null>) {
  const [place, setPlace] = useState<{ left: number; top: number; above: boolean } | null>(null);
  useLayoutEffect(() => {
    const measure = () => {
      const a = anchor.current?.getBoundingClientRect();
      const p = panel.current?.getBoundingClientRect();
      if (!a || !p) return;
      const gap = 10;
      const above = a.bottom + gap + p.height > window.innerHeight - 8 && a.top - gap - p.height > 8;
      setPlace({
        left: Math.max(8, Math.min(a.right - p.width, window.innerWidth - p.width - 8)),
        top: above ? a.top - gap - p.height : a.bottom + gap,
        above,
      });
    };
    measure();
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, true);
    return () => {
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
    };
  }, [anchor, panel]);
  return place;
}

function ColorPanel({
  anchor,
  start,
  closing,
  onChange,
  onClose,
  onClosed,
}: {
  anchor: React.RefObject<HTMLElement | null>;
  start: string;
  closing: boolean;
  onChange: (hex: string) => void;
  onClose: () => void;
  onClosed: () => void;
}) {
  // Hue is kept here: a grey has none, and dragging through one mustn't lose it.
  const [hsv, setHsv] = useState<Hsv>(() => hexToHsv(start));
  const [text, setText] = useState(start.slice(1));
  const pad = useRef<HTMLDivElement>(null);
  const self = useRef<HTMLDivElement>(null);
  const place = usePlace(anchor, self);

  useEffect(() => pad.current?.focus({ preventScroll: true }), []);

  const set = (next: Hsv) => {
    setHsv(next);
    const hex = hsvToHex(next);
    setText(hex.slice(1));
    onChange(hex);
  };

  // The pad follows the pointer from the press, also outside its edges.
  const fromPointer = (e: React.PointerEvent) => {
    const r = pad.current!.getBoundingClientRect();
    set([hsv[0], clamp((e.clientX - r.left) / r.width), clamp(1 - (e.clientY - r.top) / r.height)]);
  };

  const [h, s, v] = hsv;
  return (
    <div
      ref={self}
      className="color-panel"
      data-closing={closing || undefined}
      data-above={place?.above || undefined}
      // Measured before it's shown, so it never appears in the wrong place first.
      style={place ? { left: place.left, top: place.top } : { visibility: "hidden" }}
      role="dialog"
      aria-label="Pick a highlight colour"
      onAnimationEnd={() => closing && onClosed()}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div
        ref={pad}
        className="color-pad"
        tabIndex={0}
        data-nav
        role="slider"
        aria-label="Saturation and brightness"
        aria-valuenow={Math.round(s * 100)}
        aria-valuetext={`Saturation ${Math.round(s * 100)}%, brightness ${Math.round(v * 100)}%`}
        style={{ "--hue": h } as React.CSSProperties}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          fromPointer(e);
        }}
        onPointerMove={(e) => e.currentTarget.hasPointerCapture(e.pointerId) && fromPointer(e)}
        onKeyDown={(e) => {
          const step = e.shiftKey ? 0.1 : 0.02;
          const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
          const m = moves[e.key];
          if (!m) return;
          e.preventDefault();
          set([h, clamp(s + m[0]), clamp(v + m[1])]);
        }}
      >
        <span className="color-thumb" style={{ left: `${s * 100}%`, top: `${(1 - v) * 100}%`, background: hsvToHex(hsv) }} />
      </div>
      <input
        className="hue-slider"
        type="range"
        min={0}
        max={359}
        value={Math.round(h)}
        data-nav
        aria-label="Hue"
        onChange={(e) => set([Number(e.target.value), s, v])}
      />
      <label className="hex-field">
        <span aria-hidden>#</span>
        <input
          className="text-input"
          data-nav
          aria-label="Hex colour"
          value={text}
          maxLength={6}
          spellCheck={false}
          onChange={(e) => {
            const t = e.target.value.replace(/[^0-9a-f]/gi, "").toLowerCase();
            setText(t);
            if (t.length === 6) {
              setHsv(hexToHsv(`#${t}`));
              onChange(`#${t}`);
            }
          }}
        />
      </label>
    </div>
  );
}
