# Hearth

A fast, simple, open-source game library for Windows. One hub for every game you own on Steam, Epic, Xbox / Game Pass, GOG, EA and Ubisoft, plus games from anywhere else.

> Status: early development. Not usable yet.

## What it does

- **One library.** Installed and not-yet-installed games together. Install hands off to each game's own store.
- **Hide what you don't play.** Hidden games stay hidden, even after a re-sync (goodbye, unwanted Epic freebies).
- **Playtime, however you launch.** Hearth sits in the tray and times games you start from their folder or another launcher. Store playtime (Steam, Epic, Xbox, GOG) is merged in without double counting.
- **Duplicates merged.** Own a game on two stores? You get one tile, and you choose which store launches it.
- **Built-in look that never breaks.** No third-party themes to break on update. Accent colour, light/dark and density are built in.

## Stack

- [Tauri 2](https://tauri.app) shell with a Rust backend
- React + TypeScript + Vite frontend in `app/`
- SQLite library database in `%APPDATA%\dev.hearth.app\library.db`

```
crates/
  hub-core       data model, SQLite database, playtime merging
  hub-providers  store integrations (LibraryProvider trait)
  hub-launch     launching games and store installers
app/             frontend
app/src-tauri/   Tauri shell
```

## Development

Requires Rust (stable, MSVC toolchain), Node 20+ and WebView2 (bundled with Windows 11).

```bash
cd app
npm install
npm run tauri dev
```

Run the backend tests from the repo root:

```bash
cargo test --workspace
```

## License

GPL-3.0-or-later
