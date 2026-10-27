import { Component, useState, type ErrorInfo, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "../lib/api";
import { setPrefs, usePrefs } from "../lib/prefs";
import { TitleBar } from "./TitleBar";

const describe = (error: unknown) =>
  error instanceof Error ? (error.stack ?? `${error.name}: ${error.message}`) : String(error);

/** Errors outside React's drawing (a failed promise, an event handler) don't
 *  break the screen, so they're only written to the error log. */
export function reportUnhandledErrors() {
  const report = (what: string) => void api.reportError(what).catch(() => {});
  window.addEventListener("error", (e) => report(describe(e.error ?? e.message)));
  window.addEventListener("unhandledrejection", (e) => report(`unhandled promise: ${describe(e.reason)}`));
}

interface State {
  error: unknown;
  details: string;
}

/** Catches a screen that failed to draw and shows a way out instead of a
 *  blank window. The tracker runs in the Rust process and keeps going. */
export class CrashBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null, details: "" };

  static getDerivedStateFromError(error: unknown): Partial<State> {
    return { error };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    const report = `${describe(error)}\ncomponent stack:${info.componentStack ?? " unknown"}`;
    void api.reportError(report).catch(() => {});
    const version = isTauri() ? getVersion() : Promise.resolve("demo");
    void version
      .catch(() => "unknown")
      .then((v) => this.setState({ details: `Hearth ${v}\n${navigator.userAgent}\n\n${report}` }));
  }

  render() {
    if (this.state.error === null) return this.props.children;
    return <CrashScreen error={this.state.error} details={this.state.details} />;
  }
}

function CrashScreen({ error, details }: { error: unknown; details: string }) {
  const { consoleMode } = usePrefs();
  const [copied, setCopied] = useState(false);
  const message = error instanceof Error ? error.message : String(error);
  const copy = () =>
    void navigator.clipboard.writeText(details || describe(error)).then(() => setCopied(true), () => {});
  return (
    <>
      <TitleBar />
      <main className="crash" role="alert">
        <h1>Hearth ran into a problem</h1>
        <p>
          This screen couldn't be shown. Your library and playtime are safe, and Hearth keeps timing games from the
          tray.
        </p>
        {message && <p className="crash-message">{message}</p>}
        <div className="crash-actions">
          <button className="btn btn-primary" autoFocus onClick={() => location.reload()}>
            Reload Hearth
          </button>
          {consoleMode && (
            <button className="btn" onClick={() => (setPrefs({ consoleMode: false }), location.reload())}>
              Open desktop view
            </button>
          )}
          {!api.demo && (
            <button className="btn" onClick={() => void api.openErrorLog().catch(() => {})}>
              Show error log
            </button>
          )}
          <button className="btn" onClick={copy}>
            {copied ? "Copied" : "Copy details"}
          </button>
        </div>
      </main>
    </>
  );
}
