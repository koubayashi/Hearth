import { useState } from "react";

// Older Steam titles only exist on the legacy CDN path; try it before giving up.
export const legacy = (src: string) =>
  src.replace(
    "shared.cloudflare.steamstatic.com/store_item_assets/steam/apps",
    "cdn.cloudflare.steamstatic.com/steam/apps",
  );

export function Art({ src, title, className }: { src: string | null; title: string; className?: string }) {
  const [attempt, setAttempt] = useState(0);
  const urls = src ? [...new Set([src, legacy(src)])] : [];
  if (attempt >= urls.length) {
    return <div className={`art-fallback ${className ?? ""}`}>{title}</div>;
  }
  return (
    <img
      className={className}
      src={urls[attempt]}
      alt=""
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => setAttempt((a) => a + 1)}
    />
  );
}
