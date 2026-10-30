# Contributing to Hearth

This guide is for people working on Hearth and for the AI coding agents they use (Claude Code, Codex, Cursor, Copilot and others). If you're an agent, read all of it before you change anything, and follow it over your own defaults.

## What Hearth is

A Windows game library that puts every game you own in one place: Steam, Epic, Xbox / Game Pass, GOG and games from any folder. It launches and installs each game through its own store, and it times play even when a game is started outside Hearth. It lives in the tray, so the tracker keeps running when the window is closed.

What every change has to respect:

1. **One place, zero fiddling.** Sensible defaults beat settings, and every option has to earn its place.
2. **Your library, your rules.** Hiding, merging, collections and statuses are the user's. A sync never undoes them.
3. **Never breaks.** One tested, built-in experience. No themes or plugin system.
4. **Instant.** Starting, scrolling and searching feel immediate with 2,000+ games. No animation may delay input.
5. **Artwork-led.** Games are the content, and the chrome stays out of their way. It should look like a native Windows 11 app, not a web page, and never like flashy "gamer RGB".

Hearth never downloads games itself, and it never asks for store passwords. Sign-in goes through each store's own page.

## Setup

You need Windows 10 or 11, Rust stable (MSVC toolchain), Node 24 and WebView2 (built into Windows 11).

```bash
cd app
npm ci
npm run tauri dev      # the app, with the interface hot-reloading from Vite
npm run dev            # the interface alone in a browser, on demo data
```

The browser-only mode (`npm run dev`) runs on a built-in demo library (`app/src/lib/mock.ts`), so interface work doesn't need the Rust side or any store accounts.

**Your real library is shared.** Dev builds use the same app identifier as installed Hearth, so `tauri dev` reads and writes your real library (`%APPDATA%\dev.hearth.app\library.db`) and sign-ins. Make a backup in Settings → Backup before trying database changes. Only one Hearth runs at a time, so quit the installed one from the tray first.

On a PC short of memory, build with fewer parallel jobs: `set CARGO_BUILD_JOBS=2`. Release builds use LTO and are heavy, but you don't need one for development.

## Layout

```
crates/
  hub-core       data model, SQLite database and migrations, playtime merging
  hub-providers  store integrations (the LibraryProvider trait), artwork, save files
  hub-launch     launching games and store installers
  hub-tracker    playtime tracking from running processes
app/             React + TypeScript + Vite interface
  src/screens    pages (library, list, console, collections, stats, settings)
  src/components shared pieces (rail, poster, cover grid, menus)
  src/lib        API bridge, types, formatting, preferences
  src/styles     tokens.css (colours, spacing, motion) and the stylesheets
app/src-tauri/   the Tauri shell: commands, tray, sync, sign-in windows, updates
```

Database changes are numbered migrations in `crates/hub-core/migrations/`. Add a new file; never edit one that has shipped.

## Before you open a pull request

Run all of these, and make sure they pass:

```bash
cargo test --workspace --exclude hearth   # from the repo root
cd app
npx tsc -b
npm test
```

For interface changes, also try them in the app, in both the covers and list layouts, in light and dark mode, with the keyboard alone, and in console mode if you touched it.

## How the code is written

- **Match the code around you:** its naming, structure and amount of comments. Comments say why, not what.
- **Keep changes small and focused.** Don't reformat files or refactor code your change doesn't need.
- **Interface text is plain and short.** Say what happens, in everyday words, without jargon or exclamation marks. Errors say what to do next.
- **Use the design tokens** in `app/src/styles/tokens.css` for colour, spacing, radius and motion, never raw values. Every tone has light and dark versions; check both.
- **Everything works by keyboard and gamepad,** with visible focus. Motion respects reduced-motion, and text over artwork meets WCAG AA contrast.
- **Performance:** long lists are virtualized; keep them that way. Animate only `transform` and `opacity`.
- **Store integrations:**
  - Tokens go in Windows Credential Manager (`hub-providers/src/secrets.rs`), never in files or logs.
  - Errors that might contain a URL go through `tracking::redact`.
  - Sign-in windows use their store's own browser profile (`app/src-tauri/src/auth.rs`).
- **Tests:** new behaviour in `hub-core`, `hub-providers` or `hub-tracker` gets a unit test next to it. Use made-up data in tests, never real accounts or libraries.

## What not to change without asking first

- The app identifier (`dev.hearth.app`), the updater public key and the update endpoint in `app/src-tauri/tauri.conf.json`. Changing them cuts installed copies off from their data or their updates.
- `.github/workflows/release.yml`. Its actions are pinned to commits on purpose, because the update signing key passes through them.
- Versions and releases. The maintainer cuts releases with `scripts/release.mjs`.

## Never commit

- Secrets: keys (the updater signing key above all), certificates, tokens, `.env` files.
- Personal data: library databases, `.hearthbackup` files, logs, screenshots of a real library, or account names, emails and IDs.
- AI assistant files and working notes: `CLAUDE.md`, `AGENTS.md`, `.claude/`, `.cursor/`, `DECISIONS.md`, `BACKLOG.md`, `PRODUCT.md`, `DESIGN.md`, `.impeccable/` and the like. Keep them on your machine. `.gitignore` covers the common ones.

Check `git diff --cached --stat` before every commit.

## Commits and pull requests

- **Commit summary:** one line in plain words saying what changed for the user, such as "Hidden games: off the main rail, opened from Settings".
- **Commit body:** says why, and what you ruled out.
- **One topic per pull request.** In the description, explain any real decision you made (the choice, why, and the alternatives), since that is where the reasoning is kept.
- **AI-assisted work is welcome.** You're responsible for every line: read it, run it and test it before you open the request.

## License

By contributing, you agree that your work is licensed under GPL-3.0-or-later, as the rest of Hearth is.
