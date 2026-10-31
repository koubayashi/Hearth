import { useMemo, useState } from "react";
import { api } from "../lib/api";
import { loadSmart, ROLE_LABEL, useGameLinks, useSmartIndex } from "../lib/smart";
import type { Game, SmartRole } from "../lib/types";
import { Icon } from "./Icon";

const ROLES: SmartRole[] = ["series", "developer", "publisher"];
const SUGGESTIONS = 6;

/**
 * Fix a game's series, developers and publishers by hand. Wikidata fills
 * these in; whatever is added or taken away here stays that way through
 * every refresh.
 */
export function SmartEditor({ game, onClose }: { game: Game; onClose: () => void }) {
  const links = useGameLinks(game.id, "all");
  const [problem, setProblem] = useState<string | null>(null);

  const act = async (f: () => Promise<unknown>) => {
    setProblem(null);
    try {
      await f();
      await loadSmart();
    } catch (e) {
      setProblem(String(e));
    }
  };

  return (
    <div className="art-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        className="ways-dialog smart-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Series and studios of ${game.title}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            onClose();
          }
        }}
      >
        <div className="match-head">
          <span>Series and studios · {game.title}</span>
          <button className="icon-btn small" data-nav aria-label="Close" onClick={onClose}>
            <Icon name="close" size={16} />
          </button>
        </div>
        <p className="note ways-intro">
          Found on Wikidata and used for Browse in Collections. Anything you add or remove here stays that way, even when
          Wikidata is checked again.
        </p>
        {ROLES.map((role) => (
          <RoleField
            key={role}
            role={role}
            game={game}
            current={links[role]}
            onAdd={(id) => act(() => api.setGameLink(game.id, role, id, true))}
            onRemove={(id) => act(() => api.setGameLink(game.id, role, id, false))}
            onCreate={(name) => act(() => api.createSeries(game.id, name))}
          />
        ))}
        {problem && (
          <p className="note error" role="alert">
            {problem}
          </p>
        )}
        <div className="way-actions">
          <button className="btn btn-primary" data-nav onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

function RoleField({
  role,
  game,
  current,
  onAdd,
  onRemove,
  onCreate,
}: {
  role: SmartRole;
  game: Game;
  current: { id: string; label: string }[];
  onAdd: (id: string) => void;
  onRemove: (id: string) => void;
  onCreate: (name: string) => void;
}) {
  const index = useSmartIndex();
  const [text, setText] = useState("");
  // What can be picked: everything already used in this role in the library,
  // plus the user's own series.
  const known = useMemo(() => {
    const used = new Set(index.links.filter((l) => l.role === role || (role === "series" && l.role === "franchise")).map((l) => l.entity));
    return index.entities.filter((e) => used.has(e.id) || (role === "series" && e.id.startsWith("user:")));
  }, [index, role]);
  const q = text.trim().toLowerCase();
  const suggestions = q
    ? known.filter((e) => e.label.toLowerCase().includes(q) && !current.some((c) => c.id === e.id)).slice(0, SUGGESTIONS)
    : [];
  const exact = known.some((e) => e.label.toLowerCase() === q);
  const pick = (id: string) => {
    onAdd(id);
    setText("");
  };

  return (
    <section className="smart-field" aria-label={ROLE_LABEL[role].many}>
      <h3>{ROLE_LABEL[role].many}</h3>
      <div className="smart-chips">
        {current.length === 0 && <span className="note">None yet.</span>}
        {current.map((c) => (
          <span key={c.id} className="tag smart-chip">
            {c.label}
            <button className="chip-x" data-nav aria-label={`Remove ${c.label} from ${game.title}`} title="Remove" onClick={() => onRemove(c.id)}>
              <Icon name="close" size={11} />
            </button>
          </span>
        ))}
      </div>
      <input
        className="text-input"
        data-nav
        value={text}
        placeholder={role === "series" ? "Add to a series, or name a new one" : `Add a ${ROLE_LABEL[role].one.toLowerCase()}`}
        aria-label={`Add a ${ROLE_LABEL[role].one.toLowerCase()}`}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          e.preventDefault();
          if (suggestions[0]) pick(suggestions[0].id);
          else if (role === "series" && q && !exact) {
            onCreate(text);
            setText("");
          }
        }}
      />
      {(suggestions.length > 0 || (role === "series" && q && !exact)) && (
        <div className="smart-suggest" role="listbox">
          {suggestions.map((s) => (
            <button key={s.id} role="option" aria-selected={false} data-nav onClick={() => pick(s.id)}>
              {s.label}
            </button>
          ))}
          {role === "series" && q && !exact && (
            <button
              role="option"
              aria-selected={false}
              data-nav
              onClick={() => {
                onCreate(text);
                setText("");
              }}
            >
              <Icon name="collections" size={14} />
              New series “{text.trim()}”
            </button>
          )}
        </div>
      )}
    </section>
  );
}
