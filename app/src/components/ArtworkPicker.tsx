import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../lib/api";
import type { ArtKind, ArtOption, Game } from "../lib/types";
import { legacy } from "./Art";
import { Icon } from "./Icon";

type Kind = "cover" | "hero";
type Pair = Record<Kind, string | null>;

const TABS: { kind: Kind; label: string }[] = [
  { kind: "cover", label: "Cover" },
  { kind: "hero", label: "Background" },
];

/** One picture under two addresses (Steam's two hosts, a query or #fragment) is still one picture. */
const samePicture = (a: string | null, b: string | null) =>
  !!a && !!b && legacy(a).split(/[?#]/)[0] === legacy(b).split(/[?#]/)[0];

/**
 * Choose a game's cover and background: the current ones first, then the
 * store's own, then SteamGridDB's community art. Picks show in the preview
 * at the top and are saved together with Save; Cancel (or Esc) leaves the
 * game as it was.
 */
export function ArtworkPicker({ game, onDone, onClose }: { game: Game; onDone: () => void; onClose: () => void }) {
  const [kind, setKind] = useState<Kind>("cover");
  const [options, setOptions] = useState<ArtOption[] | null>(null);
  const [store, setStore] = useState<Pair>({ cover: null, hero: null });
  // Saved art, and what's picked here; only the difference is saved.
  const current: Pair = { cover: game.cover, hero: game.hero };
  const [chosen, setChosen] = useState<Pair>(current);
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

  const changed = (["cover", "hero"] as Kind[]).filter((k) => chosen[k] && !samePicture(chosen[k], current[k]));

  const save = async () => {
    if (changed.length === 0) return onClose();
    setBusy(true);
    setError(null);
    try {
      for (const k of changed) await api.setArtwork(game.id, k, chosen[k]!);
      onDone();
      onClose();
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
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  // The current picture leads; the store's follows unless it's the same
  // one; SteamGridDB's come after, without repeats.
  const saved = current[kind];
  const storeUrl = store[kind] && !samePicture(store[kind], saved) ? store[kind] : null;
  const community = options?.filter((o) => !broken.has(o.url) && !samePicture(o.url, saved) && !samePicture(o.url, storeUrl)) ?? null;
  const tile = (url: string, thumb: string, title: string, badge?: string) => {
    const on = samePicture(chosen[kind], url);
    return (
      <button key={url} data-nav disabled={busy} aria-pressed={on} onClick={() => setChosen((c) => ({ ...c, [kind]: url }))} title={title}>
        <img src={thumb} alt="" loading="lazy" onError={() => setBroken((b) => new Set(b).add(url))} />
        {badge && <span className="art-badge">{badge}</span>}
        {on && (
          <span className="art-check" aria-hidden>
            <Icon name="check" size={14} />
          </span>
        )}
      </button>
    );
  };

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
          <button className="icon-btn small" data-nav aria-label="Cancel" onClick={onClose}>
            <Icon name="close" size={14} />
          </button>
        </div>

        {/* What the game will look like: the background with the cover over
            it, as in its panel. Each half opens its tab. */}
        <div className="art-preview">
          <button className="art-preview-hero" data-nav aria-pressed={kind === "hero"} onClick={() => setKind("hero")} title="Change the background">
            {chosen.hero ? <img src={chosen.hero} alt="" /> : <span className="art-none">No background</span>}
          </button>
          <button className="art-preview-cover" data-nav aria-pressed={kind === "cover"} onClick={() => setKind("cover")} title="Change the cover">
            {chosen.cover ? <img src={chosen.cover} alt="" /> : <span className="art-none">No cover</span>}
          </button>
          {changed.length > 0 && <span className="art-preview-note">Not saved yet</span>}
        </div>

        <div className="art-bar">
          <div className="seg art-tabs" role="tablist">
            {TABS.map((t) => (
              <button key={t.kind} role="tab" data-nav aria-selected={kind === t.kind} aria-pressed={kind === t.kind} onClick={() => setKind(t.kind)}>
                {t.label}
                {changed.includes(t.kind) && <span className="art-dot" aria-label=" (changed)" />}
              </button>
            ))}
          </div>
          <span className="note">Pick a {kind === "cover" ? "cover" : "background"}; nothing changes until you press Save.</span>
        </div>
        {error && <p className="note error">{error}</p>}
        <div className={`art-grid ${kind}`}>
          {saved && !broken.has(saved) && tile(saved, saved, `The ${kind === "cover" ? "cover" : "background"} it has now`, "Current")}
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
          <span className="art-foot-actions">
            <button className="btn" data-nav onClick={onClose}>
              Cancel
            </button>
            <button className="btn btn-primary" data-nav disabled={busy} onClick={() => void save()}>
              {changed.length === 0 ? "Done" : "Save"}
            </button>
          </span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
