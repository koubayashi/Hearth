import { Fragment } from "react";
import { Icon, type IconName } from "./Icon";

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
    </nav>
  );
}
