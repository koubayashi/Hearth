import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "../lib/api";
import { useNowSecs } from "../lib/clock";
import { formatBytes, formatLastPlayed, inSentence } from "../lib/format";
import { boldRuns, parseNotes, readyVersion, useUpdate } from "../lib/updates";
import type { UpdateStatus } from "../lib/types";
import { Icon } from "./Icon";

/** A changelog section, set as in CHANGELOG.md: headings, lists, bold. */
export function ReleaseNotes({ md }: { md: string }) {
  const inline = (text: string) => boldRuns(text).map((run, i) => (i % 2 ? <b key={i}>{run}</b> : run));
  return (
    <div className="release-notes">
      {parseNotes(md).map((b, i) =>
        b.kind === "h" ? (
          <h4 key={i}>{b.text}</h4>
        ) : b.kind === "p" ? (
          <p key={i}>{inline(b.text)}</p>
        ) : (
          <ul key={i}>
            {b.items.map((item, j) => (
              <li key={j}>
                {inline(item.text)}
                {item.sub.length > 0 && (
                  <ul>
                    {item.sub.map((s, k) => (
                      <li key={k}>{inline(s)}</li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        ),
      )}
    </div>
  );
}

/** "What's new in 0.3.1", folded unless asked to start open. */
function NotesDisclosure({ version, md, open }: { version: string; md: string; open?: boolean }) {
  return (
    <details className="notes-disclosure" open={open}>
      <summary data-nav>
        <Icon name="chevronRight" size={14} />
        What's new in {version}
      </summary>
      <ReleaseNotes md={md} />
    </details>
  );
}

function useRestart() {
  const [problem, setProblem] = useState<string | null>(null);
  const restart = () => {
    setProblem(null);
    api.installUpdate().catch((e) => setProblem(String(e)));
  };
  return { problem, restart };
}

/**
 * At the very top of Settings while an update waits, so the rail's dot leads
 * straight to it (as iOS puts a waiting update above everything in
 * Settings). Gone once it's installed.
 */
export function UpdateReadyBanner() {
  const s = useUpdate();
  const version = readyVersion(s);
  const { problem, restart } = useRestart();
  if (!s || !version) return null;
  const installing = s.phase === "installing";
  return (
    <section className="update-ready" aria-label="Update ready">
      <div className="setting-group glass">
        <div className="setting">
          <div className="text">
            <b>Hearth {version} is ready</b>
            <span>Restart to install it. Takes a few seconds; your library stays as it is.</span>
          </div>
          <button className="btn btn-primary" data-nav disabled={installing} onClick={restart}>
            {installing ? "Restarting…" : "Restart to update"}
          </button>
        </div>
        {(problem ?? s.error) && (
          <p className="note error update-error" role="status">
            {problem ?? s.error}
          </p>
        )}
        {s.notes && <NotesDisclosure version={version} md={s.notes} open />}
      </div>
    </section>
  );
}

/** Settings › Updates: every state of an update, and what's new in this version. */
export function UpdatesSection({ notesOpen }: { notesOpen?: boolean }) {
  const s = useUpdate();
  const [current, setCurrent] = useState<string | null>(null);
  const [notes, setNotes] = useState<string | null>(null);
  const { problem, restart } = useRestart();
  useEffect(() => {
    if (isTauri()) getVersion().then(setCurrent).catch(() => {});
    else setCurrent("0.3.0");
    api.releaseNotes().then(setNotes).catch(() => {});
  }, []);
  // "Checked 2 hours ago" stays true while Settings is open.
  const now = useNowSecs(true, 60_000);
  const name = current ? `Hearth ${current}` : "Hearth";
  const ready = readyVersion(s);
  const ago = s?.last_checked ? inSentence(formatLastPlayed(s.last_checked, now * 1000)) : null;
  const checked = ago ? `Checked ${ago}` : "Not checked yet";
  const lastChecked = ago ? `Last checked ${ago}` : "Not checked yet";
  const check = () => void api.checkUpdates().catch(() => {});

  let row: React.ReactNode;
  let meta: React.ReactNode = null;
  if (!s || s.dev) {
    row = (
      <Row title={`${name}, development build`} text="Runs from the source code, so it doesn't check for updates." />
    );
  } else if (ready) {
    // The same row heads Settings; here it is again where updates live.
    row = (
      <Row title={`Hearth ${ready} is ready`} text={`You have ${current ?? "an older version"}. Restart to install it.`}>
        <button className="btn btn-primary" data-nav disabled={s.phase === "installing"} onClick={restart}>
          {s.phase === "installing" ? "Restarting…" : "Restart to update"}
        </button>
      </Row>
    );
  } else if (s.phase === "downloading") {
    row = <Downloading s={s} />;
  } else if (s.phase === "checking") {
    row = <Row title={name} text="Checking for updates…" />;
    meta = <Meta left={lastChecked} action={<button className="link" disabled>Check now</button>} />;
  } else if (s.error) {
    row = (
      <Row title={name} text={<span className="error-text">{s.error}</span>}>
        <button className="btn" data-nav onClick={check}>
          Try again
        </button>
      </Row>
    );
    meta = ago ? <Meta left={lastChecked} /> : null;
  } else {
    row = (
      <Row
        title={name}
        text={
          s.last_checked ? (
            <>
              <Icon name="check" size={14} className="up-to-date" />
              You're on the newest version.
            </>
          ) : (
            "Hearth checks for updates a minute after it starts, then twice a day."
          )
        }
      />
    );
    meta = (
      <Meta
        left={checked}
        action={
          <button className="link" data-nav onClick={check}>
            Check now
          </button>
        }
      />
    );
  }

  return (
    <section id="settings-updates">
      <h2>Updates</h2>
      <div className="setting-group glass">
        {row}
        {problem && (
          <p className="note error update-error" role="status">
            {problem}
          </p>
        )}
        {meta}
        {s?.phase === "downloading" && s.notes && s.version && <NotesDisclosure version={s.version} md={s.notes} />}
        {notes && current && !ready && s?.phase !== "downloading" && <NotesDisclosure version={current} md={notes} open={notesOpen} />}
      </div>
    </section>
  );
}

function Downloading({ s }: { s: UpdateStatus }) {
  const pct = s.total ? Math.min(100, Math.round((s.done / s.total) * 100)) : null;
  const amount = pct !== null ? `${pct}% of ${formatBytes(s.total!)}` : s.done ? `${formatBytes(s.done)} so far` : "Starting";
  return (
    <div className="setting">
      <div className="text">
        <b>Downloading Hearth {s.version}…</b>
        <span>{amount}. It installs when you restart.</span>
        <div className="update-bar" role="progressbar" aria-label="Download" aria-valuenow={pct ?? undefined} aria-valuemin={0} aria-valuemax={100}>
          <i style={{ transform: `scaleX(${(pct ?? 0) / 100})` }} />
        </div>
      </div>
    </div>
  );
}

function Row({ title, text, children }: { title: string; text: React.ReactNode; children?: React.ReactNode }) {
  return (
    <div className="setting">
      <div className="text">
        <b>{title}</b>
        <span>{text}</span>
      </div>
      {children}
    </div>
  );
}

function Meta({ left, action }: { left: string; action?: React.ReactNode }) {
  return (
    <div className="update-meta">
      <span>{left}</span>
      {action}
    </div>
  );
}
