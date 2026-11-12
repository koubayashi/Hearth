import { useSyncExternalStore } from "react";
import { api } from "./api";
import type { UpdateStatus } from "./types";

/**
 * Hearth's own update status, shared: the rail's dot, Settings, the console
 * and the "Updated to" toast all read this one copy, kept current by the
 * `update-status` event.
 */
let current: UpdateStatus | null = null;
const listeners = new Set<() => void>();
let started = false;

function publish(s: UpdateStatus) {
  current = s;
  for (const l of listeners) l();
}

function start() {
  if (started) return;
  started = true;
  void api.onUpdateStatus((p) => publish(p as UpdateStatus));
  api.updateStatus().then(publish).catch(() => {});
}

function subscribe(l: () => void) {
  start();
  listeners.add(l);
  return () => void listeners.delete(l);
}

export function useUpdate(): UpdateStatus | null {
  return useSyncExternalStore(subscribe, () => current);
}

/** A version waiting for a restart (or being installed right now). */
export function readyVersion(s: UpdateStatus | null): string | null {
  return s && (s.phase === "ready" || s.phase === "installing") ? s.version : null;
}

/** The user saw "Updated to …": clear it here at once and in Rust. */
export function markSeen() {
  if (current?.updated_to) publish({ ...current, updated_to: null });
  api.seenUpdate().catch(() => {});
}

// ---------- release notes ----------

/**
 * A changelog section's Markdown, as far as CHANGELOG.md uses it: paragraphs,
 * `###` headings, `-` lists (one level of nesting, lines wrapped with an
 * indent) and **bold**.
 */
export type NoteBlock =
  | { kind: "p"; text: string }
  | { kind: "h"; text: string }
  | { kind: "ul"; items: { text: string; sub: string[] }[] };

export function parseNotes(md: string): NoteBlock[] {
  const blocks: NoteBlock[] = [];
  let para: string[] = [];
  let list: { text: string; sub: string[] }[] | null = null;
  // Where the last line went, so a wrapped line joins it.
  let tail: "item" | "sub" | null = null;
  const flushPara = () => {
    if (para.length) blocks.push({ kind: "p", text: para.join(" ") });
    para = [];
  };
  const flushList = () => {
    if (list?.length) blocks.push({ kind: "ul", items: list });
    list = null;
    tail = null;
  };
  for (const raw of md.split(/\r?\n/)) {
    const line = raw.trimEnd();
    const text = line.trim();
    if (!text) {
      flushPara();
      // After a gap, indented text is a paragraph of the item, not more of
      // its last sub-point.
      if (tail === "sub") tail = "item";
      continue;
    }
    if (text.startsWith("### ")) {
      flushPara();
      flushList();
      blocks.push({ kind: "h", text: text.slice(4) });
      continue;
    }
    const indent = line.length - line.trimStart().length;
    if (text.startsWith("- ")) {
      flushPara();
      if (indent >= 2 && list?.length) {
        list[list.length - 1].sub.push(text.slice(2));
        tail = "sub";
      } else {
        (list ??= []).push({ text: text.slice(2), sub: [] });
        tail = "item";
      }
      continue;
    }
    if (list && indent >= 2 && tail) {
      const item = list[list.length - 1];
      if (tail === "sub") item.sub[item.sub.length - 1] += ` ${text}`;
      // A paragraph inside an item, after its sub-list: part of the item.
      else item.text += ` ${text}`;
      continue;
    }
    flushList();
    para.push(text);
  }
  flushPara();
  flushList();
  return blocks;
}

/** Splits **bold** runs out of a line: odd entries are bold. */
export function boldRuns(text: string): string[] {
  return text.split("**");
}
