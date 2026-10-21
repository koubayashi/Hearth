import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import * as autostart from "@tauri-apps/plugin-autostart";
import { api } from "../lib/api";
import type { Accounts } from "../lib/types";
import { TONES, setPrefs, usePrefs } from "../lib/prefs";

function SteamRow({ accounts }: { accounts: Accounts | null }) {
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const save = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const count = await api.steamSetApiKey(key);
      setKey("");
      setMessage({ ok: true, text: `Connected. Steam shared ${count} games; your library is refreshing.` });
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div className="text">
          <b>Steam</b>
          <span>
            {accounts?.steam ? `Signed in on this PC as ${accounts.steam}. ` : ""}
            Installed games and playtime are read automatically.
            {accounts?.steam_api_key
              ? " Your Steam Web API key is saved, so games you own but haven't installed are included too."
              : " To also see games you own but haven't installed, and playtime from every PC, add your Steam Web API key."}
          </span>
        </div>
        {accounts?.steam_api_key && (
          <button className="chip" data-nav onClick={() => api.steamClearApiKey()}>
            Remove key
          </button>
        )}
      </div>
      {!accounts?.steam_api_key && (
        <div className="key-row">
          <input
            className="text-input"
            type="password"
            data-nav
            placeholder="Paste your 32-character key"
            aria-label="Steam Web API key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && key && !busy && save()}
          />
          <button className="chip" data-nav disabled={!key || busy} onClick={save}>
            {busy ? "Checking…" : "Save key"}
          </button>
          <button className="chip" data-nav onClick={() => api.openSteamKeyPage()}>
            Get a key
          </button>
        </div>
      )}
      {message && <p className={message.ok ? "note" : "note error"} role="status">{message.text}</p>}
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
        <div className="setting">
          <div className="text">
            <b>Epic Games</b>
            <span>
              {accounts?.epic
                ? `Signed in as ${accounts.epic}. Your full library and Epic playtime are included.`
                : "Installed games show up without signing in. Sign in on epicgames.com to add games you own but haven't installed, and your Epic playtime."}
            </span>
          </div>
          {accounts?.epic ? (
            <button className="chip" data-nav onClick={() => api.epicSignOut()}>
              Sign out
            </button>
          ) : (
            <button className="chip" data-nav onClick={() => api.epicSignIn()} disabled={api.demo}>
              Sign in
            </button>
          )}
        </div>
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
