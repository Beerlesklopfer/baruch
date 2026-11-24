// SPDX-License-Identifier: AGPL-3.0-or-later
// Sprünge zu Stellen (Runde 30a, Vertrag §3.2): find/at aus der Adresse, a[data-jump], Sprungverlauf, Alt+←.
//
// - Sprung-Adresse `/?work=W&key=K[&rev=R]&find=<Text>[&at=<Byte>]` (Server: crate::xref::jump_url). Der Server
//   ignoriert find/at; hier wird daraus das Ereignis `baruch:find {text, at}` am Dokument, auf das wysiwyg.ts
//   (Satzbild, CSS Custom Highlight API) und search.ts (Quelltext, Auswahl im Textfeld) hören – beim Laden der Seite
//   und nach jedem Neuzeichnen durch workspace.ts (`baruch:rendered`).
// - Jedes `a[data-jump]` öffnet auf der Arbeitsseite den Abschnitt ohne Seitenwechsel (`baruch:navigate`); die
//   Ausgangsstelle kommt in den Sprungverlauf (sessionStorage `baruch_jumps`, nur in try/catch). Mit
//   `data-jump-origin="chat"` erscheint danach der Knopf „Zurück zum Chat“. Außerhalb der Arbeitsseite bleibt der
//   Link ein gewöhnlicher Link (die Seite lädt mit find/at).
// - Alt+← (workspace.ts): ist der Verlauf nicht leer, zurück zur Ausgangsstelle ([`popJump`]); sonst wie bisher.
//   „Vorwärts“ gibt es nur als Knopf in der Sprungleiste.
// Keine Importe: Die reinen Funktionen (Verlauf, Byte → UTF-16) laufen unter Node (tests/round30_verweise.rs).

export interface JumpEntry {
  url: string;
  origin?: string;
}

export interface JumpState {
  back: JumpEntry[];
  forward: JumpEntry[];
}

/// Höchstzahl gemerkter Sprünge.
export const MAX_JUMPS = 50;

export function emptyState(): JumpState {
  return { back: [], forward: [] };
}

/// Sprung von `from` aus: Ausgangsstelle in den Verlauf, „Vorwärts“ verfällt.
export function pushJump(state: JumpState, from: JumpEntry): JumpState {
  const back = [...state.back, from].slice(-MAX_JUMPS);
  return { back, forward: [] };
}

/// Zurück: letzte Ausgangsstelle und neuer Stand (die aktuelle Stelle `here` kommt nach „Vorwärts“); leer → null.
export function popJump(state: JumpState, here: string): { state: JumpState; entry: JumpEntry } | null {
  const entry = state.back[state.back.length - 1];
  if (!entry) return null;
  return { state: { back: state.back.slice(0, -1), forward: [...state.forward, { url: here }].slice(-MAX_JUMPS) }, entry };
}

/// Vorwärts (nur über den Knopf): umgekehrt zu [`popJump`].
export function forwardJump(state: JumpState, here: string): { state: JumpState; entry: JumpEntry } | null {
  const entry = state.forward[state.forward.length - 1];
  if (!entry) return null;
  return { state: { back: [...state.back, { url: here }].slice(-MAX_JUMPS), forward: state.forward.slice(0, -1) }, entry };
}

/// UTF-16-Stelle des Bytes `byte` (UTF-8) in `text`; hinter dem Ende → Länge.
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

/// Stelle des Texts `find` in `text`, die `near` (UTF-16) am nächsten liegt; Leerraum zählt als beliebiger Leerraum.
/// Nicht gefunden → null. Ergebnis `[start, end]` in UTF-16.
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

/// find/at aus einer Adresse (oder null).
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

// ---- Seite --------------------------------------------------------------------------------------------------

const STORE_KEY = "baruch_jumps";

function load(): JumpState {
  try {
    const raw = sessionStorage.getItem(STORE_KEY);
    if (!raw) return emptyState();
    const parsed = JSON.parse(raw) as JumpState;
    if (Array.isArray(parsed.back) && Array.isArray(parsed.forward)) return parsed;
  } catch {
    // Kein Speicher (privates Fenster, gesperrt): ohne Verlauf weiter.
  }
  return emptyState();
}

function save(state: JumpState): void {
  try {
    sessionStorage.setItem(STORE_KEY, JSON.stringify(state));
  } catch {
    // Ohne Speicher bleibt der Verlauf nur bis zum Neuladen.
  }
  renderBar(state);
}

/// Ausstehende Fundstelle für das nächste Neuzeichnen.
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

/// Zu `url` springen: auf der Arbeitsseite ohne Seitenwechsel, sonst normal.
function go(url: string): void {
  pending = findOf(url);
  if (onWorkspace()) document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url } }));
  else window.location.href = url;
}

/// Zurück zur letzten Ausgangsstelle (Alt+← bei nicht leerem Verlauf); `false` = Verlauf leer.
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

/// Sprungleiste: „← Zurück“, „Vorwärts →“, „Zurück zum Chat“ (nur bei Verlauf).
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
    if (!onWorkspace()) return; // gewöhnlicher Link: die Seite lädt mit find/at
    event.preventDefault();
    const origin = link.dataset.jumpOrigin;
    save(pushJump(load(), origin ? { url: here(), origin } : { url: here() }));
    go(link.getAttribute("href") ?? link.href);
  });
  // Nach dem Neuzeichnen (workspace.ts) die ausstehende Fundstelle markieren.
  document.addEventListener("baruch:rendered", () => {
    if (!pending) return;
    const found = pending;
    pending = null;
    // Satzbild bzw. Textfeld sind eben eingehängt; einen Takt warten, bis die Widgets stehen.
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
