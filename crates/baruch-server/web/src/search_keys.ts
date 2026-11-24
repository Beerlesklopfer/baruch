// SPDX-License-Identifier: AGPL-3.0-or-later
// Tastenbelegung der Suche nach VS Code (Runde 30a, Vertrag §3.1): reine Funktion keyAction(e, ctx) -> Action | null,
// ohne DOM (unter Node geprüft, tests/round30_suchen.rs). search.ts ruft sie im keydown-Listener am Dokument.
//
// - Strg+F / Strg+H: Suchen bzw. Ersetzen im Abschnitt (nur auf der Arbeitsseite, nicht im Chat-Eingabefeld);
// - Strg+Umschalt+F / Strg+Umschalt+H: Werk-Suche bzw. Werk-Ersetzen (Seitenleiste; außerhalb der Arbeitsseite
//   navigiert search.ts nach /suche);
// - Enter (in der Suchleiste) oder F3 (Leiste offen): nächster Treffer; Umschalt+Enter / Umschalt+F3: voriger;
// - Strg+Umschalt+1: aktuellen Treffer ersetzen; Strg+Alt+Enter: alle ersetzen (Leiste offen);
// - Alt+C, Alt+W, Alt+R: Groß/klein, ganzes Wort, regulärer Ausdruck – nur mit Fokus in der Suchleiste;
// - Alt+L: nur in der Markierung suchen (ein/aus; die Markierung im Quelltext beim Einschalten) – ebenso;
// - Escape: Leiste schließen.
// Strg gilt auch als Cmd (macOS, metaKey). Buchstaben über `code` (KeyF …) oder `key`, damit Belegungen mit anderer
// Umschalt-Ebene (Umschalt+1 = „!“) gehen.

export type Action =
  | "find-section"
  | "replace-section"
  | "find-work"
  | "replace-work"
  | "next"
  | "prev"
  | "replace-one"
  | "replace-all"
  | "toggle-case"
  | "toggle-word"
  | "toggle-regex"
  | "toggle-selection"
  | "close";

export interface KeyLike {
  key: string;
  code?: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

export interface KeyContext {
  /// Fokus im Editor (Satzbild oder Textfeld des Abschnitts).
  inEditor: boolean;
  /// Fokus in der Suchleiste (Eingabe oder Knöpfe).
  inSearch: boolean;
  /// Auf der Arbeitsseite (mit Editor).
  onWorkspace: boolean;
  /// Fokus im Chat-Eingabefeld (dort bleibt Strg+F dem Browser).
  inChat?: boolean;
  /// Suchleiste geöffnet.
  searchOpen?: boolean;
}

/// Buchstabe der Taste (klein), über `code` (KeyF → f) oder `key`.
function letter(e: KeyLike): string {
  if (e.code && /^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  return e.key.length === 1 ? e.key.toLowerCase() : "";
}

/// Aktion zur Taste oder `null` (dann bleibt die Taste dem Browser bzw. dem Editor).
export function keyAction(e: KeyLike, ctx: KeyContext): Action | null {
  const ctrl = e.ctrlKey || e.metaKey;
  const open = ctx.searchOpen === true || ctx.inSearch;
  const l = letter(e);
  if (ctrl && !e.altKey && e.shiftKey) {
    if (l === "f") return "find-work";
    if (l === "h") return "replace-work";
    if (open && (e.code === "Digit1" || e.key === "1" || e.key === "!")) return "replace-one";
    return null;
  }
  if (ctrl && !e.altKey && !e.shiftKey) {
    if ((l === "f" || l === "h") && ctx.onWorkspace && !ctx.inChat) return l === "f" ? "find-section" : "replace-section";
    return null;
  }
  if (ctrl && e.altKey && !e.shiftKey && e.key === "Enter") return open ? "replace-all" : null;
  if (e.altKey && !ctrl && !e.shiftKey && ctx.inSearch) {
    if (l === "c") return "toggle-case";
    if (l === "w") return "toggle-word";
    if (l === "r") return "toggle-regex";
    if (l === "l") return "toggle-selection";
    return null;
  }
  if (ctrl || e.altKey) return null;
  if (e.key === "F3" && open) return e.shiftKey ? "prev" : "next";
  if (e.key === "Enter" && ctx.inSearch) return e.shiftKey ? "prev" : "next";
  if (e.key === "Escape" && open) return "close";
  return null;
}
