// SPDX-License-Identifier: AGPL-3.0-or-later
// Jumps to places (Round 30a, contract §3.2): find/at from the address, a[data-jump], jump history, Alt+←.
//
// - Jump address `/?work=W&key=K[&rev=R]&find=<Text>[&at=<Byte>]` (server: crate::xref::jump_url). The server
//   ignores find/at; here it becomes the event `baruch:find {text, at}` on the document, to which wysiwyg.ts
//   (typeset view, CSS Custom Highlight API) and search.ts (source text, selection in the text field) listen – on
//   loading the page and after every redraw by workspace.ts (`baruch:rendered`).
// - Every `a[data-jump]` opens the section on the workspace page without a page change (`baruch:navigate`); the
//   place of origin goes into the jump history (sessionStorage `baruch_jumps`, only in try/catch). With
//   `data-jump-origin="chat"` the button „Zurück zum Chat“ appears afterwards. Outside the workspace page the
//   link stays an ordinary link (the page loads with find/at).
// - Alt+← (workspace.ts): if the history is not empty, back to the place of origin ([`popJump`]); otherwise as before.
//   "Forward" exists only as a button in the jump bar.
// No imports: the pure functions (history, byte → UTF-16) run under Node (tests/round30_verweise.rs).

export interface JumpEntry {
  url: string;
  origin?: string;
}

export interface JumpState {
  back: JumpEntry[];
  forward: JumpEntry[];
}

/// Maximum number of remembered jumps.
export const MAX_JUMPS = 50;

export function emptyState(): JumpState {
  return { back: [], forward: [] };
}

/// Jump from `from`: place of origin into the history, "forward" expires.
export function pushJump(state: JumpState, from: JumpEntry): JumpState {
  const back = [...state.back, from].slice(-MAX_JUMPS);
  return { back, forward: [] };
}

/// Back: last place of origin and new state (the current place `here` goes to "forward"); empty → null.
export function popJump(state: JumpState, here: string): { state: JumpState; entry: JumpEntry } | null {
  const entry = state.back[state.back.length - 1];
  if (!entry) return null;
  return { state: { back: state.back.slice(0, -1), forward: [...state.forward, { url: here }].slice(-MAX_JUMPS) }, entry };
}

/// Forward (only through the button): the reverse of [`popJump`].
export function forwardJump(state: JumpState, here: string): { state: JumpState; entry: JumpEntry } | null {
  const entry = state.forward[state.forward.length - 1];
  if (!entry) return null;
  return { state: { back: [...state.back, { url: here }].slice(-MAX_JUMPS), forward: state.forward.slice(0, -1) }, entry };
}

/// UTF-16 position of the byte `byte` (UTF-8) in `text`; behind the end → length.
export function byteToUtf16(text: string, byte: number): number {
  let bytes = 0;
  let i = 0;
  for (const ch of text) {
    if (bytes >= byte) return i;
    const cp = ch.codePointAt(0) ?? 0;
    bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    i += ch.length;
  }
  return text.length;
}

/// Position of the text `find` in `text` that lies nearest to `near` (UTF-16); whitespace counts as any whitespace.
/// Not found → null. Result `[start, end]` in UTF-16.
export function locate(text: string, find: string, near: number): [number, number] | null {
  const needle = find.trim();
  if (!needle) return null;
  const pattern = new RegExp(needle.split(/\s+/).map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("\\s+"), "g");
  let best: [number, number] | null = null;
  for (const m of text.matchAll(pattern)) {
    const start = m.index ?? 0;
    const hit: [number, number] = [start, start + m[0].length];
    if (!best || Math.abs(start - near) < Math.abs(best[0] - near)) best = hit;
  }
  return best;
}

/// find/at from an address (or null).
export function findOf(url: string): { text: string; at: number | null } | null {
  let parsed: URL;
  try {
    parsed = new URL(url, "http://baruch.invalid/");
  } catch {
    return null;
  }
  const text = parsed.searchParams.get("find");
  if (!text) return null;
  const at = parsed.searchParams.get("at");
  return { text, at: at !== null && /^\d+$/.test(at) ? Number(at) : null };
}

// ---- Page ---------------------------------------------------------------------------------------------------

const STORE_KEY = "baruch_jumps";

function load(): JumpState {
  try {
    const raw = sessionStorage.getItem(STORE_KEY);
    if (!raw) return emptyState();
    const parsed = JSON.parse(raw) as JumpState;
    if (Array.isArray(parsed.back) && Array.isArray(parsed.forward)) return parsed;
  } catch {
    // No storage (private window, blocked): continue without history.
  }
  return emptyState();
}

function save(state: JumpState): void {
  try {
    sessionStorage.setItem(STORE_KEY, JSON.stringify(state));
  } catch {
    // Without storage the history lasts only until reloading.
  }
  renderBar(state);
}

/// Pending place found for the next redraw.
let pending: { text: string; at: number | null } | null = null;

function announce(found: { text: string; at: number | null }): void {
  document.dispatchEvent(new CustomEvent("baruch:find", { detail: found }));
}

function onWorkspace(): boolean {
  return document.querySelector("[data-widget=workspace]") !== null && document.getElementById("pane-editor") !== null;
}

function here(): string {
  return window.location.pathname + window.location.search;
}

/// Jump to `url`: on the workspace page without a page change, otherwise normally.
function go(url: string): void {
  pending = findOf(url);
  if (onWorkspace()) document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url } }));
  else window.location.href = url;
}

/// Back to the last place of origin (Alt+← with a non-empty history); `false` = history empty.
export function jumpBack(): boolean {
  const result = popJump(load(), here());
  if (!result) return false;
  save(result.state);
  go(result.entry.url);
  return true;
}

function jumpForward(): void {
  const result = forwardJump(load(), here());
  if (!result) return;
  save(result.state);
  go(result.entry.url);
}

/// Jump bar: „← Zurück“, „Vorwärts →“, „Zurück zum Chat“ (only with a history).
function renderBar(state: JumpState): void {
  if (typeof document === "undefined") return;
  let bar = document.querySelector<HTMLElement>("[data-jump-bar]");
  const last = state.back[state.back.length - 1];
  if (state.back.length === 0 && state.forward.length === 0) {
    bar?.remove();
    return;
  }
  if (!bar) {
    bar = document.createElement("nav");
    bar.className = "jump-bar";
    bar.dataset.jumpBar = "";
    bar.setAttribute("aria-label", "Sprungverlauf");
    document.body.append(bar);
  }
  bar.replaceChildren();
  const button = (label: string, title: string, action: () => void) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "btn small";
    b.textContent = label;
    b.title = title;
    b.addEventListener("click", action);
    bar?.append(b);
  };
  if (last?.origin === "chat") button("Zurück zum Chat", "Zur Stelle im Chat zurück (Alt+←)", () => void jumpBack());
  else if (last) button("← Zurück", "Zur Ausgangsstelle des Sprungs (Alt+←)", () => void jumpBack());
  if (state.forward.length > 0) button("Vorwärts →", "Den letzten Sprung wiederholen", jumpForward);
}

if (typeof document !== "undefined" && typeof window !== "undefined") {
  document.addEventListener("click", (event) => {
    const link = event.target instanceof Element ? event.target.closest<HTMLAnchorElement>("a[data-jump]") : null;
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    if (!onWorkspace()) return; // ordinary link: the page loads with find/at
    event.preventDefault();
    const origin = link.dataset.jumpOrigin;
    save(pushJump(load(), origin ? { url: here(), origin } : { url: here() }));
    go(link.getAttribute("href") ?? link.href);
  });
  // After the redraw (workspace.ts) mark the pending place found.
  document.addEventListener("baruch:rendered", () => {
    if (!pending) return;
    const found = pending;
    pending = null;
    // Typeset view or text field have just been mounted; wait one tick until the widgets stand.
    setTimeout(() => announce(found), 0);
  });
  const start = () => {
    renderBar(load());
    const found = findOf(window.location.href);
    if (found) setTimeout(() => announce(found), 50);
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
}
