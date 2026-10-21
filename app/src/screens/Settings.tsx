import { TONES, setPrefs, usePrefs } from "../lib/prefs";

export function Settings() {
  const p = usePrefs();
  return (
    <div className="scroll">
      <div className="settings">
        <h1>Settings</h1>

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
