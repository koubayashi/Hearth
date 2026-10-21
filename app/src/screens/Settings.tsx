import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import * as autostart from "@tauri-apps/plugin-autostart";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import type { Accounts, Game } from "../lib/types";
import { TONES, setPrefs, usePrefs } from "../lib/prefs";
import { StoreBadges, StoreIcon } from "../components/StoreIcon";
import { Art } from "../components/Art";
import { Icon } from "../components/Icon";
import { formatPlaytime } from "../lib/format";

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
          <span>
            {connected
              ? `Connected to ${accounts?.steam ?? "your account"}. Every game you own and Steam's playtime from all your PCs are included.`
              : "Installed games and this PC's playtime are read automatically. Sign in to add games you own but haven't installed, and playtime from every PC."}
          </span>
        </div>
        {connected ? (
          <button className="chip" data-nav onClick={() => api.steamSignOut()}>
            Disconnect
          </button>
        ) : (
          <button className="chip primary-chip" data-nav disabled={api.demo} onClick={() => api.steamSignIn()}>
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
          <button className="chip" data-nav disabled={!key || busy} onClick={saveKey}>
            {busy ? "Checking…" : "Save key"}
          </button>
          <button className="chip" data-nav onClick={() => api.openSteamKeyPage()}>
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
          <span>
            {signedIn
              ? `Signed in as ${accounts?.epic}. Your full library and Epic playtime are included.`
              : "Installed games show up without signing in. Sign in to add games you own but haven't installed, and your Epic playtime."}
          </span>
        </div>
        {signedIn ? (
          <button className="chip" data-nav onClick={() => api.epicSignOut()}>
            Sign out
          </button>
        ) : (
          <button className="chip primary-chip" data-nav disabled={api.demo} onClick={() => api.epicSignIn()}>
            Sign in
          </button>
        )}
      </div>
      {!signedIn && !browserFlow && (
        <button className="link" data-nav disabled={api.demo} onClick={openBrowser}>
          Sign in with your browser instead (Google, Apple, Steam, Xbox, PlayStation…)
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
            <button className="chip" data-nav disabled={!code || busy} onClick={submit}>
              {busy ? "Signing in…" : "Sign in"}
            </button>
            <button className="chip" data-nav onClick={() => api.epicOpenBrowser()}>
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
          <b>
            <StoreIcon store="manual" size={15} /> Game folders
          </b>
          <span>
            Games installed outside a store. Each game is found whether it sits directly in the folder or in a
            sub-folder; Steam and Epic libraries inside are left to those stores.
          </span>
        </div>
        <button className="chip" data-nav onClick={add} disabled={api.demo}>
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

function StoreAccounts() {
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    const load = () => api.accounts().then(setAccounts);
    load();
    const offs = [
      api.onAccountsChanged(load),
      api.onSyncStarted(() => setSyncing(true)),
      api.onLibraryChanged(() => setSyncing(false)),
    ];
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, []);

  return (
    <>
      <h2>Stores</h2>
      <div className="setting-group glass">
        <SteamRow accounts={accounts} />
        <EpicRow accounts={accounts} />
        <GameFolders />
        <div className="setting">
          <div className="text">
            <b>Refresh library</b>
            <span>Hearth checks every store when it starts. Run it again after installing something.</span>
          </div>
          <button className="chip" data-nav disabled={syncing} onClick={() => (setSyncing(true), api.syncNow())}>
            {syncing ? "Refreshing…" : "Refresh now"}
          </button>
        </div>
      </div>
    </>
  );
}

/** Cover size, previewed with real covers from the library at true size. */
function CoverSizeSetting() {
  const { coverSize, showStoreIcons } = usePrefs();
  const [games, setGames] = useState<Game[]>([]);
  useEffect(() => {
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then((g) =>
      setGames([...g].sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0)).slice(0, 8)),
    );
  }, []);
  return (
    <div className="setting size-setting">
      <div className="text">
        <b>Cover size</b>
        <span>How big game covers are in the library. The preview below is the real size.</span>
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
      />
      <div className="size-scale">
        <span>Smaller, more games</span>
        <span>Larger</span>
      </div>
      <div className="size-preview" aria-hidden>
        {games.map((g) => (
          <span key={g.id} className={`cover ${g.installed ? "" : "out"}`} style={{ width: coverSize }}>
            <span className="cover-art">
              <Art src={g.cover} title={g.title} />
              {showStoreIcons && <StoreBadges stores={g.entries.map((e) => e.store)} />}
            </span>
            <span className="cover-title">{g.title}</span>
            <span className="cover-meta">{g.playtime_secs > 0 ? formatPlaytime(g.playtime_secs) : "Never played"}</span>
          </span>
        ))}
      </div>
    </div>
  );
}

function Startup() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    autostart.isEnabled().then(setEnabled).catch(() => setEnabled(null));
  }, []);
  const toggle = async () => {
    await (enabled ? autostart.disable() : autostart.enable());
    setEnabled(await autostart.isEnabled());
  };
  return (
    <>
      <h2>Playtime</h2>
      <div className="setting-group glass">
        <div className="setting">
          <div className="text">
            <b>Start with Windows</b>
            <span>
              Hearth starts quietly in the tray, so games you launch from their folder or another
              launcher are timed too. Closing the window keeps it running there.
            </span>
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
    </>
  );
}

export function Settings() {
  const p = usePrefs();
  return (
    <div className="scroll">
      <div className="settings">
        <h1>Settings</h1>

        <StoreAccounts />
        <Startup />

        <h2>Appearance</h2>
        <div className="setting-group glass">
          <div className="setting">
            <div className="text">
              <b>Tone</b>
              <span>The background and highlight colour of the whole app.</span>
            </div>
          </div>
          <div className="setting">
            <div className="swatches" role="group" aria-label="Tone">
              {TONES.map((t) => (
                <button
                  key={t.id}
                  className="swatch"
                  data-nav
                  aria-pressed={p.tone === t.id && !p.customAccent}
                  onClick={() => setPrefs({ tone: t.id, customAccent: null })}
                >
                  <i style={{ background: t.base, boxShadow: `inset -8px 0 ${t.accent}` }} />
                  {t.label}
                </button>
              ))}
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>Custom highlight colour</b>
              <span>Replaces the tone's highlight on the Play button and the selected game.</span>
            </div>
            <input
              type="color"
              className="color-input"
              data-nav
              aria-label="Custom highlight colour"
              value={p.customAccent ?? TONES.find((t) => t.id === p.tone)!.accent}
              onChange={(e) => setPrefs({ customAccent: e.target.value })}
            />
            {p.customAccent && (
              <button className="chip" data-nav onClick={() => setPrefs({ customAccent: null })}>
                Reset
              </button>
            )}
          </div>
          <div className="setting">
            <div className="text">
              <b>Light mode</b>
              <span>A bright Porcelain background, for daytime.</span>
            </div>
            <button
              className="switch"
              role="switch"
              data-nav
              aria-label="Light mode"
              aria-checked={p.mode === "light"}
              onClick={() => setPrefs({ mode: p.mode === "light" ? "dark" : "light" })}
            />
          </div>
        </div>

        <h2>Library</h2>
        <div className="setting-group glass">
          <CoverSizeSetting />
          <div className="setting">
            <div className="text">
              <b>Show Metacritic scores</b>
              <span>A critics' score badge next to the genres in the game panel.</span>
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
              <span>A small Steam, Epic, Xbox or other mark on each game, showing where it comes from.</span>
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
        </div>
      </div>
    </div>
  );
}
