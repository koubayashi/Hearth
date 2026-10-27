import { useState } from "react";

// Older Steam titles only exist on the legacy CDN path; try it before giving up.
export const legacy = (src: string) =>
  src.replace(
    "shared.cloudflare.steamstatic.com/store_item_assets/steam/apps",
    "cdn.cloudflare.steamstatic.com/steam/apps",
  );

/** Art that has to load fades in over its frame's plain background; art
 *  already in the cache (scrolling back, switching views) shows at once. */
const fadeInIfLoading = (img: HTMLImageElement | null) => {
  if (img && !img.complete) img.dataset.loading = "";
};

export function Art({ src, title, className }: { src: string | null; title: string; className?: string }) {
  const [attempt, setAttempt] = useState(0);
  const urls = src ? [...new Set([src, legacy(src)])] : [];
  if (attempt >= urls.length) {
    return <div className={`art-fallback ${className ?? ""}`}>{title}</div>;
  }
  return (
    <img
      ref={fadeInIfLoading}
      className={className}
      src={urls[attempt]}
      alt=""
      loading="lazy"
      decoding="async"
      draggable={false}
      onLoad={(e) => {
        const img = e.currentTarget;
        if ("loading" in img.dataset) img.classList.add("art-in");
      }}
      onError={() => setAttempt((a) => a + 1)}
    />
  );
}
