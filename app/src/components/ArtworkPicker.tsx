import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
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
  const [broken, setBroken] = useState<Set<string>>(new Set());

  useEffect(() => {
    document.querySelector<HTMLElement>(".art-dialog [role=tab]")?.focus();
  }, []);

  useEffect(() => {
    let live = true;
    setOptions(null);
    setError(null);
    setBroken(new Set());
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

  const shown = options?.filter((o) => !broken.has(o.url)) ?? null;

  // A large window over the library: the game panel is too narrow to judge art.
  return createPortal(
    <div className="art-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
    <div
      className="art-dialog"
      role="dialog"
      aria-modal="true"
      aria-label={`Change artwork for ${game.title}`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="match-head">
        <b>Change artwork · {game.title}</b>
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
      {shown?.length === 0 && <p className="note">SteamGridDB has no {kind === "cover" ? "covers" : "backgrounds"} for this game yet.</p>}
      <div className={`art-grid ${kind}`}>
        {shown?.map((o) => (
          <button key={o.url} data-nav disabled={busy} onClick={() => pick(o.url)} title={o.style || "Artwork"}>
            <img
              src={o.thumb}
              alt=""
              loading="lazy"
              onError={() => setBroken((b) => new Set(b).add(o.url))}
            />
          </button>
        ))}
      </div>
      <button className="link" data-nav disabled={busy} onClick={reset}>
        Use the store's artwork again
      </button>
    </div>
    </div>,
    document.body,
  );
}
