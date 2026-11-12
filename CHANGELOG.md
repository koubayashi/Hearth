# Changelog

What changed in each Hearth release, newest first. Each version's section
becomes its GitHub release notes, so the release build won't publish a
version that has no section here.

## Unreleased

### New

- **Hearth tells you when an update is ready.** A dot on Settings in the
  sidebar (hover it to see which version), and Settings opens with the
  update at the top: what's new in it and **Restart to update**. The tray
  menu puts the restart first while one waits. The console shows it too.
- **"Updated to Hearth 0.x"** after an update, once, with **What's new**
  leading to the release notes. They stay in Settings › Updates, and show
  without internet.
- **Settings › Updates shows every step:** checking, downloading (how
  far along), ready to install, or why a check failed, with **Try
  again**. It says when Hearth last checked, and **Check now** checks
  straight away.

### Better

- **Hearth notices installs by itself.** Come back to its window and it
  checks what's installed: a game restored from the Recycle Bin, or
  installed or uninstalled in its store, shows up without a refresh.
- **F5 refreshes your library**, like Settings › Refresh library.
- **Hearth feels less like a web page:** right-clicking shows no browser
  menu (text boxes keep cut, copy and paste), and browser shortcuts like
  Ctrl+R, Ctrl+P and Alt+← no longer reload, print or leave the page.

### Fixed

- **"Playing now" counts up.** A game's panel showed the time once, when
  it opened, and stayed there ("Playing now · 1m" after hours). It now
  updates while you play, and **Played** includes the session still
  running. The same in the Console view. Your time was always recorded
  correctly; only the display was stuck.

## 0.3.0

A big one: your library learns what's taking space on your drives,
uninstalls games the right way, treats a game and its remaster as one, and
keeps a history of every session.

### New

- **Storage.** The bottom of Stats now shows how full each drive is, with
  games and everything else marked separately. **Manage storage** lists the
  games on each drive, largest first, with how long since you last played
  each, so the ones to uninstall stand out. Sizes are measured the first
  time you open it; **Measure again** checks them after updates. A drive
  that isn't plugged in keeps its last known sizes.
- **Uninstall.** From a game's ⋯ menu or a Storage row, always through
  whoever installed it, so no store is left thinking the game is still
  there:
  - Steam games: Steam's own uninstall confirmation opens.
  - GOG games: the game's own uninstaller runs.
  - Epic, Xbox and Game Pass games: Hearth opens the Epic launcher or
    Windows' Installed apps and says where to click (neither lets other
    apps uninstall).
  - Games from your own folders: the folder goes to the Recycle Bin, so
    you can get it back.

  Hearth notices when the game is gone and updates the library.
- **Versions of one game.** A game and its remaster or Enhanced edition
  (GTA V and GTA V Enhanced) can be one game in your library, with one
  status, one playtime and one place in your collections. Hearth suggests
  it when two titles differ only by words like Enhanced, Remastered or
  Special Edition, never for a remake, and only combines when you say so.
  Play starts the installed version you played last; the ⌄ menu lists
  each version to play or install. **Versions…** in the game menu combines
  any two games by hand and separates them again, each with its own
  playtime back.
- **Play history.** Click the Played time in a game's panel to see every
  session Hearth timed: totals, the last six months week by week, and each
  session by month. A wrong one, like a game left running overnight, can
  be removed.
- **Sorting.** Sort the library by name, last played, most played,
  recently added, release date or install size. The line under each cover
  shows the value you sorted by. Sort, genres and hidden games share one
  **Sort & filter** menu.
- **Browse by series, developer and publisher.** Collections › Browse
  groups your games by series and studio, looked up automatically, with
  studios under their owners and series under their franchises. Links in a
  game's panel open them. Anything wrong can be fixed by hand with
  **Series and studios…**, and Hearth never undoes your fixes.
- **Favourites** stand out: a ★ beside the name under the cover, first in
  each library section, a ★ chip in the toolbar to show only them, and a
  Favourites shelf in Collections.
- **Tray:** your recent games, and **Stop timing** if Hearth is timing a
  game you've closed.
- Your name and signed-in stores at the foot of the sidebar and on Stats.

### Better

- **Changing artwork** shows a preview of the cover over the background
  before anything is saved. The current picture is always first, marked
  Current. Nothing changes until you press **Save**; Cancel or Esc leaves
  the game as it was.
- **Jump back in** only offers games you can pick up again: not ones
  you've uninstalled, and not ones you've marked Completed or Abandoned.
- **Console view:** Recently played shows installed games first, then the
  rest after a clear gap labelled "Not installed".
- Single choices (the sort order, which way Play starts) show round radio
  marks, so they don't look like boxes you can tick several of.
- The game panel's **Install** row says whether a game is ready to play,
  so "Playing now" can't be confused with the Playing status.
- **Ways to play** explains what they're for: mod loaders, tools,
  launching without the store.
- Settings and Stats sit in the middle of a wide window and fade under the
  window buttons instead of being cut off.

### Fixed

- A game from your own folders that you deleted no longer shows a working
  Play button when its way to play pointed inside the deleted folder.
- A version separated from a combined game gets its artwork straight away
  instead of staying blank.

### Changed

- Storage lives in Stats rather than in the sidebar: you visit it when a
  drive fills up, not every day.
- Jump back in no longer lists uninstalled, completed or abandoned games.
  They're all still in your library; Console's Recently played and Stats
  still include them.
- EA app and Ubisoft Connect are marked "Coming" in Settings: they aren't
  supported yet.

### Good to know

- 0.3.0 adds to your library file (for versions of one game). Nothing is
  removed, and 0.2.1 can still open it if you go back.
- Hearth 0.2.1 offers this update by itself; it installs when you restart
  Hearth. The installer is also on this page.

## 0.2.1

A fix for two buttons that did nothing.

### Fixed

- **Open install folder** (in a game's menu) and **Open error log** (on
  the crash screen) didn't open anything. Windows refused to start
  Explorer without a folder to start in; they work now.

## 0.2.0

Hearth's window behaves like a Windows 11 app, and its colours are yours
to set.

### New

- **Snap Layouts:** hover the Maximize button for Windows' layout flyout,
  as in any Windows 11 app.
- **Seven tones**, each with its own light mode, including three new
  ones: Rosewood, Harbor and Mono. Choose **Dark**, **Light** or **Match
  Windows**, first in Appearance (and in the console's settings).
  Switching cross-fades the whole window.
- **Your own highlight colour:** pick from the tone's own, seven presets
  that work on every tone, or any colour with Hearth's own picker
  (a saturation pad, a hue slider and a hex field). Hearth recolours as you
  drag, and warns you if a colour would be hard to see.

### Better

- In full screen, the Maximize button reads **Exit full screen** and
  leaves it.
- Every tone meets contrast standards for text and the Play button in
  both modes.
- A custom highlight survives switching tones.

## 0.1.2

### New

- **Choose the program a folder game starts.** A game from your own
  folders now says which program Play runs ("Game folder: starts
  game.exe"), and you can pick another one in its folder, such as single
  player instead of multiplayer. Rescans keep your choice.

### Fixed

- **Games that need administrator rights start.** Call of Duty: Modern
  Warfare Remastered's single player, for one, failed to start; such
  games now show Windows' usual permission prompt.

## 0.1.1

The first release: one library for the games you own everywhere.

### What Hearth does

- **Every store in one library:** Steam, Epic, GOG, Xbox and Game Pass,
  and games in folders you choose. Installed and not-installed games side
  by side; the same game on two stores is one game.
- **Sign in to each store** for the games you own but haven't installed,
  their playtime and their artwork. Sign-ins are kept in Windows'
  credential store, never in the library file.
- **Play or install** through each store's own app, or your own way: a
  script, a mod loader or another program, set as the default if you like.
- **Playtime, however you start a game.** Hearth runs in the tray and
  times every session, from Hearth, the store or the game's folder. It
  also reads store totals and some games' save files, and doesn't count
  time the PC was asleep.
- **The library:** large covers with Jump back in above, search, store
  filters, an adjustable cover size, a random game, and hiding games you
  never want to see (kept hidden across syncs). Hidden games open from
  Settings.
- **A game's panel:** description, release date, studio, genres, install
  folder and size, and a right-click menu on every game.
- **Artwork:** store art, or community art from SteamGridDB with your own
  key, and a picker to choose.
- **Statuses and collections:** Playing, Backlog, Completed and Abandoned,
  and collections of your own, on a Collections page.
- **Stats:** total playtime, most played, weekly activity, and breakdowns
  by store and status.
- **Three views:** covers, a list, and a full-screen Console view for the
  couch and a controller.
- **Drives:** games on a drive that's unplugged show "Drive not
  connected", and Hearth rescans when drives come and go.
- **Backups:** the whole library in one file you can keep anywhere, and
  automatic backups after changes.
- **Installer and updates:** installs for you alone, starts with Windows,
  and updates itself from GitHub.
- Follows Windows' transparency and contrast settings, and shows a way out
  (with an error log) instead of a blank window if something breaks.

### Good to know

- GOG and Xbox installs are found without signing in, but haven't been
  tried on a real installed game yet.
