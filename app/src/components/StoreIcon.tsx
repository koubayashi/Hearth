import type { StoreId } from "../lib/types";
import { STORE_LABEL } from "../lib/types";

// Minimal single-colour marks that hint at each store, drawn on the same
// 24px grid and 1.8 stroke as the rest of the icon set.
const marks: Record<StoreId, React.ReactNode> = {
  // Steam: the piston, a small wheel linked to a large one.
  steam: (
    <>
      <circle cx="12" cy="12" r="9.2" />
      <circle cx="15.2" cy="9.3" r="2.6" />
      <path d="M13 11l-4.6 3.6" />
      <circle cx="8" cy="15.2" r="1.5" />
    </>
  ),
  // Epic: the shield with an E.
  epic: (
    <>
      <path d="M5.5 3h13v14l-6.5 4-6.5-4z" />
      <path d="M14.5 7.5h-5v7h5M9.5 11h4" />
    </>
  ),
  // Xbox: a sphere crossed by two arcs.
  xbox: (
    <>
      <circle cx="12" cy="12" r="9.2" />
      <path d="M6.2 5.4c3.7 2.6 7.9 8.6 9.9 13.9M17.8 5.4c-3.7 2.6-7.9 8.6-9.9 13.9" />
    </>
  ),
  // GOG: a rounded tag with a G.
  gog: (
    <>
      <rect x="3" y="3" width="18" height="18" rx="4.5" />
      <path d="M15.6 9.2A4 4 0 1 0 16 13h-3.4" />
    </>
  ),
  // EA: a disc with the E and A strokes.
  ea: (
    <>
      <circle cx="12" cy="12" r="9.2" />
      <path d="M6.5 9.5h5M6.5 12h3.6M6.5 14.5h5M13 15l2.6-6 2.6 6M14.1 12.8h3" />
    </>
  ),
  // Ubisoft: the swirl.
  ubisoft: (
    <>
      <circle cx="12" cy="12" r="9.2" />
      <path d="M12 6.5a5.5 5.5 0 1 0 5.5 5.5" />
      <path d="M12 9.5a2.5 2.5 0 1 0 2.5 2.5" />
    </>
  ),
  // Anything added by hand or from another source: a box with an arrow in.
  manual: (
    <>
      <path d="M4 8.5V19a1.5 1.5 0 0 0 1.5 1.5h13A1.5 1.5 0 0 0 20 19V8.5" />
      <path d="M3 5h18v3.5H3zM12 11v6M9.5 14.5L12 17l2.5-2.5" />
    </>
  ),
};

export function StoreIcon({ store, size = 16 }: { store: StoreId; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      role="img"
      aria-label={STORE_LABEL[store]}
    >
      {marks[store]}
    </svg>
  );
}

/** Small glass chip in a cover's corner showing which stores have the game. */
export function StoreBadges({ stores }: { stores: StoreId[] }) {
  const unique = [...new Set(stores)];
  if (unique.length === 0) return null;
  return (
    <span className="store-badges" aria-hidden>
      {unique.map((s) => (
        <StoreIcon key={s} store={s} size={14} />
      ))}
    </span>
  );
}
