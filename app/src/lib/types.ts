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
}

export type InstallFilter = "all" | "installed" | "notinstalled";

export interface LibraryQuery {
  install: InstallFilter;
  stores: StoreId[];
  search: string | null;
  hidden_only: boolean;
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
  /** Steam account signed in on this PC. */
  steam: string | null;
  /** How Hearth reads the Steam library, or null for installed games only. */
  steam_auth: "session" | "api_key" | null;
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
