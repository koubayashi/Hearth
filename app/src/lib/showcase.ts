// ?showcase in the demo: the library with invented games and drawn artwork,
// for the README's screenshots, so they show no real studio's art or names.
// Every cover, hero and logo is an SVG drawn here from a seed.

type Motif = "peaks" | "hills" | "city" | "forest" | "waves" | "orbit" | "dunes";
type Face = "sans" | "serif" | "wide";

interface Look {
  title: string;
  /** Sky top, sky bottom, sun or planet, land. */
  colors: [string, string, string, string];
  motif: Motif;
  face: Face;
}

/** One per demo seed, in the same order: titles and how each is drawn. */
export const SHOWCASE: Look[] = [
  { title: "Emberfall", colors: ["#2a0f0a", "#c2401c", "#ffcf7a", "#1a0807"], motif: "peaks", face: "wide" },
  { title: "Apex Drift", colors: ["#0b1f3a", "#e46b3c", "#ffd29a", "#0a0f1c"], motif: "hills", face: "sans" },
  { title: "The Hollow Crown", colors: ["#1b1430", "#6b4a8f", "#e9d8a6", "#0e0a18"], motif: "forest", face: "serif" },
  { title: "Ashen Vow", colors: ["#1a1a1c", "#6e6a64", "#f2e6cf", "#0b0b0c"], motif: "peaks", face: "serif" },
  { title: "Meridian 2099", colors: ["#07172a", "#1f6f8b", "#f5a35c", "#050b14"], motif: "city", face: "wide" },
  { title: "Lanternlight", colors: ["#081a20", "#1d5a5e", "#f6e7b0", "#04100f"], motif: "forest", face: "serif" },
  { title: "Summit", colors: ["#2c5a8a", "#f2b8a2", "#fff4dc", "#22334f"], motif: "peaks", face: "wide" },
  { title: "Northwind", colors: ["#13202c", "#5d7a8c", "#dfe8ec", "#0b1219"], motif: "city", face: "sans" },
  { title: "Northwind Remastered", colors: ["#0c1b2e", "#3e7cb1", "#f0f6fa", "#060d17"], motif: "city", face: "sans" },
  { title: "Harvest Hollow", colors: ["#7fb7d9", "#f6d58e", "#fff7d6", "#3d6b35"], motif: "hills", face: "serif" },
  { title: "Dust & Iron", colors: ["#3a1d10", "#d9824b", "#ffe2b0", "#24120a"], motif: "dunes", face: "wide" },
  { title: "Salt & Sail", colors: ["#0d2a3f", "#3d8fb0", "#f9e9c0", "#071a28"], motif: "waves", face: "serif" },
  { title: "Ferryman", colors: ["#140b16", "#7a2335", "#f7b267", "#0a050b"], motif: "waves", face: "serif" },
  { title: "The Quiet Detective", colors: ["#16181f", "#4b4f63", "#e8c37a", "#0b0c10"], motif: "city", face: "serif" },
  { title: "Ronin's Edge", colors: ["#2b0d12", "#b3262f", "#f3e3d3", "#120507"], motif: "peaks", face: "wide" },
  { title: "Long Walk Home", colors: ["#3b4a52", "#a5b3ad", "#f4efe4", "#1d2529"], motif: "hills", face: "sans" },
  { title: "Signal Lost", colors: ["#0a0d14", "#28354d", "#9ec1d9", "#05070b"], motif: "orbit", face: "wide" },
  { title: "Orbit's End", colors: ["#100a24", "#45307a", "#f0a868", "#07041a"], motif: "orbit", face: "sans" },
  { title: "Vanguard Protocol", colors: ["#0e1a14", "#3c6b4f", "#d8f0c8", "#07100b"], motif: "dunes", face: "wide" },
  { title: "Doorways", colors: ["#e8e2d6", "#c9b8a0", "#e2553a", "#5a4a3c"], motif: "orbit", face: "sans" },
  { title: "The Last Witchling", colors: ["#1d1029", "#5b2d6e", "#c7e8a0", "#0d0714"], motif: "forest", face: "serif" },
  { title: "Lanternlight: Ascent", colors: ["#0b1430", "#2e5a8f", "#ffe9a8", "#060a1a"], motif: "peaks", face: "serif" },
  { title: "Blight Season", colors: ["#20190e", "#7a6a2f", "#e8d9a0", "#100c06"], motif: "forest", face: "serif" },
  { title: "Starward", colors: ["#05060f", "#1b2142", "#c9d6ff", "#020308"], motif: "orbit", face: "wide" },
  { title: "Deep Blue Diner", colors: ["#03263a", "#0d6f8f", "#ffd37a", "#011522"], motif: "waves", face: "sans" },
  { title: "Wildcard", colors: ["#13281f", "#2f6b4c", "#ff5d5d", "#0a1611"], motif: "orbit", face: "wide" },
  { title: "Puppet Throne", colors: ["#1c0f0c", "#6e3b24", "#f2c48d", "#0d0605"], motif: "city", face: "serif" },
  { title: "Echo Chamber", colors: ["#101418", "#3b5562", "#f2f2f2", "#080a0c"], motif: "waves", face: "wide" },
  { title: "Cells Below", colors: ["#1a0a0a", "#5e1f1f", "#f28c38", "#0c0404"], motif: "peaks", face: "sans" },
  { title: "Tower of Cards", colors: ["#1b1530", "#5d4a9a", "#ffd166", "#0d0a1a"], motif: "city", face: "wide" },
  { title: "Spirit Grove", colors: ["#06221d", "#1f7a62", "#d6ffe9", "#03120f"], motif: "forest", face: "serif" },
  { title: "Backbeat", colors: ["#25052f", "#d6336c", "#ffe066", "#12021a"], motif: "dunes", face: "wide" },
];

/** Seeded random numbers, so a game is drawn the same every time. */
function rng(seed: number) {
  let s = seed * 9301 + 49297;
  return () => {
    s = (s * 16807) % 2147483647;
    return (s % 100000) / 100000;
  };
}

function mix(a: string, b: string, t: number): string {
  const p = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  const [x, y] = [p(a), p(b)];
  return "#" + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, "0")).join("");
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/'/g, "&#39;");

/** Layers of land from far (light, high) to near (dark, low). */
function land(look: Look, w: number, h: number, r: () => number): string {
  const [, sky, , ground] = look.colors;
  const out: string[] = [];
  const layers = 4;
  for (let l = 0; l < layers; l++) {
    const fill = mix(sky, ground, 0.35 + (l / (layers - 1)) * 0.65);
    const base = h * (0.5 + l * 0.11);
    const amp = h * (0.16 - l * 0.025);
    let d = `M0 ${h} L0 ${base}`;
    if (look.motif === "peaks" || look.motif === "forest") {
      const step = look.motif === "forest" ? w / (18 + l * 8) : w / (5 + l * 3);
      for (let x = 0; x <= w + step; x += step) {
        const top = base - amp * (0.3 + r() * 0.9);
        d += ` L${x - step / 2} ${top} L${x} ${base - amp * r() * 0.3}`;
      }
    } else if (look.motif === "city") {
      let x = 0;
      while (x < w) {
        const bw = w * (0.03 + r() * 0.06);
        const top = base - amp * (0.4 + r() * 1.6);
        d += ` L${x} ${top} L${x + bw} ${top}`;
        x += bw;
      }
    } else {
      const waves = look.motif === "waves" ? 6 + l * 3 : 2 + l;
      const a = look.motif === "waves" ? amp * 0.25 : amp;
      for (let i = 0; i <= waves; i++) {
        const x = (i / waves) * w;
        const cx = x - w / waves / 2;
        d += ` Q${cx} ${base - a * (0.5 + r())} ${x} ${base - a * r() * 0.4}`;
      }
    }
    out.push(`<path d='${d} L${w} ${h} Z' fill='${fill}'/>`);
  }
  return out.join("");
}

function scene(look: Look, w: number, h: number, seed: number): string {
  const r = rng(seed);
  const [top, sky, sun] = look.colors;
  const big = look.motif === "orbit";
  const sx = w * (0.25 + r() * 0.5);
  const sy = h * (big ? 0.42 : 0.3 + r() * 0.12);
  const sr = Math.min(w, h) * (big ? 0.34 : 0.11 + r() * 0.06);
  const stars = Array.from({ length: big ? 70 : 24 }, () => `<circle cx='${(r() * w).toFixed(0)}' cy='${(r() * h * 0.55).toFixed(0)}' r='${(0.6 + r() * 1.4).toFixed(1)}' fill='#fff' opacity='${(0.2 + r() * 0.5).toFixed(2)}'/>`).join("");
  const rings = big
    ? `<ellipse cx='${sx}' cy='${sy}' rx='${sr * 1.7}' ry='${sr * 0.32}' fill='none' stroke='${sun}' stroke-opacity='.55' stroke-width='${Math.max(2, sr * 0.03)}' transform='rotate(-14 ${sx} ${sy})'/>`
    : "";
  return (
    `<defs><linearGradient id='s' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='${top}'/><stop offset='1' stop-color='${sky}'/></linearGradient>` +
    `<radialGradient id='g'><stop offset='0' stop-color='${sun}' stop-opacity='.55'/><stop offset='1' stop-color='${sun}' stop-opacity='0'/></radialGradient>` +
    `<filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 .07 0'/></filter></defs>` +
    `<rect width='${w}' height='${h}' fill='url(#s)'/>${stars}` +
    `<circle cx='${sx}' cy='${sy}' r='${sr * 2.6}' fill='url(#g)'/>` +
    `<circle cx='${sx}' cy='${sy}' r='${sr}' fill='${big ? mix(sun, top, 0.35) : sun}'/>${rings}` +
    (big ? land({ ...look, motif: "dunes" }, w, h * 1.25, r) : land(look, w, h, r)) +
    `<rect width='${w}' height='${h}' filter='url(#n)'/>`
  );
}

const FONTS: Record<Face, string> = {
  sans: "'Segoe UI Variable Display','Segoe UI',sans-serif",
  serif: "Georgia,'Times New Roman',serif",
  wide: "Bahnschrift,'Segoe UI',sans-serif",
};

/** The title set like a game's logo: one or two lines, fitted to `w`. */
function titleText(look: Look, x: number, y: number, w: number, anchor: "middle" | "start", maxH = Infinity, fill = "#fff"): string {
  const upper = look.face === "wide";
  const text = upper ? look.title.toUpperCase() : look.title;
  const words = text.split(" ");
  const lines = text.length > 13 && words.length > 1 ? [words.slice(0, Math.ceil(words.length / 2)).join(" "), words.slice(Math.ceil(words.length / 2)).join(" ")] : [text];
  const longest = Math.max(...lines.map((l) => l.length));
  // Average glyph width per font size, a little generous so nothing clips.
  const perChar = { wide: 0.68, serif: 0.62, sans: 0.58 }[look.face];
  const size = Math.min(w / (longest * perChar), w * 0.2, maxH / (lines.length * 1.05));
  const weight = look.face === "serif" ? 700 : 800;
  const spacing = upper ? size * 0.06 : look.face === "sans" ? -size * 0.02 : 0;
  const style = look.face === "serif" ? " font-style='italic'" : "";
  return lines
    .map((l, i) => `<text x='${x}' y='${y + i * size * 1.02 - (lines.length - 1) * size * 1.02}' font-family="${FONTS[look.face]}" font-size='${size.toFixed(1)}' font-weight='${weight}'${style} letter-spacing='${spacing.toFixed(1)}' text-anchor='${anchor}' fill='${fill}'>${esc(l)}</text>`)
    .join("");
}

const uri = (svg: string) => `data:image/svg+xml,${encodeURIComponent(svg)}`;

export function showcaseArt(i: number): { title: string; cover: string; hero: string; logo: string } {
  const look = SHOWCASE[i % SHOWCASE.length];
  const shade = `<linearGradient id='b' x1='0' y1='0' x2='0' y2='1'><stop offset='.55' stop-color='#000' stop-opacity='0'/><stop offset='1' stop-color='#000' stop-opacity='.65'/></linearGradient>`;
  const cover =
    `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 600 900'>${scene(look, 600, 900, i + 1)}` +
    `<defs>${shade}</defs><rect width='600' height='900' fill='url(#b)'/>${titleText(look, 300, 820, 500, "middle")}</svg>`;
  const hero = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1920 620' preserveAspectRatio='xMidYMid slice'>${scene(look, 1920, 620, i + 101)}</svg>`;
  const logo = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 900 260'>${titleText(look, 0, 220, 880, "start", 230)}</svg>`;
  return { title: look.title, cover: uri(cover), hero: uri(hero), logo: uri(logo) };
}
