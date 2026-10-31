import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { LaunchOptionInput, Via } from "./types";
import type { Accounts, ArtKind, ArtOption, Collection, Game, GameStatus, GameDetails, InstallInfo, LibraryQuery, NowPlaying, PlaySession, SmartIndex, SmartRole, StoreHit } from "./types";
import { mockBackend } from "./mock";

const on = (event: string) => (cb: (payload: unknown) => void) => listen(event, (e) => cb(e.payload));

// In a plain browser (vite dev without Tauri) fall back to demo data so the UI
// can be designed and tested without the Rust side.
const backend = isTauri()
  ? {
      demo: false,
      listGames: (query: LibraryQuery) => invoke<Game[]>("list_games", { query }),
      gameCounts: () => invoke<{ library: number; hidden: number }>("game_counts"),
      getGame: (id: number) => invoke<Game | null>("get_game", { id }),
      setHidden: (id: number, hidden: boolean) => invoke<void>("set_hidden", { id, hidden }),
      setFavorite: (id: number, favorite: boolean) =>
        invoke<void>("set_favorite", { id, favorite }),
      launch: (id: number, via?: Via) => invoke<void>("launch_game", { id, via: via ?? null }),
      saveLaunchOption: (option: LaunchOptionInput) => invoke<number>("save_launch_option", { option }),
      deleteLaunchOption: (id: number) => invoke<void>("delete_launch_option", { id }),
      setDefaultLaunch: (gameId: number, option: number | null) =>
        invoke<void>("set_default_launch", { gameId, option }),
      setFolderExe: (gameId: number, path: string | null) => invoke<void>("set_folder_exe", { gameId, path }),
      accounts: () => invoke<Accounts>("accounts"),
      syncNow: () => invoke<void>("sync_now"),
      epicSignIn: () => invoke<void>("epic_sign_in"),
      epicSignOut: () => invoke<void>("epic_sign_out"),
      xboxSignIn: () => invoke<void>("xbox_sign_in"),
      xboxSignOut: () => invoke<void>("xbox_sign_out"),
      gogSignIn: () => invoke<void>("gog_sign_in"),
      gogSignOut: () => invoke<void>("gog_sign_out"),
      steamSetApiKey: (key: string) => invoke<number>("steam_set_api_key", { key }),
      steamSignIn: () => invoke<void>("steam_sign_in"),
      steamSignOut: () => invoke<void>("steam_sign_out"),
      epicOpenBrowser: () => invoke<void>("epic_open_browser"),
      epicSubmitCode: (code: string) => invoke<void>("epic_submit_code", { code }),
      openSteamKeyPage: () => invoke<void>("open_steam_key_page"),
      nowPlaying: () => invoke<NowPlaying[]>("now_playing"),
      playSessions: () => invoke<PlaySession[]>("play_sessions"),
      deletePlaySession: (id: number) => invoke<void>("delete_play_session", { id }),
      backupExport: (path: string, prefs: string) => invoke<void>("backup_export", { path, prefs }),
      autoBackupStatus: () => invoke<{ folder: string | null; last_at: number | null; error: string | null }>("auto_backup_status"),
      autoBackupSet: (folder: string | null) => invoke<{ folder: string | null; last_at: number | null; error: string | null }>("auto_backup_set", { folder }),
      savePrefsSnapshot: (prefs: string) => invoke<void>("save_prefs_snapshot", { prefs }),
      reportError: (message: string) => invoke<void>("report_error", { message }),
      openErrorLog: () => invoke<void>("open_error_log"),
      backupRestore: (path: string) =>
        invoke<{ prefs: string | null; created_at: number | null; ways: { name: string; path: string; args: string }[] }>("backup_restore", { path }),
      restoreWays: (keep: boolean) => invoke<void>("restore_ways", { keep }),
      gameDetails: (id: number) => invoke<GameDetails | null>("game_details", { id }),
      installInfo: (id: number, refresh: boolean) => invoke<InstallInfo | null>("install_info", { id, refresh }),
      savedGameInfo: () => invoke<{ id: number; details: GameDetails | null; install: InstallInfo | null }[]>("saved_game_info"),
      openInstallFolder: (id: number) => invoke<void>("open_install_folder", { id }),
      openStorePage: (id: number) => invoke<void>("open_store_page", { id }),
      gameFolders: () => invoke<string[]>("game_folders"),
      searchStore: (term: string) => invoke<StoreHit[]>("search_store", { term }),
      sgdbSetKey: (key: string) => invoke<void>("sgdb_set_key", { key }),
      setStatus: (id: number, status: GameStatus | null) => invoke<void>("set_status", { id, status }),
      listCollections: () => invoke<Collection[]>("list_collections"),
      createCollection: (name: string) => invoke<number>("create_collection", { name }),
      renameCollection: (id: number, name: string) => invoke<void>("rename_collection", { id, name }),
      deleteCollection: (id: number) => invoke<void>("delete_collection", { id }),
      setInCollection: (collection: number, game: number, member: boolean) =>
        invoke<void>("set_in_collection", { collection, game, member }),
      sgdbClearKey: () => invoke<void>("sgdb_clear_key"),
      sgdbCopyKey: () => invoke<void>("sgdb_copy_key"),
      openSgdbKeyPage: () => invoke<void>("open_sgdb_key_page"),
      artworkOptions: (id: number, kind: ArtKind) => invoke<ArtOption[]>("artwork_options", { id, kind }),
      setArtwork: (id: number, kind: ArtKind, url: string) => invoke<void>("set_artwork", { id, kind, url }),
      resetArtwork: (id: number) => invoke<void>("reset_artwork", { id }),
      storeArtwork: (id: number) => invoke<{ cover: string | null; hero: string | null }>("store_artwork", { id }),
      smartIndex: () => invoke<SmartIndex>("smart_index"),
      setGameLink: (gameId: number, role: SmartRole, entity: string, on: boolean) =>
        invoke<void>("set_game_link", { gameId, role, entity, on }),
      createSeries: (gameId: number, name: string) => invoke<string>("create_series", { gameId, name }),
      setEntityParent: (child: string, parent: string, on: boolean) => invoke<void>("set_entity_parent", { child, parent, on }),
      setStoreMatch: (id: number, steamAppId: number) =>
        invoke<GameDetails | null>("set_store_match", { id, steamAppId }),
      addGameFolder: (path: string) => invoke<string[]>("add_game_folder", { path }),
      removeGameFolder: (path: string) => invoke<string[]>("remove_game_folder", { path }),
      onNowPlaying: on("now-playing"),
      onLibraryChanged: on("library-changed"),
      onAccountsChanged: on("accounts-changed"),
      onSyncStarted: on("sync-started"),
      onSmartStatus: on("smart-status"),
      onSmartChanged: on("smart-changed"),
      onAutoBackup: on("auto-backup"),
      onSyncError: on("sync-error"),
      updateReady: () => invoke<string | null>("update_ready"),
      installUpdate: () => invoke<void>("install_update"),
      onUpdateReady: on("update-ready"),
    }
  : mockBackend;

export const api = backend;
