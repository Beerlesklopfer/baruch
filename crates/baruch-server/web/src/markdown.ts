// SPDX-License-Identifier: AGPL-3.0-or-later
// Markdown headings while typing (user decision 2026-11-07): `# Titel` … `###### Titel` at the
// start of a line becomes a Typst heading (`=` × level, levels 4–6 limited to 3). Authoritative is the
// rule of the server (`baruch_typst::markdown`, on saving also without JavaScript); here the same cases
// for the conversion live in the source editor and in the typeset view. In doubt the widget does not convert –
// the server decides on saving.

/// Deepest heading level of the outline.
const MAX_LEVEL = 3;

/// Markdown heading of a line (without line end): level (limited) and title, otherwise `null`.
/// Exceptions as in the server: `\#`, quotation marks, Typst code (`#heading`, `#set`, `#let`), indentation,
/// without a title, more than six hash signs.
export function markdownHeading(line: string): { level: number; title: string } | null {
  const match = /^(#{1,6}) (.*)$/.exec(line);
  if (!match) return null;
  let title = match[2].trim();
  // A closing sequence „ ##“ is dropped (only after whitespace, as in Markdown).
  const closing = /^(.*?)[ \t]+#+$|^#+$/.exec(title);
  if (closing) title = (closing[1] ?? "").trim();
  if (!title) return null;
  return { level: Math.min(match[1].length, MAX_LEVEL), title };
}

/// The line as a Typst heading, otherwise `null`.
export function typstHeadingLine(line: string): string | null {
  const found = markdownHeading(line);
  return found ? `${"=".repeat(found.level)} ${found.title}` : null;
}

/// Does the line that begins at `offset` start in top-level markup? A cautious approximation
/// of the scanner of the server, line by line: raw blocks (```), block and line comments, strings
/// in brackets and open brackets before it count; in doubt `false`.
export function lineInMarkup(text: string, offset: number): boolean {
  let fence = 0;
  let comment = 0;
  let depth = 0;
  let inString = false;
  const before = text.slice(0, offset);
  const lines = before.split("\n");
  lines.pop(); // the start of the line itself (empty if `offset` is a line start)
  for (const line of lines) {
    if (fence > 0) {
      const close = /^\s*(`{3,})/.exec(line);
      if (close && close[1].length === fence) fence = 0;
      continue;
    }
    if (comment === 0 && depth === 0 && !inString) {
      const open = /^(`{3,})/.exec(line);
      if (open) {
        fence = open[1].length;
        continue;
      }
    }
    for (let i = 0; i < line.length; i++) {
      const c = line[i];
      const next = line[i + 1];
      if (comment > 0) {
        if (c === "*" && next === "/") (comment--, i++);
        else if (c === "/" && next === "*") (comment++, i++);
      } else if (inString) {
        if (c === "\\") i++;
        else if (c === '"') inString = false;
      } else if (c === "\\") {
        i++;
      } else if (c === "/" && next === "*") {
        comment++;
        i++;
      } else if (c === "/" && next === "/" && line[i - 1] !== ":") {
        break;
      } else if (c === '"' && depth > 0) {
        inString = true;
      } else if (c === "[" || c === "(" || c === "{") {
        depth++;
      } else if ((c === "]" || c === ")" || c === "}") && depth > 0) {
        depth--;
      }
    }
  }
  return fence === 0 && comment === 0 && depth === 0 && !inString;
}

/// Source editor: convert the line at the cursor (on Enter or on leaving). The cursor stays
/// at the same place in the title. `true` if the text has changed.
export function convertLineAt(area: HTMLTextAreaElement, caret = area.selectionStart): boolean {
  const value = area.value;
  const start = value.lastIndexOf("\n", caret - 1) + 1;
  const endAt = value.indexOf("\n", caret);
  const end = endAt < 0 ? value.length : endAt;
  const line = value.slice(start, end).replace(/\r$/, "");
  const heading = typstHeadingLine(line);
  if (heading === null || !lineInMarkup(value, start)) return false;
  const oldPrefix = line.indexOf(" ") + 1;
  const newPrefix = heading.indexOf(" ") + 1;
  const inLine = Math.min(heading.length, Math.max(newPrefix, caret - start + newPrefix - oldPrefix));
  area.setRangeText(heading, start, start + line.length, "preserve");
  area.setSelectionRange(start + inLine, start + inLine);
  return true;
}

/// Convert all lines of a text (on leaving the editor): like `markdown_headings` in the server.
export function convertText(text: string): string {
  let out = "";
  for (const raw of text.split("\n")) {
    const cr = raw.endsWith("\r") ? "\r" : "";
    const line = cr ? raw.slice(0, -1) : raw;
    const heading = typstHeadingLine(line);
    const piece = heading !== null && lineInMarkup(out, out.length) ? heading + cr : raw;
    out += piece + "\n";
  }
  return out.slice(0, -1);
}
