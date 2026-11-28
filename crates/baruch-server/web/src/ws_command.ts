// SPDX-License-Identifier: AGPL-3.0-or-later
// Commands to the typeset view (Round 18, interface R18-S1; Round 30a adds the command `xref` with `arg` =
// JSON `{key, text, page}`: cross-reference `#link(<kN>)[…]` at the cursor position, see xref.ts; Round 19b `scripture`
// and `cite`, see below).
// The ribbon (ribbon.ts) and the typeset-view tools (sheet.ts) address the typeset view (wysiwyg.ts) only through
// these two events:
// - `baruch:ws-command` on `div.wysiwyg[data-widget=wysiwyg]` (without bubbles): the open typeset view carries out
//   the command and sets `detail.handled = true`; if it is closed, `handled` stays `false`, and the sender inserts
//   in the source text itself.
// - `baruch:ws-caret`: with the typeset view open wysiwyg.ts puts ONE promise into `detail.wait` (source text and
//   block boundary at the cursor), e.g. for the image at the cursor position.

import { t } from "./i18n.js";






// Round 19b: `scripture` (Bible passage following the work template, `arg` = JSON `{fn, text?}`: `fn` = function name
// of the template, e.g. „bibelstelle“; without `text` the typeset view asks for the passage) and `cite` (literature
// reference `@schlüssel`, `arg` = key; without it the typeset view asks). The pure building blocks for this stand
// here so that ribbon.ts, wysiwyg.ts and tests use the same ones (this file imports nothing).

export const WS_COMMAND = "baruch:ws-command";

export type WsCommandName =
  | "scene"
  | "work"
  | "scripture"
  | "cite"
  | "footnote"
  | "quote"
  | "list"
  | "enum"
  | "terms"
  | "break"
  | "link"
  | "xref"
  | "index_mark";

/** Function name of the template from the snippet `#name[]` (only letters, digits, `_`, `-`), otherwise `null`. */
export function scriptureFunction(snippet: string): string | null {
  const match = /^#([\p{L}_][\p{L}\p{N}_-]*)\[\]$/u.exec(snippet.trim());
  return match ? match[1] : null;
}

/** Typst text of a Bible passage: `#fn[Stelle]`; markup characters of the passage escaped with `\` (the same as
 *  `needs_escape` in baruch-wysiwyg), line breaks become spaces, several `/` become one (no comment `//`).
 *  `null` if name or passage are empty or invalid (at most 200 characters). */
export function scriptureSource(fn: string, reference: string): string | null {
  if (!/^[\p{L}_][\p{L}\p{N}_-]*$/u.test(fn)) return null;
  const text = reference.replace(/(?:[\r\n]|\p{Zl}|\p{Zp})+/gu, " ").replace(/\/{2,}/g, "/").trim();
  if (text === "" || [...text].length > 200) return null;
  const escaped = text.replace(/[\\#*_$@<>[\]`~]/g, (c) => `\\${c}`);
  return `#${fn}[${escaped}]`;
}

/** Key of a literature reference (`@schlüssel`): letters, digits, `_`, `-`, `:`, `.` like Typst labels; trimmed
 *  at the edges, a leading `@` is dropped, a full stop at the end too (it belonged to the sentence). `null` if invalid.
 *  */
export function citeKey(input: string): string | null {
  const key = input.trim().replace(/^@/, "").replace(/\.+$/, "");
  return key !== "" && /^[\p{L}\p{N}_:.-]+$/u.test(key) ? key : null;
}

export interface WsCommandDetail {
  cmd: WsCommandName;
  arg?: string;
  handled: boolean;
}

/** On `div.wysiwyg[data-widget=wysiwyg]` (without bubbles). `true` if the open typeset view has carried out the
command. */
export function sendWsCommand(root: HTMLElement, cmd: WsCommandName, arg?: string): boolean {
  const detail: WsCommandDetail = { cmd, handled: false };
  if (arg !== undefined) detail.arg = arg;
  root.dispatchEvent(new CustomEvent<WsCommandDetail>(WS_COMMAND, { detail }));
  return detail.handled;
}

export const WS_CARET = "baruch:ws-caret";

/**
 * With the typeset view open wysiwyg.ts puts ONE promise into `wait`: complete source text as with
 * `baruch:serialize-text` and the UTF-16 position of a block boundary (line start) behind the topmost block at the
 * cursor.
 */
export interface WsCaretDetail {
  wait: Promise<{ body: string; offset: number }>[];
}

// ---- Round 31c: index marks ------------------------------------------------------------------------------------
// Command `index_mark` with `arg` = JSON `{op: "add" | "edit" | "remove", mark?}`: `add` puts the mark directly behind
// the selected word (or the word at the cursor), `edit`/`remove` change or remove the mark that the context menu
// has designated with `data-ctx-target`. The building blocks below apply equally to typeset view and source text;
// `markSource`, `validateMark` and `parseMarkAt` follow `mark_source`, `IndexMark::validate` and `parse_mark` in
// `baruch_typst::index`, `markLabel` follows `index_mark_label` in baruch-wysiwyg (Node comparison tests in the
// server).

/** Index mark like `baruch_typst::index::IndexMark` (empty additions are missing). */
export interface IndexMark {
  register: string;
  term: string;
  sub?: string;
  see?: string;
  also?: string;
  sort?: string;
}

/** Arg of the command `index_mark`. */
export interface IndexMarkCommand {
  op: "add" | "edit" | "remove";
  mark?: IndexMark;
}

/** Built-in indexes with title (like `builtin_registers`). */
export const BUILTIN_REGISTERS: readonly { slug: string; title: string }[] = [
  { slug: "stichwort", get title() { return t("widget.ws_command.register_stichwort"); } },
  { slug: "personen", get title() { return t("widget.ws_command.register_personen"); } },
  { slug: "bibel", get title() { return t("widget.ws_command.register_bibel"); } },
];

/** Longest field of a mark in characters (`MAX_MARK_FIELD_CHARS`). */
export const MAX_MARK_FIELD_CHARS = 200;

const MARK_KEYS = ["register", "term", "sub", "see", "also", "sort"] as const;

/** `^[a-z][a-z0-9-]{0,31}$` (like `valid_register_slug`). */
export function validRegisterSlug(slug: string): boolean {
  return /^[a-z][a-z0-9-]{0,31}$/.test(slug);
}

/** Mark without empty additions, fields in Typst order. */
export function cleanMark(mark: IndexMark): IndexMark {
  const out: IndexMark = { register: mark.register ?? "", term: mark.term ?? "" };
  for (const key of ["sub", "see", "also", "sort"] as const) {
    const value = mark[key];
    if (typeof value === "string" && value !== "") out[key] = value;
  }
  return out;
}

/** Message like `IndexMark::validate`, `null` = valid. */
export function validateMark(mark: IndexMark): string | null {
  if (!validRegisterSlug(mark.register)) {
    return t("widget.ws_command.bad_slug", { slug: mark.register });
  }
  if (mark.term.trim() === "") return t("widget.ws_command.term_empty");
  const fields: [string, string | undefined][] = [
    [t("widget.ws_command.field_term"), mark.term],
    [t("widget.ws_command.field_sub"), mark.sub],
    [t("widget.ws_command.field_see"), mark.see],
    [t("widget.ws_command.field_also"), mark.also],
    [t("widget.ws_command.field_sort"), mark.sort],
  ];
  for (const [label, value] of fields) {
    const text = value ?? "";
    if ([...text].length > MAX_MARK_FIELD_CHARS) return t("widget.ws_command.too_long", { label, max: MAX_MARK_FIELD_CHARS });
    if (/\p{Cc}/u.test(text)) return t("widget.ws_command.control_chars", { label });
  }
  return null;
}

function typstString(text: string): string {
  let out = '"';
  for (const c of text) {
    if (c === '"') out += '\\"';
    else if (c === "\\") out += "\\\\";
    else if (c === "\n") out += "\\n";
    else if (c === "\t") out += "\\t";
    else if (c === "\r") out += "\\r";
    else if (/\p{Cc}/u.test(c)) out += `\\u{${(c.codePointAt(0) ?? 0).toString(16)}}`;
    else out += c;
  }
  return `${out}"`;
}

/** Canonical Typst form like `mark_source`: `#metadata((register: "…", term: "…"[, sub: "…"]…))<baruch-idx>`. */
export function markSource(mark: IndexMark): string {
  const parts: string[] = [];
  for (const key of MARK_KEYS) {
    const value = mark[key] ?? "";
    if (value === "" && key !== "register" && key !== "term") continue;
    parts.push(`${key}: ${typstString(value)}`);
  }
  return `#metadata((${parts.join(", ")}))<baruch-idx>`;
}

/** Title of an index: built-in with title, otherwise from `titles` (own indexes), otherwise „Register „slug““. */
export function registerTitle(slug: string, titles: Record<string, string> = {}): string {
  const builtin = BUILTIN_REGISTERS.find((r) => r.slug === slug);
  if (builtin) return builtin.title;
  return titles[slug] ?? t("widget.ws_command.register_named", { slug });
}

/** Description of a mark (title/aria-label) like `index_mark_label` in baruch-wysiwyg. */
export function markLabel(mark: IndexMark): string {
  let label = t("widget.ws_command.mark_label", { register: registerTitle(mark.register), term: mark.term });
  if (mark.sub) label += ` › ${mark.sub}`;
  if (mark.see) label += `; ${t("widget.ws_command.mark_see", { see: mark.see })}`;
  if (mark.also) label += `; ${t("widget.ws_command.mark_also", { also: mark.also })}`;
  return label;
}

/** Reads a canonical mark from `at` like `parse_mark` (whitespace around `:`/`,` and inside at the brackets allowed,
 *  keys in fixed order, register and term mandatory, no space before the label, valid by
 *  `validateMark`). `end` = position behind the mark (UTF-16); otherwise `null`. */
export function parseMarkAt(text: string, at: number): { mark: IndexMark; end: number } | null {
  let pos = at;
  const eat = (token: string) => {
    if (text.startsWith(token, pos)) {
      pos += token.length;
      return true;
    }
    return false;
  };
  const space = () => {
    while (text[pos] === " " || text[pos] === "\t") pos++;
  };
  const string = (): string | null => {
    if (!eat('"')) return null;
    let out = "";
    while (pos < text.length) {
      const c = text[pos++];
      if (c === '"') return out;
      if (c === "\n" || c === "\r") return null;
      if (c !== "\\") {
        out += c;
        continue;
      }
      const e = text[pos++];
      if (e === '"') out += '"';
      else if (e === "\\") out += "\\";
      else if (e === "n") out += "\n";
      else if (e === "t") out += "\t";
      else if (e === "r") out += "\r";
      else if (e === "u") {
        const m = /^\{([0-9a-fA-F]{1,6})\}/.exec(text.slice(pos, pos + 9));
        if (!m) return null;
        const code = parseInt(m[1], 16);
        if (code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return null;
        out += String.fromCodePoint(code);
        pos += m[0].length;
      } else return null;
    }
    return null;
  };
  if (!eat("#metadata((")) return null;
  const mark: IndexMark = { register: "", term: "" };
  const seen = new Set<string>();
  let next = 0;
  space();
  for (;;) {
    if (eat(")")) break;
    const ident = /^[a-z]+/.exec(text.slice(pos, pos + 16));
    if (!ident) return null;
    pos += ident[0].length;
    const index = (MARK_KEYS as readonly string[]).indexOf(ident[0]);
    if (index < next) return null;
    next = index + 1;
    seen.add(ident[0]);
    space();
    if (!eat(":")) return null;
    space();
    const value = string();
    if (value === null) return null;
    mark[MARK_KEYS[index]] = value;
    space();
    if (eat(",")) {
      space();
      continue;
    }
    if (!eat(")")) return null;
    break;
  }
  if (!seen.has("register") || !seen.has("term")) return null;
  if (!eat(")") || !eat("<baruch-idx>")) return null;
  const clean = cleanMark(mark);
  if (validateMark(clean) !== null) return null;
  return { mark: clean, end: pos };
}

/** All canonical marks in the text (source text; comments and raw text are not excluded). */
export function findMarks(text: string): { start: number; end: number; mark: IndexMark }[] {
  const out: { start: number; end: number; mark: IndexMark }[] = [];
  let from = 0;
  for (;;) {
    const start = text.indexOf("#metadata((", from);
    if (start < 0) return out;
    const found = parseMarkAt(text, start);
    if (found) {
      out.push({ start, end: found.end, mark: found.mark });
      from = found.end;
    } else from = start + 1;
  }
}

/** Word characters (letters, digits, hyphen, apostrophe in the word). */
const WORD = /[\p{L}\p{N}\p{M}_'’-]/u;

/** Word around the position `pos` (boundaries in UTF-16); no word → empty at `pos`. */
export function wordAt(text: string, pos: number): { start: number; end: number; word: string } {
  let start = Math.max(0, Math.min(pos, text.length));
  let end = start;
  while (start > 0 && WORD.test(text[start - 1])) start--;
  while (end < text.length && WORD.test(text[end])) end++;
  return { start, end, word: text.slice(start, end) };
}

/** Selection in the source text → selected word: whitespace at the edge drops, an empty selection takes the word at the
cursor. */
export function markedWord(text: string, start: number, end: number): { start: number; end: number; word: string } {
  if (end <= start) return wordAt(text, start);
  let a = start;
  let b = end;
  while (a < b && /\s/.test(text[a])) a++;
  while (b > a && /\s/.test(text[b - 1])) b--;
  return { start: a, end: b, word: text.slice(a, b) };
}

/** Message why no mark may stand at the position `pos` (heading line, comment, inside a mark), otherwise
 *  `null` – the rules of `insert_mark_at` in the server as far as they can be checked without a Typst reader. */
export function markRefusal(text: string, pos: number): string | null {
  const lineStart = text.lastIndexOf("\n", pos - 1) + 1;
  const lineEndAt = text.indexOf("\n", pos);
  const line = text.slice(lineStart, lineEndAt < 0 ? text.length : lineEndAt);
  if (/^\s*=/.test(line) || line.includes("<baruch-part>") || line.includes("<baruch-unnumbered>")) {
    return t("widget.ws_command.refuse_heading");
  }
  if (text.slice(lineStart, pos).includes("//")) return t("widget.ws_command.refuse_comment");
  if (findMarks(text).some((m) => m.start < pos && pos < m.end)) return t("widget.ws_command.refuse_inside");
  return null;
}

/** Put a mark behind the selected word into the source text: new text, insertion position, inserted text and cursor
 *  behind the mark – or a message. */
export function markInSource(
  text: string,
  start: number,
  end: number,
  mark: IndexMark,
): { text: string; at: number; caret: number; insert: string } | { error: string } {
  const clean = cleanMark(mark);
  const problem = validateMark(clean);
  if (problem) return { error: problem };
  const word = markedWord(text, start, end);
  if (word.word === "") return { error: t("widget.ws_command.no_word") };
  const refusal = markRefusal(text, word.end);
  if (refusal) return { error: refusal };
  // Never the same mark twice behind the same word (like `mark_word` in the server, follow-up fix R31).
  for (let pos = word.end, found = parseMarkAt(text, pos); found; pos = found.end, found = parseMarkAt(text, pos)) {
    if (MARK_KEYS.every((k) => (found!.mark[k] ?? "") === (clean[k] ?? ""))) {
      return { error: t("widget.ws_command.duplicate") };
    }
  }
  const insert = markSource(clean);
  return {
    text: text.slice(0, word.end) + insert + text.slice(word.end),
    at: word.end,
    caret: word.end + insert.length,
    insert,
  };
}
