import { Fragment } from "react";
import { Icon, type IconName } from "./Icon";
import { StoreIcon } from "./StoreIcon";
import { pickName, useAccounts } from "../lib/gamertag";
import { usePrefs } from "../lib/prefs";
import type { StoreId } from "../lib/types";

export type Route = "library" | "collections" | "stats" | "hidden" | "settings";

const ITEMS: { id: Route; label: string; icon: IconName }[] = [
  { id: "library", label: "Library", icon: "library" },
  { id: "collections", label: "Collections", icon: "collections" },
  { id: "stats", label: "Stats", icon: "stats" },
];

export function Rail({
  route,
  counts,
  onNavigate,
  onConsole,
}: {
  onConsole: () => void;
  route: Route;
  counts: Partial<Record<Route, number>>;
  onNavigate: (r: Route) => void;
}) {
  const item = (id: Route, label: string, icon: IconName) => (
    <button
      key={id}
      className="nav"
      data-nav
      aria-current={route === id ? "page" : undefined}
      // Hidden games are a place inside the library: Library reads as their parent.
      data-parent={id === "library" && route === "hidden" ? "" : undefined}
      onClick={() => onNavigate(id)}
    >
      <Icon name={icon} />
      {label}
      {counts[id] !== undefined && <small>{counts[id]}</small>}
    </button>
  );
  return (
    <nav className="rail glass" aria-label="Main">
      <div className="brand">
        <Icon name="flame" size={22} />
        Hearth
      </div>
      {ITEMS.map((i) => (
        <Fragment key={i.id}>
          {item(i.id, i.label, i.icon)}
          {/* Hidden games open from Settings and have no rail item of their
              own. While they're open, one unfolds under Library, lit as
              where you are, and folds back into it when you leave. */}
          {i.id === "library" && (
            <div className="nav-sub-wrap" data-open={route === "hidden" ? "" : undefined} inert={route !== "hidden"}>
              <div className="nav-sub-clip">
                <div className="nav nav-sub" aria-current={route === "hidden" ? "page" : undefined}>
                  <Icon name="hidden" size={16} />
                  Hidden games
                  {counts.hidden !== undefined && <small>{counts.hidden}</small>}
                  <button
                    className="nav-sub-close"
                    data-nav
                    title="Back to library (Esc)"
                    aria-label="Leave hidden games"
                    onClick={() => onNavigate("library")}
                  >
                    <Icon name="close" size={12} />
                  </button>
                </div>
              </div>
            </div>
          )}
        </Fragment>
      ))}
      <div className="spacer" />
      <button className="nav" data-nav title="Fullscreen, for the couch and a controller (Start button)" onClick={onConsole}>
        <Icon name="gamepad" />
        Console
      </button>
      {item("settings", "Settings", "settings")}
      <Profile onOpen={() => onNavigate("settings")} />
    </nav>
  );
}

/**
 * Who's signed in, at the foot of the rail: your name (the one chosen in
 * Settings, shared with Stats and the console) and the stores you're signed
 * in to. Opens Settings, where both are changed.
 */
function Profile({ onOpen }: { onOpen: () => void }) {
  const accounts = useAccounts();
  const p = usePrefs();
  const picked = pickName(accounts, p);
  if (!picked) return null;
  const stores = (["steam", "xbox", "epic", "gog"] as StoreId[]).filter((s) => accounts?.[s as "steam" | "xbox" | "epic" | "gog"]);
  const initial = [...picked.name.trim()][0]?.toUpperCase() ?? "?";
  return (
    <button className="nav profile" data-nav title="Your name and store accounts, in Settings" onClick={onOpen}>
      <span className="monogram" aria-hidden>
        {initial}
      </span>
      <span className="who">
        <b>{picked.name}</b>
        {stores.length > 0 && (
          <span className="who-stores" aria-label={`Signed in to ${stores.length} ${stores.length === 1 ? "store" : "stores"}`}>
            {stores.map((s) => (
              <StoreIcon key={s} store={s} size={12} />
            ))}
          </span>
        )}
      </span>
    </button>
  );
}
