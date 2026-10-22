import { Icon, type IconName } from "./Icon";

export type Route = "library" | "collections" | "stats" | "hidden" | "backloggd" | "settings";

const ITEMS: { id: Route; label: string; icon: IconName }[] = [
  { id: "library", label: "Library", icon: "library" },
  { id: "collections", label: "Collections", icon: "collections" },
  { id: "stats", label: "Stats", icon: "stats" },
  { id: "hidden", label: "Hidden", icon: "hidden" },
];

export function Rail({
  route,
  counts,
  onNavigate,
  showBackloggd,
}: {
  showBackloggd: boolean;
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
      {ITEMS.map((i) => item(i.id, i.label, i.icon))}
      {showBackloggd && item("backloggd", "Backloggd", "book")}
      <div className="spacer" />
      {item("settings", "Settings", "settings")}
    </nav>
  );
}
