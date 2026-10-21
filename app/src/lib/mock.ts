import type { Accounts, Game, GameDetails, InstallInfo, LibraryQuery, NowPlaying, StoreId } from "./types";

const noop = async (_cb: (payload: unknown) => void) => () => {};

// Synthetic demo library for designing in a plain browser. Artwork comes from
// Steam's public CDN; replace with the metadata service once it exists.
const art = (appid: number, kind: "library_600x900" | "library_hero" | "logo") =>
  `https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/${appid}/${kind}.${
    kind === "logo" ? "png" : "jpg"
  }`;

const H = 3600;
const DAY = 86400;
const now = Math.floor(Date.now() / 1000);

type Seed = [title: string, appid: number, store: StoreId, installed: boolean, hours: number, daysAgo: number | null];

const seeds: Seed[] = [
  ["Hades II", 1145350, "steam", true, 41.2, 0.1],
  ["Forza Horizon 5", 1551360, "xbox", true, 112.7, 1],
  ["Baldur's Gate 3", 1086940, "gog", true, 203.1, 3],
  ["Elden Ring", 1245620, "steam", true, 88.4, 9],
  ["Cyberpunk 2077", 1091500, "gog", false, 64.0, 60],
  ["Hollow Knight", 367520, "steam", true, 31.5, 21],
  ["Celeste", 504230, "epic", false, 0, null],
  ["Stardew Valley", 413150, "steam", true, 146.3, 14],
  ["Red Dead Redemption 2", 1174180, "epic", false, 12.8, 200],
  ["Sea of Thieves", 1172620, "xbox", true, 19.4, 30],
  ["Hades", 1145360, "epic", false, 58.0, 400],
  ["Disco Elysium", 632470, "gog", true, 27.9, 45],
  ["Sekiro: Shadows Die Twice", 814380, "steam", false, 6.2, 500],
  ["Death Stranding", 1190460, "epic", false, 0, null],
  ["Control", 870780, "epic", true, 14.1, 90],
  ["Outer Wilds", 753640, "steam", true, 22.6, 120],
  ["Halo Infinite", 1240440, "xbox", false, 9.3, 300],
  ["Portal 2", 620, "steam", true, 18.0, 700],
  ["The Witcher 3: Wild Hunt", 292030, "gog", false, 97.5, 800],
  ["Hollow Knight: Silksong", 1030300, "steam", true, 4.4, 2],
  ["A Plague Tale: Requiem", 1182900, "epic", false, 0, null],
  ["Starfield", 1716740, "xbox", false, 3.1, 365],
  ["Dave the Diver", 1868140, "steam", true, 11.0, 50],
  ["Balatro", 2379780, "steam", true, 72.9, 5],
  ["Lies of P", 1627720, "xbox", false, 0, null],
  ["Returnal", 1649240, "ubisoft", false, 2.0, 250],
  ["Dead Cells", 588650, "manual", false, 15.7, 600],
  ["Slay the Spire", 646570, "ea", false, 0, null],
  ["Ori and the Will of the Wisps", 1057090, "xbox", true, 13.3, 150],
  ["Hi-Fi Rush", 1817230, "xbox", false, 0, null],
];

const games: Game[] = seeds.map(([title, appid, store, installed, hours, daysAgo], i) => ({
  id: i + 1,
  title,
  favorite: i % 7 === 0,
  hidden: false,
  installed,
  added_at: now - (i + 1) * 9 * DAY,
  last_played: daysAgo === null ? null : Math.floor(now - daysAgo * DAY),
  playtime_secs: Math.round(hours * H),
  cover: art(appid, "library_600x900"),
  hero: art(appid, "library_hero"),
  logo: art(appid, "logo"),
  description: null,
  entries: [
    ...(title === "Hollow Knight"
      ? [{ id: 1000, game_id: i + 1, store: "gog" as StoreId, store_game_id: "hk", installed: false, install_dir: null, hidden: false, store_playtime_secs: null, store_baseline_secs: null }]
      : []),
    {
      id: i + 1,
      game_id: i + 1,
      store,
      store_game_id: String(appid),
      installed,
      install_dir: installed ? `C:\\Games\\${title}` : null,
      hidden: false,
      store_playtime_secs: null,
      store_baseline_secs: null,
    },
  ],
}));

const norm = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

export const mockBackend = {
  demo: true,
  async launch(_id: number) {},
  accounts: async (): Promise<Accounts> => ({ epic: null, steam: "Demo player", steam_auth: null }),
  async steamSetApiKey(_key: string): Promise<number> {
    throw new Error("Demo library: add your key in the desktop app");
  },
  async steamSignIn() {},
  async steamSignOut() {},
  async epicOpenBrowser() {},
  async epicSubmitCode(_code: string) {},
  async openSteamKeyPage() {},
  async syncNow() {},
  async epicSignIn() {},
  async epicSignOut() {},
  nowPlaying: async (): Promise<NowPlaying[]> => [],
  // Synthetic details so the panel can be designed without the store API.
  async gameDetails(id: number): Promise<GameDetails | null> {
    const g = games.find((g) => g.id === id);
    if (!g) return null;
    return {
      steam_app_id: Number(g.entries[0]?.store_game_id ?? 0),
      release_date: "Sep 25, 2025",
      developers: ["Supergiant Games"],
      publishers: ["Supergiant Games"],
      genres: ["Action", "Indie", "RPG"],
      features: ["Single-player", "Full controller support", "Cloud saves", "Achievements"],
      metacritic: 70 + (id % 25),
      metacritic_url: null,
      recommendations: 120000,
    };
  },
  async installInfo(id: number, _refresh: boolean): Promise<InstallInfo | null> {
    const g = games.find((g) => g.id === id);
    const dir = g?.entries.find((e) => e.installed)?.install_dir;
    return dir ? { path: dir, bytes: 1_000_000_000 * (8 + (id % 90)) } : null;
  },
  async openInstallFolder(_id: number) {},
  async openStorePage(_id: number) {},
  onNowPlaying: noop,
  onLibraryChanged: noop,
  onAccountsChanged: noop,
  onSyncStarted: noop,
  onSyncError: noop,
  async listGames(q: LibraryQuery): Promise<Game[]> {
    const search = q.search ? norm(q.search) : "";
    return games
      .filter((g) => g.hidden === q.hidden_only)
      .filter((g) =>
        q.install === "all" ? true : q.install === "installed" ? g.installed : !g.installed,
      )
      .filter((g) => q.stores.length === 0 || g.entries.some((e) => q.stores.includes(e.store)))
      .filter((g) => !search || norm(g.title).includes(search))
      .sort((a, b) => a.title.localeCompare(b.title))
      // Copies, like the real IPC boundary, so UI state never aliases the store.
      .map((g) => structuredClone(g));
  },
  async getGame(id: number) {
    const g = games.find((g) => g.id === id);
    return g ? structuredClone(g) : null;
  },
  async setHidden(id: number, hidden: boolean) {
    const g = games.find((g) => g.id === id);
    if (g) g.hidden = hidden;
  },
  async setFavorite(id: number, favorite: boolean) {
    const g = games.find((g) => g.id === id);
    if (g) g.favorite = favorite;
  },
};
