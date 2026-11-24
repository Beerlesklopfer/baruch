// SPDX-License-Identifier: AGPL-3.0-or-later
// Befehle an das Satzbild (Runde 18, Schnittstelle R18-S1; Runde 30a ergänzt den Befehl `xref` mit `arg` =
// JSON `{key, text, page}`: Querverweis `#link(<kN>)[…]` an der Cursorstelle, siehe xref.ts; Runde 19b `scripture`
// und `cite`, siehe unten).
// Das Menüband (ribbon.ts) und die Satzbild-Werkzeuge (sheet.ts) sprechen das Satzbild (wysiwyg.ts) nur über
// diese beiden Ereignisse an:
// - `baruch:ws-command` an `div.wysiwyg[data-widget=wysiwyg]` (ohne bubbles): Das offene Satzbild führt den
//   Befehl aus und setzt `detail.handled = true`; ist es zu, bleibt `handled` `false`, und der Absender fügt im
//   Quelltext selbst ein.
// - `baruch:ws-caret`: Bei offenem Satzbild legt wysiwyg.ts EIN Promise in `detail.wait` (Quelltext und
//   Blockgrenze am Cursor), etwa für das Bild an der Cursorstelle.

// Runde 19b: `scripture` (Bibelstelle nach der Werkvorlage, `arg` = JSON `{fn, text?}`: `fn` = Funktionsname der
// Vorlage, z. B. „bibelstelle“; ohne `text` fragt das Satzbild nach der Stelle) und `cite` (Literaturverweis
// `@schlüssel`, `arg` = Schlüssel; ohne fragt das Satzbild). Die reinen Bausteine dafür stehen hier, damit ribbon.ts,
// wysiwyg.ts und Tests dieselben nutzen (diese Datei importiert nichts).

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

/** Funktionsname der Vorlage aus dem Snippet `#name[]` (nur Buchstaben, Ziffern, `_`, `-`), sonst `null`. */
export function scriptureFunction(snippet: string): string | null {
  const match = /^#([\p{L}_][\p{L}\p{N}_-]*)\[\]$/u.exec(snippet.trim());
  return match ? match[1] : null;
}

/** Typst-Text einer Bibelstelle: `#fn[Stelle]`; Markup-Zeichen der Stelle mit `\` maskiert (dieselben wie
 *  `needs_escape` in baruch-wysiwyg), Zeilenumbrüche werden Leerzeichen, mehrere `/` einer (kein Kommentar `//`).
 *  `null`, wenn Name oder Stelle leer bzw. ungültig sind (höchstens 200 Zeichen). */
export function scriptureSource(fn: string, reference: string): string | null {
  if (!/^[\p{L}_][\p{L}\p{N}_-]*$/u.test(fn)) return null;
  const text = reference.replace(/(?:[\r\n]|\p{Zl}|\p{Zp})+/gu, " ").replace(/\/{2,}/g, "/").trim();
  if (text === "" || [...text].length > 200) return null;
  const escaped = text.replace(/[\\#*_$@<>[\]`~]/g, (c) => `\\${c}`);
  return `#${fn}[${escaped}]`;
}

/** Schlüssel eines Literaturverweises (`@schlüssel`): Buchstaben, Ziffern, `_`, `-`, `:`, `.` wie Typst-Labels; am
 *  Rand beschnitten, ein führendes `@` entfällt, ein Punkt am Ende auch (er gehörte zum Satz). `null`, wenn ungültig. */
export function citeKey(input: string): string | null {
  const key = input.trim().replace(/^@/, "").replace(/\.+$/, "");
  return key !== "" && /^[\p{L}\p{N}_:.-]+$/u.test(key) ? key : null;
}

export interface WsCommandDetail {
  cmd: WsCommandName;
  arg?: string;
  handled: boolean;
}

/** An `div.wysiwyg[data-widget=wysiwyg]` (ohne bubbles). `true`, wenn das offene Satzbild den Befehl ausgeführt hat. */
export function sendWsCommand(root: HTMLElement, cmd: WsCommandName, arg?: string): boolean {
  const detail: WsCommandDetail = { cmd, handled: false };
  if (arg !== undefined) detail.arg = arg;
  root.dispatchEvent(new CustomEvent<WsCommandDetail>(WS_COMMAND, { detail }));
  return detail.handled;
}

export const WS_CARET = "baruch:ws-caret";

/**
 * Bei offenem Satzbild legt wysiwyg.ts EIN Promise in `wait`: vollständiger Quelltext wie bei
 * `baruch:serialize-text` und die UTF-16-Stelle einer Blockgrenze (Zeilenanfang) hinter dem obersten Block am Cursor.
 */
export interface WsCaretDetail {
  wait: Promise<{ body: string; offset: number }>[];
}

// ---- Runde 31c: Registermarken ---------------------------------------------------------------------------------
// Befehl `index_mark` mit `arg` = JSON `{op: "add" | "edit" | "remove", mark?}`: `add` setzt die Marke direkt hinter
// das markierte Wort (bzw. das Wort am Cursor), `edit`/`remove` ändern bzw. entfernen die Marke, die das Kontextmenü
// mit `data-ctx-target` bezeichnet hat. Die Bausteine unten gelten gleich für Satzbild und Quelltext; `markSource`,
// `validateMark` und `parseMarkAt` folgen `mark_source`, `IndexMark::validate` und `parse_mark` in
// `baruch_typst::index`, `markLabel` folgt `index_mark_label` in baruch-wysiwyg (Node-Vergleichstests im Server).

/** Registermarke wie `baruch_typst::index::IndexMark` (leere Zusätze fehlen). */
export interface IndexMark {
  register: string;
  term: string;
  sub?: string;
  see?: string;
  also?: string;
  sort?: string;
}

/** Arg des Befehls `index_mark`. */
export interface IndexMarkCommand {
  op: "add" | "edit" | "remove";
  mark?: IndexMark;
}

/** Eingebaute Register mit Titel (wie `builtin_registers`). */
export const BUILTIN_REGISTERS: readonly { slug: string; title: string }[] = [
  { slug: "stichwort", title: "Stichwortregister" },
  { slug: "personen", title: "Personenregister" },
  { slug: "bibel", title: "Bibelstellenregister" },
];

/** Längstes Feld einer Marke in Zeichen (`MAX_MARK_FIELD_CHARS`). */
export const MAX_MARK_FIELD_CHARS = 200;

const MARK_KEYS = ["register", "term", "sub", "see", "also", "sort"] as const;

/** `^[a-z][a-z0-9-]{0,31}$` (wie `valid_register_slug`). */
export function validRegisterSlug(slug: string): boolean {
  return /^[a-z][a-z0-9-]{0,31}$/.test(slug);
}

/** Marke ohne leere Zusätze, Felder in Typst-Reihenfolge. */
export function cleanMark(mark: IndexMark): IndexMark {
  const out: IndexMark = { register: mark.register ?? "", term: mark.term ?? "" };
  for (const key of ["sub", "see", "also", "sort"] as const) {
    const value = mark[key];
    if (typeof value === "string" && value !== "") out[key] = value;
  }
  return out;
}

/** Meldung wie `IndexMark::validate`, `null` = gültig. */
export function validateMark(mark: IndexMark): string | null {
  if (!validRegisterSlug(mark.register)) {
    return `Register „${mark.register}“: keine gültige Kennung (Kleinbuchstabe am Anfang, dann Kleinbuchstaben, Ziffern oder Bindestrich, höchstens 32 Zeichen).`;
  }
  if (mark.term.trim() === "") return "Stichwort: darf nicht leer sein.";
  const fields: [string, string | undefined][] = [
    ["Stichwort", mark.term],
    ["Unterstichwort", mark.sub],
    ["siehe", mark.see],
    ["siehe auch", mark.also],
    ["Sortierschlüssel", mark.sort],
  ];
  for (const [label, value] of fields) {
    const text = value ?? "";
    if ([...text].length > MAX_MARK_FIELD_CHARS) return `${label}: höchstens ${MAX_MARK_FIELD_CHARS} Zeichen.`;
    if (/\p{Cc}/u.test(text)) return `${label}: ohne Zeilenumbruch und Steuerzeichen.`;
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

/** Kanonische Typst-Form wie `mark_source`: `#metadata((register: "…", term: "…"[, sub: "…"]…))<baruch-idx>`. */
export function markSource(mark: IndexMark): string {
  const parts: string[] = [];
  for (const key of MARK_KEYS) {
    const value = mark[key] ?? "";
    if (value === "" && key !== "register" && key !== "term") continue;
    parts.push(`${key}: ${typstString(value)}`);
  }
  return `#metadata((${parts.join(", ")}))<baruch-idx>`;
}

/** Titel eines Registers: eingebaut mit Titel, sonst aus `titles` (eigene Register), sonst „Register „slug““. */
export function registerTitle(slug: string, titles: Record<string, string> = {}): string {
  const builtin = BUILTIN_REGISTERS.find((r) => r.slug === slug);
  if (builtin) return builtin.title;
  return titles[slug] ?? `Register „${slug}“`;
}

/** Beschreibung einer Marke (title/aria-label) wie `index_mark_label` in baruch-wysiwyg. */
export function markLabel(mark: IndexMark): string {
  let label = `Registereintrag im ${registerTitle(mark.register)}: ${mark.term}`;
  if (mark.sub) label += ` › ${mark.sub}`;
  if (mark.see) label += `; siehe: ${mark.see}`;
  if (mark.also) label += `; siehe auch: ${mark.also}`;
  return label;
}

/** Liest eine kanonische Marke ab `at` wie `parse_mark` (Leerraum um `:`/`,` und innen an den Klammern erlaubt,
 *  Schlüssel in fester Reihenfolge, register und term Pflicht, kein Leerzeichen vor dem Label, gültig nach
 *  `validateMark`). `end` = Stelle hinter der Marke (UTF-16); sonst `null`. */
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

/** Alle kanonischen Marken im Text (Quelltext; Kommentare und Rohtext werden nicht ausgenommen). */
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

/** Wortzeichen (Buchstaben, Ziffern, Bindestrich, Apostroph im Wort). */
const WORD = /[\p{L}\p{N}\p{M}_'’-]/u;

/** Wort um die Stelle `pos` (Grenzen in UTF-16); kein Wort → leer an `pos`. */
export function wordAt(text: string, pos: number): { start: number; end: number; word: string } {
  let start = Math.max(0, Math.min(pos, text.length));
  let end = start;
  while (start > 0 && WORD.test(text[start - 1])) start--;
  while (end < text.length && WORD.test(text[end])) end++;
  return { start, end, word: text.slice(start, end) };
}

/** Auswahl im Quelltext → markiertes Wort: Leerraum am Rand fällt weg, eine leere Auswahl nimmt das Wort am Cursor. */
export function markedWord(text: string, start: number, end: number): { start: number; end: number; word: string } {
  if (end <= start) return wordAt(text, start);
  let a = start;
  let b = end;
  while (a < b && /\s/.test(text[a])) a++;
  while (b > a && /\s/.test(text[b - 1])) b--;
  return { start: a, end: b, word: text.slice(a, b) };
}

/** Meldung, warum an der Stelle `pos` keine Marke stehen darf (Überschriftszeile, Kommentar, in einer Marke), sonst
 *  `null` – die Regeln von `insert_mark_at` im Server, soweit sie sich ohne Typst-Leser prüfen lassen. */
export function markRefusal(text: string, pos: number): string | null {
  const lineStart = text.lastIndexOf("\n", pos - 1) + 1;
  const lineEndAt = text.indexOf("\n", pos);
  const line = text.slice(lineStart, lineEndAt < 0 ? text.length : lineEndAt);
  if (/^\s*=/.test(line) || line.includes("<baruch-part>") || line.includes("<baruch-unnumbered>")) {
    return "In einer Überschriftszeile steht keine Registermarke; bitte ein Wort im Fließtext wählen.";
  }
  if (text.slice(lineStart, pos).includes("//")) return "In einem Kommentar steht keine Registermarke.";
  if (findMarks(text).some((m) => m.start < pos && pos < m.end)) return "Hier steht schon eine Registermarke.";
  return null;
}

/** Marke hinter das markierte Wort in den Quelltext setzen: neuer Text, Einfügestelle, eingefügter Text und Cursor
 *  hinter der Marke – oder eine Meldung. */
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
  if (word.word === "") return { error: "Bitte ein Wort markieren oder den Cursor in ein Wort setzen." };
  const refusal = markRefusal(text, word.end);
  if (refusal) return { error: refusal };
  // Nie dieselbe Marke zweimal hinter dasselbe Wort (wie `mark_word` im Server, Nachbesserung R31).
  for (let pos = word.end, found = parseMarkAt(text, pos); found; pos = found.end, found = parseMarkAt(text, pos)) {
    if (MARK_KEYS.every((k) => (found!.mark[k] ?? "") === (clean[k] ?? ""))) {
      return { error: "Dieses Wort trägt diese Registermarke schon; nichts geändert." };
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
