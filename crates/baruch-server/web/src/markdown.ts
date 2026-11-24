// SPDX-License-Identifier: AGPL-3.0-or-later
// Markdown-Überschriften beim Tippen (Nutzerentscheidung 2026-11-07): `# Titel` … `###### Titel` am
// Zeilenanfang wird zur Typst-Überschrift (`=` × Ebene, Ebene 4–6 auf 3 begrenzt). Maßgeblich ist die
// Regel des Servers (`baruch_typst::markdown`, beim Speichern auch ohne JavaScript); hier dieselben Fälle
// für die Umwandlung live im Quelltext-Editor und im Satzbild. Im Zweifel wandelt das Widget nicht um –
// der Server entscheidet beim Speichern.

/// Tiefste Überschriftsebene der Gliederung.
const MAX_LEVEL = 3;

/// Markdown-Überschrift einer Zeile (ohne Zeilenende): Ebene (begrenzt) und Titel, sonst `null`.
/// Ausnahmen wie im Server: `\#`, Anführungszeichen, Typst-Code (`#heading`, `#set`, `#let`), Einzug,
/// ohne Titel, mehr als sechs Rauten.
export function markdownHeading(line: string): { level: number; title: string } | null {
  const match = /^(#{1,6}) (.*)$/.exec(line);
  if (!match) return null;
  let title = match[2].trim();
  // Schließende Folge „ ##“ entfällt (nur nach Leerraum, wie in Markdown).
  const closing = /^(.*?)[ \t]+#+$|^#+$/.exec(title);
  if (closing) title = (closing[1] ?? "").trim();
  if (!title) return null;
  return { level: Math.min(match[1].length, MAX_LEVEL), title };
}

/// Die Zeile als Typst-Überschrift, sonst `null`.
export function typstHeadingLine(line: string): string | null {
  const found = markdownHeading(line);
  return found ? `${"=".repeat(found.level)} ${found.title}` : null;
}

/// Beginnt die Zeile, die bei `offset` anfängt, im Markup der obersten Ebene? Vorsichtige Näherung
/// an den Scanner des Servers, Zeile für Zeile: Raw-Blöcke (```), Block- und Zeilenkommentare, Strings
/// in Klammern und offene Klammern davor zählen; im Zweifel `false`.
export function lineInMarkup(text: string, offset: number): boolean {
  let fence = 0;
  let comment = 0;
  let depth = 0;
  let inString = false;
  const before = text.slice(0, offset);
  const lines = before.split("\n");
  lines.pop(); // der Anfang der Zeile selbst (leer, wenn `offset` ein Zeilenanfang ist)
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

/// Quelltext-Editor: die Zeile am Cursor umwandeln (beim Enter bzw. beim Verlassen). Der Cursor bleibt
/// an derselben Stelle im Titel. `true`, wenn sich der Text geändert hat.
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

/// Alle Zeilen eines Texts umwandeln (beim Verlassen des Editors): wie `markdown_headings` im Server.
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
