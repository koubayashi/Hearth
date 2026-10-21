import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { Game, StoreHit } from "../lib/types";
import { Icon } from "./Icon";

/**
 * Pick a game's Steam store page by hand, for the few games Hearth can't
 * match by name. The choice supplies the game's details and artwork.
 */
export function StoreMatch({ game, onDone, onClose }: { game: Game; onDone: () => void; onClose: () => void }) {
  const [term, setTerm] = useState(game.title);
  const [hits, setHits] = useState<StoreHit[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  const search = async (t: string) => {
    if (!t.trim()) return;
    setBusy(true);
    setError(null);
    try {
      setHits(await api.searchStore(t));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    input.current?.select();
    search(game.title);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id]);

  const pick = async (h: StoreHit) => {
    setBusy(true);
    try {
      await api.setStoreMatch(game.id, h.steam_app_id);
      onDone();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <div
      className="match"
      role="dialog"
      aria-label={`Find ${game.title} on the Steam store`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="match-head">
        <b>Find on Steam store</b>
        <button className="icon-btn small" data-nav aria-label="Close" onClick={onClose}>
          <Icon name="close" size={14} />
        </button>
      </div>
      <p className="note">Pick this game's store page. Hearth uses its artwork and details.</p>
      <form
        className="key-row"
        onSubmit={(e) => {
          e.preventDefault();
          search(term);
        }}
      >
        <input
          ref={input}
          className="text-input"
          data-nav
          value={term}
          onChange={(e) => setTerm(e.target.value)}
          aria-label="Search the Steam store"
        />
        <button className="chip" data-nav type="submit" disabled={busy}>
          Search
        </button>
      </form>
      {error && <p className="note error">{error}</p>}
      <ul className="match-list">
        {hits?.length === 0 && <li className="note">No store pages found. Try a shorter name.</li>}
        {hits?.map((h) => (
          <li key={h.steam_app_id}>
            <button data-nav disabled={busy} onClick={() => pick(h)}>
              <img
                src={`https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/${h.steam_app_id}/capsule_231x87.jpg`}
                alt=""
                loading="lazy"
              />
              <span>{h.name}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
