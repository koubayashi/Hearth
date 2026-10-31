import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useStartWithWindows } from "../lib/autostart";
import { confirm, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import type { Accounts } from "../lib/types";
import { TONES, contrast, setPrefs, usePrefs, useShownMode, type NameSource } from "../lib/prefs";
import { pickName, useAccounts } from "../lib/gamertag";
import { isFullscreen, setFullscreen } from "../lib/fullscreen";
import { formatLastPlayed, inSentence } from "../lib/format";
import { StoreIcon } from "../components/StoreIcon";
import { AccentPicker } from "../components/AccentPicker";
import { Icon } from "../components/Icon";

type Note = { ok: boolean; text: string } | null;

function SteamRow({ accounts }: { accounts: Accounts | null }) {
  const [useKey, setUseKey] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<Note>(null);
  const connected = accounts?.steam_auth != null;

  const saveKey = async () => {
    setBusy(true);
    setNote(null);
    try {
      const count = await api.steamSetApiKey(key);
      setKey("");
      setUseKey(false);
      setNote({ ok: true, text: `Connected. Steam shared ${count} games; your library is refreshing.` });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>
            <StoreIcon store="steam" size={15} /> Steam
          </b>
          {connected ? (
            <SignedIn name={accounts?.steam ?? "your Steam account"} detail="Every game you own, with playtime from all your PCs." />
          ) : (
            <span>Installed games and this PC's playtime are read automatically. Sign in for the rest of your library.</span>
          )}
        </div>
        {connected ? (
          <button className="btn" data-nav onClick={() => api.steamSignOut()}>
            Disconnect
          </button>
        ) : (
          <button className="btn btn-primary" data-nav disabled={api.demo} onClick={() => api.steamSignIn()}>
            Sign in
          </button>
        )}
      </div>
      {!connected && !useKey && (
        <button className="link" data-nav onClick={() => setUseKey(true)}>
          Use a Steam Web API key instead
        </button>
      )}
      {!connected && useKey && (
        <div className="key-row">
          <input
            className="text-input"
            type="password"
            data-nav
            placeholder="Paste your 32-character key"
            aria-label="Steam Web API key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && key && !busy && saveKey()}
          />
          <button className="btn btn-primary" data-nav disabled={!key || busy} onClick={saveKey}>
            {busy ? "Checking…" : "Save key"}
          </button>
          <button className="btn" data-nav onClick={() => api.openSteamKeyPage()}>
            Get a key
          </button>
        </div>
      )}
      {note && (
        <p className={note.ok ? "note" : "note error"} role="status">
          {note.text}
        </p>
      )}
    </div>
  );
}

function EpicRow({ accounts }: { accounts: Accounts | null }) {
  const [browserFlow, setBrowserFlow] = useState(false);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<Note>(null);
  const signedIn = !!accounts?.epic;

  const openBrowser = async () => {
    setBrowserFlow(true);
    setNote(null);
    await api.epicOpenBrowser();
  };
  const submit = async () => {
    setBusy(true);
    setNote(null);
    try {
      await api.epicSubmitCode(code);
      setCode("");
      setBrowserFlow(false);
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>
            <StoreIcon store="epic" size={15} /> Epic Games
          </b>
          {signedIn ? (
            <SignedIn name={accounts!.epic!} detail="Your full library and Epic playtime." />
          ) : (
            <span>Installed games show up without signing in. Sign in for the rest of your library and Epic playtime.</span>
          )}
        </div>
        {signedIn ? (
          <button className="btn" data-nav onClick={() => api.epicSignOut()}>
            Sign out
          </button>
        ) : (
          <button className="btn btn-primary" data-nav disabled={api.demo} onClick={() => api.epicSignIn()}>
            Sign in
          </button>
        )}
      </div>
      {!signedIn && !browserFlow && (
        <button className="link" data-nav disabled={api.demo} onClick={openBrowser}>
          Sign in with Google, Apple, Steam, Xbox or PlayStation instead
        </button>
      )}
      {!signedIn && browserFlow && (
        <>
          <p className="note">
            Sign in to Epic in the browser window that opened. Epic then shows a short page with an{" "}
            <b>authorizationCode</b>. Copy that code, or the whole page, and paste it here.
          </p>
          <div className="key-row">
            <input
              className="text-input"
              data-nav
              placeholder="Paste the authorization code"
              aria-label="Epic authorization code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && code && !busy && submit()}
            />
            <button className="btn btn-primary" data-nav disabled={!code || busy} onClick={submit}>
              {busy ? "Signing in…" : "Sign in"}
            </button>
            <button className="btn" data-nav onClick={() => api.epicOpenBrowser()}>
              Open again
            </button>
          </div>
        </>
      )}
      {note && (
        <p className={note.ok ? "note" : "note error"} role="status">
          {note.text}
        </p>
      )}
    </div>
  );
}

/** Microsoft account sign-in for the Xbox games you've played and their playtime. */
function XboxRow({ accounts }: { accounts: Accounts | null }) {
  const signedIn = !!accounts?.xbox;
  return (
    <div className="setting">
      <div className="text">
        <b>
          <StoreIcon store="xbox" size={15} /> Xbox
        </b>
        {signedIn ? (
          <SignedIn name={accounts!.xbox!} detail="Games you've played on Xbox or PC, with Xbox playtime." />
        ) : (
          <span>
            Installed games show up without signing in. Sign in with your Microsoft account for the games you've played
            and Xbox playtime. Games you've never started don't show.
          </span>
        )}
      </div>
      {signedIn ? (
        <button className="btn" data-nav onClick={() => api.xboxSignOut()}>
          Sign out
        </button>
      ) : (
        <button className="btn btn-primary" data-nav disabled={api.demo} onClick={() => api.xboxSignIn()}>
          Sign in
        </button>
      )}
    </div>
  );
}

/** GOG sign-in for every game the account owns. */
function GogRow({ accounts }: { accounts: Accounts | null }) {
  const signedIn = !!accounts?.gog;
  return (
    <div className="setting">
      <div className="text">
        <b>
          <StoreIcon store="gog" size={15} /> GOG
        </b>
        {signedIn ? (
          <SignedIn name={accounts!.gog!} detail="Every game you own on GOG. GOG doesn't share playtime." />
        ) : (
          <span>Installed games show up without signing in. Sign in for the rest of your GOG library.</span>
        )}
      </div>
      {signedIn ? (
        <button className="btn" data-nav onClick={() => api.gogSignOut()}>
          Sign out
        </button>
      ) : (
        <button className="btn btn-primary" data-nav disabled={api.demo} onClick={() => api.gogSignIn()}>
          Sign in
        </button>
      )}
    </div>
  );
}

/** Stores Hearth doesn't read yet, named so nobody goes looking for their games. */
function ComingRow() {
  return (
    <div className="setting">
      <div className="text">
        <b>
          <StoreIcon store="ea" size={15} /> EA app · <StoreIcon store="ubisoft" size={15} /> Ubisoft Connect
        </b>
        <span>Not read yet. Until then, add their installed games with Game folders below.</span>
      </div>
      <span className="tag">Coming</span>
    </div>
  );
}

/** Optional SteamGridDB key for community covers and backgrounds. */
function SgdbRow({ accounts }: { accounts: Accounts | null }) {
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<Note>(null);
  const on = !!accounts?.sgdb;

  const save = async () => {
    setBusy(true);
    setNote(null);
    try {
      await api.sgdbSetKey(key);
      setKey("");
      setNote({ ok: true, text: "Connected. Covers are being updated in the background." });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>Artwork from SteamGridDB</b>
          <span>
            {on
              ? "Community covers and backgrounds. Change any game's art from its ⋯ menu."
              : "Optional community covers and backgrounds, often nicer than store art. Needs a free key."}
          </span>
        </div>
        {on && (
          <div className="key-actions">
            <button
              className="btn"
              data-nav
              onClick={() =>
                api
                  .sgdbCopyKey()
                  .then(() => setNote({ ok: true, text: "Key copied to the clipboard." }))
                  .catch((e) => setNote({ ok: false, text: String(e) }))
              }
            >
              Copy key
            </button>
            <button className="btn" data-nav onClick={() => (setNote(null), api.sgdbClearKey())}>
              Remove key
            </button>
          </div>
        )}
      </div>
      {!on && (
        <div className="key-row">
          <input
            className="text-input"
            type="password"
            data-nav
            placeholder="Paste your SteamGridDB API key"
            aria-label="SteamGridDB API key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && key && !busy && save()}
          />
          <button className="btn btn-primary" data-nav disabled={!key || busy} onClick={save}>
            {busy ? "Checking…" : "Save key"}
          </button>
          <button className="btn" data-nav onClick={() => api.openSgdbKeyPage()}>
            Get a key
          </button>
        </div>
      )}
      {note && (
        <p className={note.ok ? "note" : "note error"} role="status">
          {note.text}
        </p>
      )}
    </div>
  );
}

/** Plain folders of games (DRM-free, copies, other launchers). */
function GameFolders() {
  const [folders, setFolders] = useState<string[]>([]);
  const [note, setNote] = useState<string | null>(null);
  useEffect(() => {
    api.gameFolders().then(setFolders);
  }, []);

  const add = async () => {
    setNote(null);
    try {
      const picked = isTauri() ? await openDialog({ directory: true, title: "Choose a folder with games" }) : null;
      if (typeof picked === "string") setFolders(await api.addGameFolder(picked));
    } catch (e) {
      setNote(String(e));
    }
  };

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>Folders with games</b>
          <span>Games installed outside a store, found in the folder or its sub-folders.</span>
        </div>
        <button className="btn" data-nav onClick={add} disabled={api.demo}>
          Add folder
        </button>
      </div>
      {folders.length > 0 && (
        <ul className="folder-list">
          {folders.map((f) => (
            <li key={f}>
              <Icon name="folder" size={15} />
              <span>{f}</span>
              <button
                className="icon-btn small"
                data-nav
                aria-label={`Stop scanning ${f}`}
                title="Remove"
                onClick={async () => setFolders(await api.removeGameFolder(f))}
              >
                <Icon name="close" size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {note && <p className="note error" role="status">{note}</p>}
    </div>
  );
}

/** Signed-in state, stated plainly under the store's name. */
function SignedIn({ name, detail }: { name: string; detail: string }) {
  return (
    <>
      <span className="signed-in">
        <Icon name="check" size={14} />
        Signed in as <b>{name}</b>
      </span>
      <span>{detail}</span>
    </>
  );
}

/**
 * Your name: in the sidebar, on Stats and in the console. Steam and Epic names can
 * differ, so one is chosen; a store you aren't signed in to can't be picked.
 */
function NameSetting({ accounts }: { accounts: Accounts | null }) {
  const p = usePrefs();
  const picked = pickName(accounts, p);
  const current = p.nameSource === "custom" || p.nameSource === "off" ? p.nameSource : picked?.source;
  const choices: { source: NameSource; label: string; available: boolean }[] = [
    { source: "steam", label: "Steam", available: !!accounts?.steam },
    { source: "epic", label: "Epic", available: !!accounts?.epic },
    { source: "xbox", label: "Xbox", available: !!accounts?.xbox },
    { source: "gog", label: "GOG", available: !!accounts?.gog },
    { source: "custom", label: "Your own", available: true },
    { source: "off", label: "None", available: true },
  ];
  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>Your name</b>
          <span>
            {picked ? (
              <>
                Shown as <b className="inline-name">{picked.name}</b> in the sidebar, on Stats and in the console.
              </>
            ) : p.nameSource === "off" ? (
              "Hearth shows no name."
            ) : (
              "Sign in to Steam or Epic, or type your own name."
            )}
          </span>
        </div>
        <div className="seg seg-boxed" role="group" aria-label="Name from">
          {choices.map((c) => (
            <button
              key={c.source}
              data-nav
              disabled={!c.available}
              title={c.available ? undefined : `Not signed in to ${c.label}`}
              aria-pressed={current === c.source}
              onClick={() => setPrefs({ nameSource: c.source })}
            >
              {c.label}
            </button>
          ))}
        </div>
      </div>
      {p.nameSource === "custom" && (
        <input
          className="text-input"
          data-nav
          maxLength={32}
          placeholder="Your name"
          aria-label="Your name"
          value={p.customName}
          onChange={(e) => setPrefs({ customName: e.target.value })}
        />
      )}
    </div>
  );
}

/** Re-checks every store. An action, so it sits by the page title. */
function RefreshButton() {
  const [syncing, setSyncing] = useState(false);
  useEffect(() => {
    const offs = [api.onSyncStarted(() => setSyncing(true)), api.onLibraryChanged(() => setSyncing(false))];
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, []);
  return (
    <button
      className="btn"
      data-nav
      disabled={syncing}
      title="Hearth checks every store when it starts. Run it again after installing something."
      onClick={() => (setSyncing(true), api.syncNow())}
    >
      <Icon name="refresh" size={15} />
      {syncing ? "Refreshing…" : "Refresh library"}
    </button>
  );
}

const SECTIONS = [
  { id: "stores", label: "Stores" },
  { id: "folders", label: "Game folders" },
  { id: "playtime", label: "Playtime" },
  { id: "appearance", label: "Appearance" },
  { id: "library", label: "Library" },
  { id: "backup", label: "Backup" },
  { id: "updates", label: "Updates" },
] as const;

/**
 * Where you are on the page, and a way to jump: highlights the section in
 * view as you scroll, like Windows Settings.
 */
function SectionIndex() {
  const [active, setActive] = useState<string>(SECTIONS[0].id);
  useEffect(() => {
    const root = document.querySelector<HTMLElement>(".settings-scroll");
    if (!root) return;
    // The last section whose heading has reached the top of the view is
    // current; at the very bottom it's the last one, however short it is.
    let frame = 0;
    const update = () => {
      frame = 0;
      const limit = root.getBoundingClientRect().top + 120;
      const atEnd = root.scrollTop + root.clientHeight >= root.scrollHeight - 2;
      let current: string = SECTIONS[0].id;
      for (const x of SECTIONS) {
        const el = document.getElementById(`settings-${x.id}`);
        if (el && el.getBoundingClientRect().top <= limit) current = x.id;
      }
      setActive(atEnd ? SECTIONS[SECTIONS.length - 1].id : current);
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    root.addEventListener("scroll", onScroll, { passive: true });
    update();
    return () => {
      root.removeEventListener("scroll", onScroll);
      cancelAnimationFrame(frame);
    };
  }, []);
  const go = (id: string) => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    document.getElementById(`settings-${id}`)?.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" });
  };
  return (
    <nav className="settings-index" aria-label="Settings sections">
      {SECTIONS.map((x) => (
        <button key={x.id} data-nav aria-current={active === x.id ? "true" : undefined} onClick={() => go(x.id)}>
          {x.label}
        </button>
      ))}
    </nav>
  );
}

/**
 * The cover size slider. While you drag it (or nudge it with the keys), the
 * rest of Settings fades out and your real library shows underneath at the
 * new size, so you pick the size against your own games.
 */
/** The window's full screen state; F11 changes it too, so it follows resizes. */
function FullscreenSetting() {
  const [on, setOn] = useState(false);
  useEffect(() => {
    const read = () => void isFullscreen().then(setOn);
    read();
    window.addEventListener("resize", read);
    return () => window.removeEventListener("resize", read);
  }, []);
  return (
    <div className="setting">
      <div className="text">
        <b>Full screen</b>
        <span>Fill the whole screen, without the taskbar. F11 switches it any time.</span>
      </div>
      <button
        className="switch"
        role="switch"
        data-nav
        aria-label="Full screen"
        aria-checked={on}
        onClick={() => setFullscreen(!on).then(() => isFullscreen().then(setOn))}
      />
    </div>
  );
}

function CoverSizeSetting({ onPeek }: { onPeek: (on: boolean) => void }) {
  const { coverSize } = usePrefs();
  const idle = useRef<ReturnType<typeof setTimeout> | null>(null);
  const stop = () => {
    if (idle.current) clearTimeout(idle.current);
    onPeek(false);
  };
  useEffect(() => stop, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="setting size-setting">
      <div className="text">
        <b>Cover size</b>
        <span>Your library shows behind Settings while you adjust.</span>
      </div>
      <input
        type="range"
        min={110}
        max={260}
        step={10}
        value={coverSize}
        data-nav
        aria-label="Cover size"
        onChange={(e) => setPrefs({ coverSize: Number(e.target.value) })}
        onPointerDown={(e) => {
          onPeek(true);
          // Ends wherever the pointer is let go, even outside the slider.
          window.addEventListener("pointerup", stop, { once: true });
          window.addEventListener("pointercancel", stop, { once: true });
          e.currentTarget.focus();
        }}
        onKeyDown={(e) => {
          if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(e.key)) return;
          // Keyboard and controller: show the library while the keys move it.
          onPeek(true);
          if (idle.current) clearTimeout(idle.current);
          idle.current = setTimeout(() => onPeek(false), 900);
        }}
        onBlur={stop}
      />
      <div className="size-scale">
        <span>Smaller, more games</span>
        <span>Larger</span>
      </div>
    </div>
  );
}

const BACKUP_FILTER = [{ name: "Hearth backup", extensions: ["hearthbackup"] }];

type AutoStatus = { folder: string | null; last_at: number | null; error: string | null };

/** After the library changes, a backup to a folder you choose: at most hourly, the newest 10 kept. */
function AutoBackupRow() {
  const [status, setStatus] = useState<AutoStatus | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    const load = () => api.autoBackupStatus().then(setStatus).catch(() => {});
    load();
    const off = api.onAutoBackup(load);
    return () => void off.then((f) => f());
  }, []);
  const on = !!status?.folder;

  const choose = async () => {
    setProblem(null);
    const picked = isTauri() ? await openDialog({ directory: true, title: "Choose a folder for automatic backups" }) : null;
    if (typeof picked !== "string") return;
    try {
      setStatus(await api.autoBackupSet(picked));
    } catch (e) {
      setProblem(String(e));
    }
  };
  const turnOff = async () => {
    setProblem(null);
    try {
      setStatus(await api.autoBackupSet(null));
    } catch (e) {
      setProblem(String(e));
    }
  };

  const line = !on
    ? "After changes, a backup to a folder you choose, such as your OneDrive folder. At most hourly; the newest 10 kept."
    : status?.last_at
      ? `To ${status.folder}. Last one ${inSentence(formatLastPlayed(status.last_at))}.`
      : `To ${status?.folder}.`;

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>Automatic backups</b>
          <span>{line}</span>
        </div>
        {on && (
          <button className="btn" data-nav onClick={choose}>
            Change folder
          </button>
        )}
        <button
          className="switch"
          role="switch"
          data-nav
          aria-label="Automatic backups"
          aria-checked={on}
          disabled={status === null || api.demo}
          onClick={() => (on ? turnOff() : choose())}
        />
      </div>
      {(problem || status?.error) && (
        <p className="note error" role="status">
          {problem ?? status?.error}
        </p>
      )}
    </div>
  );
}

/**
 * Everything Hearth knows about your games, in one file you keep wherever you
 * like: another drive, or a OneDrive / Google Drive / Dropbox folder.
 */
function BackupSection() {
  const p = usePrefs();
  const [busy, setBusy] = useState<"backup" | "restore" | null>(null);
  const [note, setNote] = useState<Note>(null);

  const backup = async () => {
    setNote(null);
    const stamp = new Date().toISOString().slice(0, 10);
    const path = isTauri()
      ? await saveDialog({ title: "Save a Hearth backup", defaultPath: `Hearth backup ${stamp}.hearthbackup`, filters: BACKUP_FILTER })
      : null;
    if (!path && isTauri()) return;
    setBusy("backup");
    try {
      await api.backupExport(path ?? "", JSON.stringify(p));
      setPrefs({ lastBackupAt: Date.now() });
      setNote({ ok: true, text: `Saved to ${path}` });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(null);
    }
  };

  const restore = async () => {
    setNote(null);
    const path = isTauri()
      ? await openDialog({ title: "Choose a Hearth backup", multiple: false, directory: false, filters: BACKUP_FILTER })
      : "";
    if (typeof path !== "string" || (isTauri() && !path)) return;
    // The one action here that replaces data: ask first.
    const sure = isTauri()
      ? await confirm("Replace this PC's Hearth library with the backup? Your current library is saved as a copy first.", {
          title: "Restore backup",
          kind: "warning",
          okLabel: "Restore",
          cancelLabel: "Cancel",
        })
      : true;
    if (!sure) return;
    setBusy("restore");
    try {
      const r = await api.backupRestore(path);
      // Ways to play run programs, and a backup file may not be yours: show
      // what they'd run before they come back.
      if (r.ways.length > 0) {
        const list = r.ways.map((w) => "• " + w.name + ": " + [w.path, w.args].filter(Boolean).join(" ")).join("\n");
        const keep = await confirm(
          `This backup has ways to play that start these programs:\n\n${list}\n\nKeep them only if you added them yourself.`,
          { title: "Ways to play in this backup", kind: "warning", okLabel: "Keep", cancelLabel: "Leave out" },
        );
        await api.restoreWays(keep);
      }
      if (r.prefs) {
        try {
          setPrefs(JSON.parse(r.prefs));
        } catch {
          // Settings are a bonus; the library itself is restored.
        }
      }
      const when = r.created_at ? new Date(r.created_at * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : null;
      setNote({ ok: true, text: `Restored the backup${when ? ` from ${when}` : ""}. Sign in to Steam and Epic again if they ask.` });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(null);
    }
  };

  return (
    <section id="settings-backup">
      <h2>Backup</h2>
      <div className="setting-group glass">
        <div className="setting setting-stack">
          <div className="setting-row">
            <div className="text">
              <b>Back up your library</b>
              <span>Games, Hearth's playtime, statuses, collections and settings in one file. Sign-ins aren't included.</span>
              <span className="backup-when">
                {p.lastBackupAt ? `Last backup ${inSentence(formatLastPlayed(Math.floor(p.lastBackupAt / 1000)))}` : "No backup yet"}
              </span>
            </div>
            <button className="btn btn-primary" data-nav disabled={busy !== null} onClick={backup}>
              {busy === "backup" ? "Saving…" : "Back up now"}
            </button>
          </div>
          {note && (
            <p className={note.ok ? "note" : "note error"} role="status">
              {note.text}
            </p>
          )}
        </div>
        <AutoBackupRow />
        <div className="setting">
          <div className="text">
            <b>Restore from a backup</b>
            <span>Replaces this library with a backup file. The current one is kept as a copy.</span>
          </div>
          <button className="btn" data-nav disabled={busy !== null} onClick={restore}>
            {busy === "restore" ? "Restoring…" : "Restore…"}
          </button>
        </div>
      </div>
    </section>
  );
}

function UpdatesSection() {
  const [version, setVersion] = useState<string | null>(null);
  const [ready, setReady] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (isTauri()) getVersion().then(setVersion).catch(() => {});
    const load = () => api.updateReady().then(setReady).catch(() => {});
    load();
    const off = api.onUpdateReady(load);
    return () => void off.then((f) => f());
  }, []);
  const install = async () => {
    setProblem(null);
    setBusy(true);
    try {
      await api.installUpdate();
    } catch (e) {
      setProblem(String(e));
      setBusy(false);
    }
  };
  return (
    <section id="settings-updates">
      <h2>Updates</h2>
      <div className="setting-group glass">
        <div className="setting">
          <div className="text">
            <b>{ready ? `Hearth ${ready} is ready` : version ? `Hearth ${version}` : "Hearth"}</b>
            <span>
              {ready
                ? "Restart to install it. Takes a few seconds; your library stays as it is."
                : import.meta.env.DEV
                  ? "This is a development build, so it doesn't check for updates."
                  : "New versions download in the background and wait here for a restart."}
            </span>
          </div>
          {ready && (
            <button className="btn btn-primary" data-nav disabled={busy} onClick={install}>
              {busy ? "Restarting…" : "Restart to update"}
            </button>
          )}
        </div>
        {problem && (
          <p className="note error" role="status">
            {problem}
          </p>
        )}
      </div>
    </section>
  );
}

function Startup() {
  const { enabled, toggle } = useStartWithWindows();
  return (
    <section id="settings-playtime">
      <h2>Playtime</h2>
      <div className="setting-group glass">
        <div className="setting">
          <div className="text">
            <b>Start with Windows</b>
            <span>Hearth waits in the tray, so games started outside it are timed too.</span>
          </div>
          <button
            className="switch"
            role="switch"
            data-nav
            aria-label="Start with Windows"
            aria-checked={!!enabled}
            disabled={enabled === null}
            onClick={toggle}
          />
        </div>
      </div>
    </section>
  );
}

export function Settings({
  onPeek = () => {},
  hiddenCount = 0,
  onShowHidden = () => {},
}: {
  onPeek?: (on: boolean) => void;
  hiddenCount?: number;
  onShowHidden?: () => void;
}) {
  const p = usePrefs();
  const accounts = useAccounts();
  // Appearance: swatches and the colour picker show the mode on screen.
  const shown = useShownMode(p);
  const tone = TONES.find((t) => t.id === p.tone) ?? TONES[0];
  const toneLabel = tone.label;
  // Focus rings and selections use the highlight: it has to stand out from the ground.
  const faint = !!p.customAccent && contrast(p.customAccent, tone[shown].base) < 3;
  return (
    <div className="scroll settings-scroll">
      <div className="settings">
        <SectionIndex />
        <div className="settings-content">
        <header className="settings-head">
          <h1>Settings</h1>
          <RefreshButton />
        </header>

        <section id="settings-stores">
          <h2>Stores</h2>
          <div className="setting-group glass">
            <SteamRow accounts={accounts} />
            <EpicRow accounts={accounts} />
            <XboxRow accounts={accounts} />
            <GogRow accounts={accounts} />
            <ComingRow />
          </div>
        </section>

        <section id="settings-folders">
          <h2>Game folders</h2>
          <div className="setting-group glass">
            <GameFolders />
          </div>
        </section>

        <Startup />

        <section id="settings-appearance">
        <h2>Appearance</h2>
        <div className="setting-group glass">
          <div className="setting">
            <div className="text">
              <b>Mode</b>
              <span>{p.mode === "system" ? `Follows Windows, ${shown} right now.` : "Dark, light, or whichever Windows uses."}</span>
            </div>
            <div className="seg seg-boxed" role="group" aria-label="Mode">
              {(["dark", "light", "system"] as const).map((m) => (
                <button key={m} data-nav aria-pressed={p.mode === m} onClick={() => setPrefs({ mode: m })}>
                  {m === "dark" ? "Dark" : m === "light" ? "Light" : "Match Windows"}
                </button>
              ))}
            </div>
          </div>
          <div className="setting setting-stack">
            <div className="text">
              <b>Tone</b>
              <span>The background and highlight colour, shown here as they look in {shown} mode.</span>
            </div>
            <div className="swatches" role="group" aria-label="Tone">
              {TONES.map((t) => (
                <button
                  key={t.id}
                  className="swatch"
                  data-nav
                  aria-pressed={p.tone === t.id}
                  onClick={() => setPrefs({ tone: t.id })}
                >
                  <i style={{ background: t[shown].base, boxShadow: `inset -8px 0 ${t[shown].accent}` }} />
                  {t.label}
                </button>
              ))}
            </div>
          </div>
          <div className="setting setting-stack">
            <div className="text">
              <b>Highlight colour</b>
              <span>
                {p.customAccent
                  ? `Used instead of ${toneLabel}'s, in both modes. The first colour goes back to it.`
                  : `${toneLabel}'s own, or one of these. The last one picks any colour.`}
              </span>
              {faint && <span className="note warn">Hard to see on the {shown} background. Pick a {shown === "light" ? "deeper" : "lighter"} colour.</span>}
            </div>
            <AccentPicker
              value={p.customAccent}
              toneAccent={tone[shown].accent}
              toneLabel={toneLabel}
              onChange={(customAccent) => setPrefs({ customAccent })}
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>Soften the art behind the cover</b>
              <span>A light blur on the game art in the game panel, so the cover stands out.</span>
            </div>
            <button
              className="switch"
              role="switch"
              data-nav
              aria-label="Soften the art behind the cover"
              aria-checked={p.blurPosterArt}
              onClick={() => setPrefs({ blurPosterArt: !p.blurPosterArt })}
            />
          </div>
        </div>
        </section>

        <section id="settings-library">
        <h2>Library</h2>
        <div className="setting-group glass">
          <div className="setting">
            <div className="text">
              <b>Layout</b>
              <span>Covers beside the game panel, or a list of titles beside a full game page.</span>
            </div>
            <div className="seg seg-boxed" role="group" aria-label="Layout">
              <button data-nav aria-pressed={p.libraryView === "covers"} onClick={() => setPrefs({ libraryView: "covers" })}>
                <Icon name="grid" size={15} />
                Covers
              </button>
              <button data-nav aria-pressed={p.libraryView === "list"} onClick={() => setPrefs({ libraryView: "list" })}>
                <Icon name="list" size={15} />
                List
              </button>
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>Console</b>
              <span>Full screen with big art, made for a TV and a controller. The Start button opens and closes it.</span>
            </div>
            <button className="btn" data-nav onClick={() => setPrefs({ consoleMode: true })}>
              <Icon name="gamepad" size={16} />
              Open console
            </button>
          </div>
          <NameSetting accounts={accounts} />
          <FullscreenSetting />
          <CoverSizeSetting onPeek={onPeek} />
          <div className="setting">
            <div className="text">
              <b>Show Metacritic scores</b>
              <span>A critics' score next to the genres in the game panel.</span>
            </div>
            <button
              className="switch"
              role="switch"
              data-nav
              aria-label="Show Metacritic scores"
              aria-checked={p.showScores}
              onClick={() => setPrefs({ showScores: !p.showScores })}
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>Show store icons</b>
              <span>A small mark on each cover showing which store it's from.</span>
            </div>
            <button
              className="switch"
              role="switch"
              data-nav
              aria-label="Show store icons"
              aria-checked={p.showStoreIcons}
              onClick={() => setPrefs({ showStoreIcons: !p.showStoreIcons })}
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>Show “Jump back in”</b>
              <span>Your recently played games above the library.</span>
            </div>
            <button
              className="switch"
              role="switch"
              data-nav
              aria-label="Show Jump back in"
              aria-checked={p.showRecent}
              onClick={() => setPrefs({ showRecent: !p.showRecent })}
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>Favourites first</b>
              <span>Starred games get their own section at the top of the library. Star a game from its panel or menu.</span>
            </div>
            <button
              className="switch"
              role="switch"
              data-nav
              aria-label="Favourites first"
              aria-checked={p.showFavoritesRow}
              onClick={() => setPrefs({ showFavoritesRow: !p.showFavoritesRow })}
            />
          </div>
          <SgdbRow accounts={accounts} />
          <div className="setting">
            <div className="text">
              <b>Hidden games</b>
              <span>
                {hiddenCount === 0
                  ? "Nothing hidden. Hide a game from its menu to keep it out of the library."
                  : `${hiddenCount} ${hiddenCount === 1 ? "game" : "games"} kept out of the library. Bring one back from its menu.`}
              </span>
            </div>
            <button className="btn" data-nav disabled={hiddenCount === 0} onClick={onShowHidden}>
              Show
            </button>
          </div>
        </div>
        </section>

        <BackupSection />
        <UpdatesSection />
        </div>
      </div>
    </div>
  );
}
