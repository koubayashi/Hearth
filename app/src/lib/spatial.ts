// Arrow-key focus movement between [data-nav] elements by on-screen position.
// With the gamepad bridge this is also D-pad navigation.

type Dir = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight";

function candidates(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-nav]")).filter(
    // Laid out, and not inside an inert area (the library behind Settings).
    (el) => el.offsetParent !== null && !el.closest("[inert]"),
  );
}

export function moveFocus(from: HTMLElement, dir: Dir): boolean {
  const a = from.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;

  for (const el of candidates()) {
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
  best.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
  return true;
}

export function installSpatialNav(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (!e.key.startsWith("Arrow")) return;
    const target = e.target as HTMLElement;
    if (target.matches("input, textarea, select")) return;
    if (!target.closest("[data-nav]")) {
      const first = candidates()[0];
      if (first) {
        first.focus();
        e.preventDefault();
      }
      return;
    }
    if (moveFocus(target.closest<HTMLElement>("[data-nav]")!, e.key as Dir)) e.preventDefault();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
