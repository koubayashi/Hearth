import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Game, LibraryQuery } from "./types";
import { mockBackend } from "./mock";

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
      onLibraryChanged: (cb: () => void) => listen("library-changed", cb),
    }
  : mockBackend;

export const api = backend;
