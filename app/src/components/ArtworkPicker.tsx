import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { ArtKind, ArtOption, Game } from "../lib/types";
import { Icon } from "./Icon";

const TABS: { kind: ArtKind; label: string }[] = [
  { kind: "cover", label: "Cover" },
  { kind: "hero", label: "Background" },
];

/** Choose a game's cover and background from SteamGridDB's community art. */
export function ArtworkPicker({ game, onDone, onClose }: { game: Game; onDone: () => void; onClose: () => void }) {
  const [kind, setKind] = useState<ArtKind>("cover");
  const [options, setOptions] = useState<ArtOption[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    setOptions(null);
    setError(null);
    api
      .artworkOptions(game.id, kind)
      .then((o) => live && setOptions(o))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [game.id, kind]);

  const pick = async (url: string) => {
    setBusy(true);
    try {
      await api.setArtwork(game.id, kind, url);
      onDone();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  const reset = async () => {
    setBusy(true);
    try {
      await api.resetArtwork(game.id);
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
      aria-label={`Change artwork for ${game.title}`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="match-head">
        <b>Change artwork</b>
        <button className="icon-btn small" data-nav aria-label="Close" onClick={onClose}>
          <Icon name="close" size={14} />
        </button>
      </div>
      <div className="seg art-tabs" role="tablist">
        {TABS.map((t) => (
          <button
            key={t.kind}
            role="tab"
            data-nav
            aria-selected={kind === t.kind}
            aria-pressed={kind === t.kind}
            onClick={() => setKind(t.kind)}
          >
            {t.label}
          </button>
        ))}
      </div>
      {error && <p className="note error">{error}</p>}
      {!error && options === null && <p className="note">Loading community artwork…</p>}
      {options?.length === 0 && <p className="note">SteamGridDB has no {kind === "cover" ? "covers" : "backgrounds"} for this game yet.</p>}
      <div className={`art-grid ${kind}`}>
        {options?.map((o) => (
          <button key={o.url} data-nav disabled={busy} onClick={() => pick(o.url)} title={o.style || "Artwork"}>
            <img src={o.thumb} alt="" loading="lazy" />
          </button>
        ))}
      </div>
      <button className="link" data-nav disabled={busy} onClick={reset}>
        Use the store's artwork again
      </button>
    </div>
  );
}
