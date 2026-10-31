import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../lib/api";
import type { ArtKind, ArtOption, Game } from "../lib/types";
import { Icon } from "./Icon";

const TABS: { kind: "cover" | "hero"; label: string }[] = [
  { kind: "cover", label: "Cover" },
  { kind: "hero", label: "Background" },
];

/**
 * Choose a game's cover and background: the store's own first, then
 * SteamGridDB's community art. Picking one applies it at once and keeps the
 * window open, so the other tab is a click away; Done closes it.
 */
export function ArtworkPicker({ game, onDone, onClose }: { game: Game; onDone: () => void; onClose: () => void }) {
  const [kind, setKind] = useState<"cover" | "hero">("cover");
  const [options, setOptions] = useState<ArtOption[] | null>(null);
  const [store, setStore] = useState<{ cover: string | null; hero: string | null }>({ cover: null, hero: null });
  // What each tab shows now: the game's art, then whatever was picked here.
  const [chosen, setChosen] = useState<{ cover: string | null; hero: string | null }>({ cover: game.cover, hero: game.hero });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [broken, setBroken] = useState<Set<string>>(new Set());

  useEffect(() => {
    document.querySelector<HTMLElement>(".art-dialog [role=tab]")?.focus();
    api.storeArtwork(game.id).then(setStore).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    let live = true;
    setOptions(null);
    setError(null);
    setBroken(new Set());
    api
      .artworkOptions(game.id, kind as ArtKind)
      .then((o) => live && setOptions(o))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [game.id, kind]);

  const pick = async (url: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.setArtwork(game.id, kind, url);
      setChosen((c) => ({ ...c, [kind]: url }));
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const reset = async () => {
    setBusy(true);
    try {
      await api.resetArtwork(game.id);
      onDone();
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  const storeUrl = store[kind];
  // The store's picture leads the grid, and isn't repeated if SteamGridDB has it too.
  const community = options?.filter((o) => !broken.has(o.url) && o.url !== storeUrl) ?? null;
  const tile = (url: string, thumb: string, title: string, badge?: string) => (
    <button
      key={url}
      data-nav
      disabled={busy}
      aria-pressed={chosen[kind] === url}
      onClick={() => pick(url)}
      title={title}
    >
      <img src={thumb} alt="" loading="lazy" onError={() => setBroken((b) => new Set(b).add(url))} />
      {badge && <span className="art-badge">{badge}</span>}
      {chosen[kind] === url && (
        <span className="art-check" aria-hidden>
          <Icon name="check" size={14} />
        </span>
      )}
    </button>
  );

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
      <div className="art-bar">
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
        <span className="note">Picks apply right away. Change the {kind === "cover" ? "background" : "cover"} too, then press Done.</span>
      </div>
      {error && <p className="note error">{error}</p>}
      <div className={`art-grid ${kind}`}>
        {storeUrl && !broken.has(storeUrl) && tile(storeUrl, storeUrl, "The store's own artwork", "Store")}
        {community?.map((o) => tile(o.url, o.thumb, o.style || "Artwork"))}
      </div>
      {!error && options === null && <p className="note">Loading community artwork…</p>}
      {community?.length === 0 && (
        <p className="note">SteamGridDB has no other {kind === "cover" ? "covers" : "backgrounds"} for this game yet.</p>
      )}
      <div className="art-foot">
        <button className="link" data-nav disabled={busy} onClick={reset} title="Store art, and SteamGridDB's favourite when you have a key">
          Back to automatic artwork
        </button>
        <button className="btn btn-primary" data-nav onClick={onClose}>
          Done
        </button>
      </div>
    </div>
    </div>,
    document.body,
  );
}
