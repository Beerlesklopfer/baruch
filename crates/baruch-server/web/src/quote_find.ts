// SPDX-License-Identifier: AGPL-3.0-or-later
// Zitate im Satzbild wiederfinden (Nachbesserung Runde 17, für das Widget „scripture-check“): reine Funktionen ohne
// DOM, damit ein Test sie unter Node prüfen kann (tests/quote_find.rs).
//
// Der Server liefert je Stelle den Zitattext (`quote`, Typst-Quelltext) und seinen Bereich im Quelltext
// (`quote_span`, Bytes). Das Satzbild kennt keine Quellpositionen; deshalb:
// - Der Zitattext wird von Typst-Auszeichnung befreit (`#emph[…]`, `*`, `_`, `\`), wie er im Satzbild steht.
// - Gesucht wird nur nach ganzen Wörtern (kein Buchstabe und keine Ziffer direkt davor oder danach).
// - Gleicher Wortlaut an mehreren Stellen: Die Zitate werden in der Reihenfolge ihres `quote_span` den Fundstellen
//   zugeordnet – zuerst den Fundstellen in Anführungszeichen, sonst allen. Passt die Zahl nicht, bleibt das Zitat
//   unmarkiert (nicht eindeutig), statt jede Fundstelle in beiden Farben zu zeigen.

/// Text mit zusammengefasstem Leerraum und der Stelle jedes Zeichens im Ausgangstext.
export function collapse(text: string): { flat: string; at: number[] } {
  let flat = "";
  const at: number[] = [];
  let space = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (/\s/.test(c)) {
      if (!space && flat.length > 0) {
        flat += " ";
        at.push(i);
      }
      space = true;
      continue;
    }
    space = false;
    flat += c;
    at.push(i);
  }
  return { flat, at };
}

/// Zitattext ohne Typst-Auszeichnung: `#emph[x]`/`#strong[x]` → x, `\x` → x, `*` und `_` entfallen.
export function plainQuote(typst: string): string {
  let text = typst;
  for (let i = 0; i < 8; i++) {
    const next = text.replace(/#[A-Za-z]+\[([^\[\]]*)\]/g, "$1");
    if (next === text) break;
    text = next;
  }
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === "\\" && i + 1 < text.length) {
      out += text[++i];
    } else if (c !== "*" && c !== "_") {
      out += c;
    }
  }
  return out;
}

function isWordChar(c: string | undefined): boolean {
  return c !== undefined && /[\p{L}\p{N}]/u.test(c);
}

/// Alle Fundstellen von `needle` in `haystack` als ganze Wörter (Leerraum zählt als ein Leerzeichen): [Anfang, Ende)
/// im Ausgangstext.
export function findAll(haystack: string, needle: string): [number, number][] {
  const n = collapse(needle.trim()).flat;
  if (n === "") return [];
  const h = collapse(haystack);
  const out: [number, number][] = [];
  let from = 0;
  for (;;) {
    const i = h.flat.indexOf(n, from);
    if (i < 0) break;
    const before = i > 0 ? h.flat[i - 1] : undefined;
    const after = h.flat[i + n.length];
    if (!(isWordChar(n[0]) && isWordChar(before)) && !(isWordChar(n[n.length - 1]) && isWordChar(after))) {
      out.push([h.at[i], h.at[i + n.length - 1] + 1]);
      from = i + n.length;
    } else {
      from = i + 1;
    }
  }
  return out;
}

const OPENING = "„“\"»«‚‘‹›'";
const CLOSING = "“”\"«»‘’‹›'";

/// Steht die Fundstelle [s, e) in Anführungszeichen (Leerraum dazwischen zählt nicht)?
function quoted(haystack: string, s: number, e: number): boolean {
  let a = s - 1;
  while (a >= 0 && /\s/.test(haystack[a])) a--;
  let b = e;
  while (b < haystack.length && /\s/.test(haystack[b])) b++;
  return a >= 0 && b < haystack.length && OPENING.includes(haystack[a]) && CLOSING.includes(haystack[b]);
}

/// Ein Zitat, wie das Widget es zuordnet.
export interface Placeable {
  quote: string | null;
  quote_span?: [number, number] | null;
}

/// Ordnet jedem Zitat höchstens eine Fundstelle im Satzbild zu (siehe Kopf der Datei). Ergebnis in der Reihenfolge
/// von `marks`; `null` = ohne Zitat oder nicht eindeutig gefunden. Zwei Stellen mit demselben `quote_span` sind
/// dasselbe Zitat und bekommen dieselbe Fundstelle.
export function placeQuotes(haystack: string, marks: Placeable[]): ([number, number] | null)[] {
  const result: ([number, number] | null)[] = marks.map(() => null);
  const groups = new Map<string, number[]>();
  marks.forEach((mark, i) => {
    if (!mark.quote) return;
    const text = collapse(plainQuote(mark.quote).trim()).flat;
    if (text === "") return;
    const list = groups.get(text) ?? [];
    list.push(i);
    groups.set(text, list);
  });
  for (const [text, members] of groups) {
    // Gleiche Bereiche = ein Zitat; ohne Bereich zählt jede Stelle für sich (Reihenfolge der Liste).
    const spans: string[] = [];
    const slotOf = new Map<number, number>();
    for (const i of members) {
      const span = marks[i].quote_span;
      const id = span ? `${span[0]}:${span[1]}` : `#${i}`;
      let slot = spans.indexOf(id);
      if (slot < 0) {
        spans.push(id);
        slot = spans.length - 1;
      }
      slotOf.set(i, slot);
    }
    const order = spans
      .map((id, slot) => ({ slot, start: id.startsWith("#") ? Number.MAX_SAFE_INTEGER : Number(id.split(":")[0]) }))
      .sort((a, b) => a.start - b.start || a.slot - b.slot)
      .map((x) => x.slot);
    const all = findAll(haystack, text);
    const inQuotes = all.filter(([s, e]) => quoted(haystack, s, e));
    const places = inQuotes.length === spans.length ? inQuotes : all.length === spans.length ? all : null;
    if (!places) continue;
    for (const i of members) {
      result[i] = places[order.indexOf(slotOf.get(i)!)];
    }
  }
  return result;
}
