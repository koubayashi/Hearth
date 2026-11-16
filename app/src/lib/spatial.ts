// Arrow-key focus movement between [data-nav] elements by on-screen position.
// With the gamepad bridge this is also D-pad navigation.

type Dir = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight";

const FOCUSABLE = "[data-nav], button:not(:disabled), input:not(:disabled), select, textarea, a[href], [tabindex]:not([tabindex='-1'])";

/** The open menu or dialog, the last one opened: focus stays inside it. */
function openLayer(): HTMLElement | null {
  const layers = document.querySelectorAll<HTMLElement>('[role="menu"], [role="dialog"]');
  return layers.length ? layers[layers.length - 1] : null;
}

function candidates(layer: HTMLElement | null): HTMLElement[] {
  const all = layer ? layer.querySelectorAll<HTMLElement>(FOCUSABLE) : document.querySelectorAll<HTMLElement>("[data-nav]");
  return Array.from(all).filter(
    // Laid out, and not inside an inert area (the library behind Settings).
    (el) => el.offsetParent !== null && !el.closest("[inert]"),
  );
}

function reducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function moveFocus(from: HTMLElement, dir: Dir, layer: HTMLElement | null = openLayer()): boolean {
  const a = from.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;

  for (const el of candidates(layer)) {
    if (el === from) continue;
    const b = el.getBoundingClientRect();
    const bx = b.left + b.width / 2;
    const by = b.top + b.height / 2;
    const dx = bx - ax;
    const dy = by - ay;
    const along = dir === "ArrowLeft" ? -dx : dir === "ArrowRight" ? dx : dir === "ArrowUp" ? -dy : dy;
    if (along <= 1) continue;
    const across = dir === "ArrowLeft" || dir === "ArrowRight" ? Math.abs(dy) : Math.abs(dx);
    // Strongly prefer staying in the same row/column.
    const score = along + across * 3;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  if (!best) return false;
  best.focus({ preventScroll: true });
  best.scrollIntoView({ block: "nearest", inline: "nearest", behavior: reducedMotion() ? "auto" : "smooth" });
  return true;
}

export function installSpatialNav(): () => void {
  const onKey = (e: KeyboardEvent) => {
    const layer = openLayer();
    const target = e.target as HTMLElement;
    // Tab cycles inside an open menu or dialog instead of leaving it.
    if (e.key === "Tab" && layer) {
      const items = candidates(layer);
      if (!items.length) return;
      const i = items.indexOf(document.activeElement as HTMLElement);
      const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : (i + 1) % items.length;
      items[next].focus();
      e.preventDefault();
      return;
    }
    if (!e.key.startsWith("Arrow") || e.defaultPrevented) return;
    const sideways = e.key === "ArrowLeft" || e.key === "ArrowRight";
    if (target.matches("textarea, select, input[type=number]")) return;
    // A slider takes left and right; up and down leave it. The controller's
    // keys are synthetic and don't move a slider by themselves.
    if (target.matches("input[type=range]") && sideways) {
      if (!e.isTrusted) {
        const range = target as HTMLInputElement;
        if (e.key === "ArrowRight") range.stepUp();
        else range.stepDown();
        range.dispatchEvent(new Event("input", { bubbles: true }));
        e.preventDefault();
      }
      return;
    }
    // Left and right move the caret in a text box; up and down leave it, so
    // a controller can't get stuck there.
    const input = target.matches("input:not([type=range])");
    if (input && sideways) return;
    const from = layer?.contains(target) ? target : target.closest<HTMLElement>("[data-nav]") ?? (input ? target : null);
    // Nothing focused yet, or focus is behind the open menu or dialog.
    if (!from || (layer && !layer.contains(from))) {
      const first = candidates(layer)[0];
      if (first) {
        first.focus();
        e.preventDefault();
      }
      return;
    }
    // Inside a menu or dialog an arrow with nowhere to go stays put.
    if (moveFocus(from, e.key as Dir, layer) || layer) e.preventDefault();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
