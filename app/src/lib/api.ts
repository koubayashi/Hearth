import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Accounts, Game, LibraryQuery, NowPlaying } from "./types";
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
      nowPlaying: () => invoke<NowPlaying[]>("now_playing"),
      onNowPlaying: on("now-playing"),
      onLibraryChanged: on("library-changed"),
      onAccountsChanged: on("accounts-changed"),
      onSyncStarted: on("sync-started"),
      onSyncError: on("sync-error"),
    }
  : mockBackend;

export const api = backend;
