import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Accounts, ArtKind, ArtOption, Game, GameDetails, InstallInfo, LibraryQuery, NowPlaying, StoreHit } from "./types";
import { mockBackend } from "./mock";

const on = (event: string) => (cb: (payload: unknown) => void) => listen(event, (e) => cb(e.payload));

// In a plain browser (vite dev without Tauri) fall back to demo data so the UI
// can be designed and tested without the Rust side.
const backend = isTauri()
  ? {
      demo: false,
      listGames: (query: LibraryQuery) => invoke<Game[]>("list_games", { query }),
      getGame: (id: number) => invoke<Game | null>("get_game", { id }),
      setHidden: (id: number, hidden: boolean) => invoke<void>("set_hidden", { id, hidden }),
      setFavorite: (id: number, favorite: boolean) =>
        invoke<void>("set_favorite", { id, favorite }),
      launch: (id: number) => invoke<void>("launch_game", { id }),
      accounts: () => invoke<Accounts>("accounts"),
      syncNow: () => invoke<void>("sync_now"),
      epicSignIn: () => invoke<void>("epic_sign_in"),
      epicSignOut: () => invoke<void>("epic_sign_out"),
      steamSetApiKey: (key: string) => invoke<number>("steam_set_api_key", { key }),
      steamSignIn: () => invoke<void>("steam_sign_in"),
      steamSignOut: () => invoke<void>("steam_sign_out"),
      epicOpenBrowser: () => invoke<void>("epic_open_browser"),
      epicSubmitCode: (code: string) => invoke<void>("epic_submit_code", { code }),
      openSteamKeyPage: () => invoke<void>("open_steam_key_page"),
      nowPlaying: () => invoke<NowPlaying[]>("now_playing"),
      gameDetails: (id: number) => invoke<GameDetails | null>("game_details", { id }),
      installInfo: (id: number, refresh: boolean) => invoke<InstallInfo | null>("install_info", { id, refresh }),
      openInstallFolder: (id: number) => invoke<void>("open_install_folder", { id }),
      openStorePage: (id: number) => invoke<void>("open_store_page", { id }),
      gameFolders: () => invoke<string[]>("game_folders"),
      searchStore: (term: string) => invoke<StoreHit[]>("search_store", { term }),
      sgdbSetKey: (key: string) => invoke<void>("sgdb_set_key", { key }),
      sgdbClearKey: () => invoke<void>("sgdb_clear_key"),
      openSgdbKeyPage: () => invoke<void>("open_sgdb_key_page"),
      artworkOptions: (id: number, kind: ArtKind) => invoke<ArtOption[]>("artwork_options", { id, kind }),
      setArtwork: (id: number, kind: ArtKind, url: string) => invoke<void>("set_artwork", { id, kind, url }),
      resetArtwork: (id: number) => invoke<void>("reset_artwork", { id }),
      setStoreMatch: (id: number, steamAppId: number) =>
        invoke<GameDetails | null>("set_store_match", { id, steamAppId }),
      addGameFolder: (path: string) => invoke<string[]>("add_game_folder", { path }),
      removeGameFolder: (path: string) => invoke<string[]>("remove_game_folder", { path }),
      onNowPlaying: on("now-playing"),
      onLibraryChanged: on("library-changed"),
      onAccountsChanged: on("accounts-changed"),
      onSyncStarted: on("sync-started"),
      onSyncError: on("sync-error"),
    }
  : mockBackend;

export const api = backend;
