import type { LaunchOption, LaunchOptionInput, Via } from "./types";
import type { Accounts, ArtKind, ArtOption, Collection, Game, GameStatus, GameDetails, InstallInfo, LibraryQuery, NowPlaying, PlaySession, SmartIndex, SmartRole, StoreHit, StoreId } from "./types";

// ?sgdb in the URL shows Settings as if a key were saved, to design that state.
const demoSgdb = typeof location !== "undefined" && location.search.includes("sgdb");
// ?signedin shows the stores as signed in.
const demoSignedIn = typeof location !== "undefined" && location.search.includes("signedin");

const noop = async (_cb: (payload: unknown) => void) => () => {};
// The demo's own "library changed" event, so edits show up everywhere.
const changeListeners = new Set<(payload: unknown) => void>();
const libraryChanged = () => changeListeners.forEach((cb) => cb(null));

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

const demoEntities: Record<string, string> = {
  Q1: "Hades", Q2: "Hollow Knight", Q3: "Forza Horizon", Q4: "Forza", Q5: "Halo", Q6: "Ori",
  Q10: "Supergiant Games", Q11: "Team Cherry", Q12: "Playground Games", Q13: "343 Industries",
  Q14: "Rare", Q15: "Moon Studios", Q16: "Xbox Game Studios", Q17: "Microsoft", Q18: "Remedy Entertainment",
  Q19: "Bethesda Game Studios", Q20: "Bethesda Softworks", Q21: "Tango Gameworks",
};
const demoParents: [string, string][] = [
  ["Q3", "Q4"], ["Q12", "Q16"], ["Q13", "Q16"], ["Q14", "Q16"], ["Q16", "Q17"], ["Q19", "Q20"], ["Q21", "Q20"], ["Q20", "Q17"],
];
const demoLinks: [string, SmartRole, string][] = [
  ["Hades", "series", "Q1"], ["Hades II", "series", "Q1"], ["Hades", "developer", "Q10"], ["Hades II", "developer", "Q10"],
  ["Hades", "publisher", "Q10"], ["Hollow Knight", "series", "Q2"], ["Hollow Knight: Silksong", "series", "Q2"],
  ["Hollow Knight", "developer", "Q11"], ["Hollow Knight: Silksong", "developer", "Q11"],
  ["Forza Horizon 5", "series", "Q3"], ["Forza Horizon 5", "developer", "Q12"], ["Forza Horizon 5", "publisher", "Q16"],
  ["Halo Infinite", "series", "Q5"], ["Halo Infinite", "developer", "Q13"], ["Halo Infinite", "publisher", "Q16"],
  ["Sea of Thieves", "developer", "Q14"], ["Sea of Thieves", "publisher", "Q16"],
  ["Ori and the Will of the Wisps", "series", "Q6"], ["Ori and the Will of the Wisps", "developer", "Q15"],
  ["Ori and the Will of the Wisps", "publisher", "Q16"], ["Control", "developer", "Q18"],
  ["Starfield", "developer", "Q19"], ["Starfield", "publisher", "Q20"], ["Hi-Fi Rush", "developer", "Q21"], ["Hi-Fi Rush", "publisher", "Q20"],
];
const smartEdits: SmartIndex["links"] = [];
const smartRemoved = new Set<string>();
const removedSessions = new Set<number>();

// ?games=2000 repeats the seeds (numbered) to test a large library.
const demoCount = typeof location !== "undefined" ? Number(new URLSearchParams(location.search).get("games")) || 0 : 0;
const allSeeds: Seed[] =
  demoCount > seeds.length
    ? Array.from({ length: demoCount }, (_, i) => {
        const [title, ...rest] = seeds[i % seeds.length];
        const round = Math.floor(i / seeds.length);
        return [round ? `${title} ${round + 1}` : title, ...rest];
      })
    : seeds;

const games: Game[] = allSeeds.map(([title, appid, store, installed, hours, daysAgo], i) => ({
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
  status: (["playing", "backlog", null, "completed", null, "backlog"] as (GameStatus | null)[])[i % 6],
  collections: i % 4 === 0 ? [1] : [],
  // One demo game lives on an unplugged external drive.
  offline_drive: title === "Dead Cells" ? "E:" : null,
  launch_options: [] as LaunchOption[],
  entries: [
    ...(title === "Hollow Knight"
      ? [{ id: 1000, game_id: i + 1, store: "gog" as StoreId, store_game_id: "hk", installed: false, install_dir: null, hidden: false, store_playtime_secs: null, store_baseline_secs: null, launch_exe: null, offline: false }]
      : []),
    {
      id: i + 1,
      game_id: i + 1,
      store,
      store_game_id: String(appid),
      installed,
      install_dir: title === "Dead Cells" ? "E:\\Games\\Dead Cells" : installed ? `${i % 3 === 0 ? "D" : "C"}:\\Games\\${title}` : null,
      hidden: false,
      store_playtime_secs: null,
      store_baseline_secs: null,
      launch_exe: null,
      offline: title === "Dead Cells",
    },
  ],
}));

const collections: Collection[] = [{ id: 1, name: "Co-op night", count: 0 }];

const norm = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

export const mockBackend = {
  demo: true,
  async launch(_id: number, _via?: Via) {},
  async saveLaunchOption(o: LaunchOptionInput): Promise<number> {
    const game = games.find((g) => g.id === o.game_id);
    if (!game) throw new Error("That game is no longer in your library");
    if (!o.name.trim()) throw new Error("Give it a name, like \u201cOffline server\u201d.");
    if (!o.path.trim()) throw new Error("That program can't be found. Choose it again with Browse.");
    const existing = game.launch_options.find((x) => x.id === o.id);
    const id = existing?.id ?? Date.now();
    const next = { id, game_id: game.id, name: o.name.trim(), path: o.path.trim(), args: o.args.trim(), working_dir: o.working_dir?.trim() || null, is_default: existing?.is_default ?? false };
    game.launch_options = existing ? game.launch_options.map((x) => (x.id === id ? next : x)) : [...game.launch_options, next];
    libraryChanged();
    return id;
  },
  async deleteLaunchOption(id: number) {
    for (const g of games) g.launch_options = g.launch_options.filter((x) => x.id !== id);
    libraryChanged();
  },
  async setDefaultLaunch(gameId: number, option: number | null) {
    const game = games.find((g) => g.id === gameId);
    if (game) game.launch_options = game.launch_options.map((x) => ({ ...x, is_default: x.id === option }));
    libraryChanged();
  },
  async setFolderExe(gameId: number, path: string | null) {
    const entry = games.find((g) => g.id === gameId)?.entries.find((e) => e.store === "manual" && e.installed);
    if (entry) entry.launch_exe = path ? (path.split(/[\\/]/).pop() ?? null) : null;
    libraryChanged();
  },
  accounts: async (): Promise<Accounts> => ({
    epic: demoSignedIn ? "DemoPlayer" : null,
    sgdb: demoSgdb,
    steam: "Demo player",
    steam_auth: demoSignedIn ? "session" : null,
    xbox: demoSignedIn ? "DemoGamertag" : null,
    gog: null,
  }),
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
  async xboxSignIn() {},
  async xboxSignOut() {},
  async gogSignIn() {},
  async gogSignOut() {},
  nowPlaying: async (): Promise<NowPlaying[]> => [],
  // A few months of evening sessions on a handful of games, deterministic.
  async playSessions(): Promise<PlaySession[]> {
    const out: PlaySession[] = [];
    const now = Math.floor(Date.now() / 1000);
    for (let day = 150; day >= 0; day--) {
      const r = (day * 7919) % 97;
      if (r % 3 === 0 || (day > 60 && day < 80)) continue;
      const game_id = [1, 1, 2, 3, 5, 1, 8][r % 7];
      const started_at = now - day * 86400 - 3 * H;
      out.push({ id: day, game_id, started_at, ended_at: started_at + 1200 + (r % 13) * 900, source: "tracker" });
    }
    return out.filter((s) => !removedSessions.has(s.id));
  },
  async deletePlaySession(id: number) {
    removedSessions.add(id);
    libraryChanged();
  },
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
      summary: "Defy the god of the dead as you hack and slash out of the Underworld in this rogue-like dungeon crawler from the creators of Bastion and Transistor.",
    };
  },
  async installInfo(id: number, _refresh: boolean): Promise<InstallInfo | null> {
    const g = games.find((g) => g.id === id);
    const dir = (g?.entries.find((e) => e.installed) ?? g?.entries.find((e) => e.offline))?.install_dir;
    return dir ? { path: dir, bytes: 1_000_000_000 * (8 + (id % 90)) } : null;
  },
  async savedGameInfo() {
    return Promise.all(
      // A few sizes start unknown, so the demo shows Storage measuring them.
      games.map(async (g) => {
        const install = await this.installInfo(g.id, false);
        return { id: g.id, details: await this.gameDetails(g.id), install: install && g.id % 5 === 2 ? { ...install, bytes: null } : install };
      }),
    );
  },
  async storageDrives(_roots: string[]) {
    return [
      { root: "C:", label: "Windows", total: 1_000_000_000_000, free: 212_000_000_000 },
      { root: "D:", label: "Games", total: 2_000_000_000_000, free: 1_140_000_000_000 },
      { root: "F:", label: "Backup", total: 4_000_000_000_000, free: 3_100_000_000_000 },
    ];
  },
  async openInstallFolder(_id: number) {},
  async openStorePage(_id: number) {},
  async setStatus(id: number, status: GameStatus | null) {
    const g = games.find((g) => g.id === id);
    if (g) g.status = status;
  },
  async listCollections(): Promise<Collection[]> {
    return collections.map((c) => ({ ...c, count: games.filter((g) => g.collections.includes(c.id)).length }));
  },
  async createCollection(name: string): Promise<number> {
    const id = Math.max(0, ...collections.map((c) => c.id)) + 1;
    collections.push({ id, name: name.trim(), count: 0 });
    return id;
  },
  async renameCollection(id: number, name: string) {
    const c = collections.find((c) => c.id === id);
    if (c) c.name = name.trim();
  },
  async deleteCollection(id: number) {
    collections.splice(collections.findIndex((c) => c.id === id), 1);
    games.forEach((g) => (g.collections = g.collections.filter((c) => c !== id)));
  },
  async setInCollection(collection: number, game: number, member: boolean) {
    const g = games.find((g) => g.id === game);
    if (!g) return;
    g.collections = g.collections.filter((c) => c !== collection);
    if (member) g.collections.push(collection);
  },
  async sgdbSetKey(_key: string) {
    throw new Error("Demo library: add your SteamGridDB key in the desktop app");
  },
  async sgdbClearKey() {},
  async sgdbCopyKey() {},
  async autoBackupStatus(): Promise<{ folder: string | null; last_at: number | null; error: string | null }> {
    return { folder: null, last_at: null, error: null };
  },
  async autoBackupSet(_folder: string | null): Promise<{ folder: string | null; last_at: number | null; error: string | null }> {
    throw new Error("Demo library: automatic backups work in the desktop app");
  },
  async savePrefsSnapshot(_prefs: string) {},
  async reportError(message: string) {
    console.error(message);
  },
  async openErrorLog() {},
  async backupExport(_path: string, _prefs: string): Promise<void> {
    throw new Error("Demo library: backups work in the desktop app");
  },
  async backupRestore(_path: string): Promise<{ prefs: string | null; created_at: number | null; ways: { name: string; path: string; args: string }[] }> {
    throw new Error("Demo library: backups work in the desktop app");
  },
  async restoreWays(_keep: boolean) {},
  async openSgdbKeyPage() {},
  // Demo choices reuse the library's own art so the picker can be designed.
  async artworkOptions(_id: number, kind: ArtKind): Promise<ArtOption[]> {
    // As many as SteamGridDB returns for a popular game, to test long lists.
    return [...games, ...games, ...games].slice(0, 64).map((g, i) => {
      const url = (kind === "cover" ? g.cover : g.hero) ?? "";
      return { url: `${url}#${i}`, thumb: url, score: 64 - i, style: "alternate" };
    });
  },
  async setArtwork(id: number, kind: ArtKind, url: string) {
    const g = games.find((g) => g.id === id);
    if (g && kind === "cover") g.cover = url;
    if (g && kind === "hero") g.hero = url;
  },
  async resetArtwork(_id: number) {},
  // A small, hand-made slice of what Wikidata gives, to design the shelves with.
  async smartIndex(): Promise<SmartIndex> {
    const id = (title: string) => games.find((g) => g.title === title)?.id ?? 0;
    const entities = Object.entries(demoEntities).map(([id, label]) => ({ id, label }));
    const links: SmartIndex["links"] = demoLinks
      .map(([title, role, entity]): SmartIndex["links"][number] => ({ game_id: id(title), role, entity, by_user: false }))
      .concat(smartEdits)
      .filter((l) => l.game_id > 0 && !smartRemoved.has(`${l.game_id} ${l.role} ${l.entity}`));
    return { entities, parents: demoParents.map(([child, parent]) => ({ child, parent })), links };
  },
  async setGameLink(gameId: number, role: SmartRole, entity: string, on: boolean) {
    const key = `${gameId} ${role} ${entity}`;
    if (on) {
      smartRemoved.delete(key);
      smartEdits.push({ game_id: gameId, role, entity, by_user: true });
    } else {
      smartRemoved.add(key);
    }
  },
  async setEntityParent(child: string, parent: string, on: boolean) {
    const at = demoParents.findIndex(([c, p]) => c === child && p === parent);
    if (on && at < 0) demoParents.push([child, parent]);
    if (!on && at >= 0) demoParents.splice(at, 1);
  },
  async createSeries(gameId: number, name: string) {
    const id = `user:${Object.keys(demoEntities).length}`;
    demoEntities[id] = name;
    smartEdits.push({ game_id: gameId, role: "series", entity: id, by_user: true });
    return id;
  },
  async storeArtwork(id: number) {
    const g = games.find((g) => g.id === id);
    return { cover: g?.cover ?? null, hero: g?.hero ?? null };
  },
  async searchStore(term: string): Promise<StoreHit[]> {
    return games
      .filter((g) => g.title.toLowerCase().includes(term.toLowerCase().slice(0, 4)))
      .map((g) => ({ steam_app_id: Number(g.entries[0]?.store_game_id ?? 0), name: g.title }));
  },
  async setStoreMatch(_id: number, _steamAppId: number): Promise<GameDetails | null> {
    return null;
  },
  async gameFolders(): Promise<string[]> {
    return ["D:\\Games"];
  },
  async addGameFolder(path: string): Promise<string[]> {
    return ["D:\\Games", path];
  },
  async removeGameFolder(_path: string): Promise<string[]> {
    return [];
  },
  onNowPlaying: noop,
  onLibraryChanged: async (cb: (payload: unknown) => void) => {
    // Each subscription is its own entry, as with Tauri's events: React's dev
    // mode subscribes the same handler twice and unsubscribes one.
    const entry = (p: unknown) => cb(p);
    changeListeners.add(entry);
    return () => void changeListeners.delete(entry);
  },
  onAccountsChanged: noop,
  onSyncStarted: noop,
  onSmartStatus: noop,
  onSmartChanged: noop,
  onAutoBackup: noop,
  onSyncError: noop,
  async updateReady(): Promise<string | null> {
    return null;
  },
  async installUpdate() {},
  onUpdateReady: noop,
  async listGames(q: LibraryQuery): Promise<Game[]> {
    const search = q.search ? norm(q.search) : "";
    return games
      .filter((g) => g.hidden === q.hidden_only)
      .filter((g) =>
        q.install === "all" ? true : q.install === "installed" ? g.installed : !g.installed,
      )
      .filter((g) => q.stores.length === 0 || g.entries.some((e) => q.stores.includes(e.store)))
      .filter((g) => !search || norm(g.title).includes(search))
      .filter((g) => !q.status || g.status === q.status)
      .filter((g) => q.collection == null || g.collections.includes(q.collection))
      .sort((a, b) => a.title.localeCompare(b.title))
      // Copies, like the real IPC boundary, so UI state never aliases the store.
      .map((g) => structuredClone(g));
  },
  async gameCounts() {
    return { library: games.filter((g) => !g.hidden).length, hidden: games.filter((g) => g.hidden).length };
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
