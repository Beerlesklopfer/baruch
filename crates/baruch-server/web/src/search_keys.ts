// SPDX-License-Identifier: AGPL-3.0-or-later
// Key bindings of the search following VS Code (Round 30a, contract §3.1): pure function keyAction(e, ctx) -> Action
// | null, without DOM (checked under Node, tests/round30_suchen.rs). search.ts calls it in the keydown listener on the
// document.
//
// - Ctrl+F / Ctrl+H: search or replace in the section (only on the workspace page, not in the chat input field);
// - Ctrl+Shift+F / Ctrl+Shift+H: work search or work replace (sidebar; outside the workspace page
//   search.ts navigates to /suche);
// - Enter (in the search bar) or F3 (bar open): next hit; Shift+Enter / Shift+F3: previous;
// - Ctrl+Shift+1: replace the current hit; Ctrl+Alt+Enter: replace all (bar open);
// - Alt+C, Alt+W, Alt+R: case, whole word, regular expression – only with the focus in the search bar;
// - Alt+L: search only in the selection (on/off; the selection in the source text when switching on) – likewise;
// - Escape: close the bar.
// Ctrl also counts as Cmd (macOS, metaKey). Letters through `code` (KeyF …) or `key` so that layouts with another
// shift level (Shift+1 = „!“) work.

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
  /// Focus in the editor (typeset view or text field of the section).
  inEditor: boolean;
  /// Focus in the search bar (input or buttons).
  inSearch: boolean;
  /// On the workspace page (with editor).
  onWorkspace: boolean;
  /// Focus in the chat input field (there Ctrl+F stays with the browser).
  inChat?: boolean;
  /// Search bar opened.
  searchOpen?: boolean;
}

/// Letter of the key (lower case), through `code` (KeyF → f) or `key`.
function letter(e: KeyLike): string {
  if (e.code && /^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  return e.key.length === 1 ? e.key.toLowerCase() : "";
}

/// Action for the key or `null` (then the key stays with the browser or the editor).
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
