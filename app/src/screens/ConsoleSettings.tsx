import { useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { readyVersion, useUpdate } from "../lib/updates";
import { TONES, setPrefs, usePrefs, useShownMode } from "../lib/prefs";
import { setFullscreen } from "../lib/fullscreen";
import { useStartWithWindows } from "../lib/autostart";
import { nameOptions, pickName, useAccounts } from "../lib/gamertag";
import { Icon } from "../components/Icon";

/**
 * The few settings that make sense from the couch. Sign-ins, keys, folders
 * and backups need typing and file pickers, so they stay in the desktop view.
 */
export function ConsoleSettings({ onOpenDesktop }: { onOpenDesktop: () => void }) {
  const p = usePrefs();
  // An update waiting for a restart leads the list, as in the desktop view.
  const update = readyVersion(useUpdate());
  const [updating, setUpdating] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const shown = useShownMode(p);
  const startup = useStartWithWindows();
  // Only names that exist; typing your own needs the desktop view.
  const accounts = useAccounts();
  const picked = pickName(accounts, p);
  const current = p.nameSource === "off" ? "off" : picked?.source;
  const label: Record<string, string> = { steam: "Steam", epic: "Epic", xbox: "Xbox", gog: "GOG", custom: "Your own" };
  const names = [
    ...nameOptions(accounts, p.customName).map((o) => ({ source: o.source, label: label[o.source] })),
    { source: "off" as const, label: "None" },
  ];
  // Up and down go from row to row; left and right only move between the
  // tones, never off to the Exit button at the top.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    const tone = (e.target as HTMLElement).closest(".cx-tone");
    const next = e.key === "ArrowRight" ? tone?.nextElementSibling : tone?.previousElementSibling;
    if (next instanceof HTMLElement) next.focus();
  };
  return (
    <div className="cx-settings" onKeyDown={onKeyDown}>
      <h1 className="cx-heading">Settings</h1>

      {update && (
        <Row title={`Hearth ${update} is ready`} text={problem ?? "Restart to install it. Takes a few seconds; your library stays as it is."}>
          <button
            className="cx-button"
            data-nav
            disabled={updating}
            onClick={() => {
              setProblem(null);
              setUpdating(true);
              api.installUpdate().catch((e) => (setProblem(String(e)), setUpdating(false)));
            }}
          >
            <Icon name="refresh" size={20} />
            {updating ? "Restarting…" : "Restart to update"}
          </button>
        </Row>
      )}

      <Row title="Tone" text="The background and highlight colour.">
        <div className="cx-tones" role="group" aria-label="Tone">
          {TONES.map((t) => (
            <button
              key={t.id}
              className="cx-tone"
              data-nav
              aria-pressed={p.tone === t.id}
              onClick={() => setPrefs({ tone: t.id, customAccent: null })}
            >
              <i style={{ background: t[shown].base, boxShadow: `inset -10px 0 ${t[shown].accent}` }} />
              {t.label}
            </button>
          ))}
        </div>
      </Row>
      <Row title="Your name" text={picked ? `Shown as ${picked.name} at the top.` : "No name is shown at the top."}>
        <div className="cx-tones" role="group" aria-label="Your name">
          {names.map((n) => (
            <button
              key={n.source}
              className="cx-tone"
              data-nav
              aria-pressed={current === n.source}
              onClick={() => setPrefs({ nameSource: n.source })}
            >
              {n.label}
            </button>
          ))}
        </div>
      </Row>
      <Row title="Mode" text={p.mode === "system" ? `Follows Windows, ${shown} right now.` : "Dark, light, or whichever Windows uses."}>
        <div className="cx-tones" role="group" aria-label="Mode">
          {(["dark", "light", "system"] as const).map((m) => (
            <button key={m} className="cx-tone" data-nav aria-pressed={p.mode === m} onClick={() => setPrefs({ mode: m })}>
              {m === "dark" ? "Dark" : m === "light" ? "Light" : "Match Windows"}
            </button>
          ))}
        </div>
      </Row>
      <Row title="Full screen" text="The console fills the whole screen while it's open.">
        <Switch
          label="Full screen"
          on={p.consoleFullscreen}
          onToggle={() => {
            setPrefs({ consoleFullscreen: !p.consoleFullscreen });
            void setFullscreen(!p.consoleFullscreen);
          }}
        />
      </Row>
      <Row title="Button hints" text="Which controller button does what, shown once you use a controller.">
        <Switch label="Button hints" on={p.consoleHints} onToggle={() => setPrefs({ consoleHints: !p.consoleHints })} />
      </Row>
      <Row title="Start with Windows" text="Hearth waits in the tray, so games started outside it are timed too.">
        <Switch label="Start with Windows" on={!!startup.enabled} disabled={startup.enabled === null} onToggle={startup.toggle} />
      </Row>
      <Row title="More settings" text="Store sign-ins, game folders, artwork keys and backups are in the desktop view.">
        <button className="cx-button" data-nav onClick={onOpenDesktop}>
          <Icon name="settings" size={20} />
          Open in desktop view
        </button>
      </Row>
    </div>
  );
}

function Row({ title, text, children }: { title: string; text: string; children: ReactNode }) {
  return (
    <div className="cx-setting">
      <div className="cx-setting-text">
        <b>{title}</b>
        <span>{text}</span>
      </div>
      {children}
    </div>
  );
}

function Switch({ label, on, disabled, onToggle }: { label: string; on: boolean; disabled?: boolean; onToggle: () => void }) {
  return <button className="switch cx-switch" role="switch" data-nav aria-label={label} aria-checked={on} disabled={disabled} onClick={onToggle} />;
}
