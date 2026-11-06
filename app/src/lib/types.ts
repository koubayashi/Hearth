// Mirrors hub-core's model.rs; keep the two in sync.

export type StoreId = "steam" | "epic" | "xbox" | "gog" | "ea" | "ubisoft" | "manual";

export interface StoreEntry {
  id: number;
  game_id: number;
  store: StoreId;
  store_game_id: string;
  installed: boolean;
  install_dir: string | null;
  hidden: boolean;
  store_playtime_secs: number | null;
  store_baseline_secs: number | null;
  /** Folder games: the program Play starts instead of the scan's choice, relative to install_dir. */
  launch_exe: string | null;
  /** Its install folder is on a drive that isn't connected right now. */
  offline: boolean;
  /** The store's own name for it: a version's name once games are combined. */
  title?: string | null;
  /** When the store last saw it played, if it says. */
  last_played?: number | null;
}

export interface Game {
  id: number;
  title: string;
  favorite: boolean;
  hidden: boolean;
  installed: boolean;
  added_at: number;
  last_played: number | null;
  playtime_secs: number;
  cover: string | null;
  hero: string | null;
  logo: string | null;
  description: string | null;
  entries: StoreEntry[];
  status: GameStatus | null;
  /** Ids of the collections this game is in. */
  collections: number[];
  /** "E:" when the game's only install is on a drive that isn't connected. */
  offline_drive: string | null;
  /** Ways to play the user added, besides the store's own launch. */
  launch_options: LaunchOption[];
}

/** A program the user added to play a game another way (a script, a mod loader, another exe). */
export interface LaunchOption {
  id: number;
  game_id: number;
  name: string;
  path: string;
  /** Passed exactly as typed, like a shortcut's arguments. */
  args: string;
  working_dir: string | null;
  /** The Play button uses this instead of the store. */
  is_default: boolean;
  /** Its program isn't on disk any more (a deleted game folder took it). */
  missing?: boolean;
}

/** How to start a game when the user picks: the store, or one of its ways to play. */
export type Via = { kind: "store" } | { kind: "option"; id: number } | { kind: "entry"; id: number };

export interface LaunchOptionInput {
  id: number | null;
  game_id: number;
  name: string;
  path: string;
  args: string;
  working_dir: string | null;
}

export type GameStatus = "playing" | "backlog" | "completed" | "abandoned";

export const STATUSES: { id: GameStatus; label: string }[] = [
  { id: "playing", label: "Playing" },
  { id: "backlog", label: "Backlog" },
  { id: "completed", label: "Completed" },
  { id: "abandoned", label: "Abandoned" },
];

export interface Collection {
  id: number;
  name: string;
  count: number;
}

export type InstallFilter = "all" | "installed" | "notinstalled";

export interface LibraryQuery {
  install: InstallFilter;
  stores: StoreId[];
  search: string | null;
  hidden_only: boolean;
  status?: GameStatus | null;
  collection?: number | null;
}

export const STORE_LABEL: Record<StoreId, string> = {
  steam: "Steam",
  epic: "Epic",
  xbox: "Xbox",
  gog: "GOG",
  ea: "EA",
  ubisoft: "Ubisoft",
  manual: "Manual",
};

export interface Accounts {
  /** Display name when signed in to Epic. */
  epic: string | null;
  /** A SteamGridDB API key is saved. */
  sgdb: boolean;
  /** Steam account signed in on this PC. */
  steam: string | null;
  /** How Hearth reads the Steam library, or null for installed games only. */
  steam_auth: "session" | "api_key" | null;
  /** Gamertag when signed in to Xbox. */
  xbox: string | null;
  /** Username when signed in to GOG. */
  gog: string | null;
}

export interface NowPlaying {
  game_id: number;
  title: string;
  /** Unix seconds. */
  started_at: number;
}

/** Store-page details, from Steam's public store API. */
export interface GameDetails {
  steam_app_id: number;
  release_date: string | null;
  developers: string[];
  publishers: string[];
  genres: string[];
  features: string[];
  metacritic: number | null;
  metacritic_url: string | null;
  recommendations: number | null;
  /** Steam's one-paragraph description, as plain text. */
  summary?: string | null;
}

export interface InstallInfo {
  path: string;
  /** Folder size; null while unknown. */
  bytes: number | null;
}

export interface StoreHit {
  steam_app_id: number;
  name: string;
}

export type ArtKind = "cover" | "hero" | "logo";

/** A SteamGridDB picture to choose from. */
export interface ArtOption {
  url: string;
  thumb: string;
  score: number;
  style: string;
}

/** One recorded play session (unix seconds). */
export interface PlaySession {
  id: number;
  game_id: number;
  started_at: number;
  ended_at: number;
  source: "hub" | "tracker";
}

/** What games can be browsed by. Franchises count as series: they're series' parents. */
export type SmartRole = "series" | "developer" | "publisher";

/** Series, companies, "part of" steps and game links, from Wikidata and the user's fixes. */
export interface SmartIndex {
  entities: { id: string; label: string }[];
  parents: { child: string; parent: string }[];
  /** "franchise" comes from Wikidata only, and shows as a series. */
  links: { game_id: number; role: SmartRole | "franchise"; entity: string; by_user: boolean }[];
}
