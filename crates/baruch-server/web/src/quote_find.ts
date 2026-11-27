// SPDX-License-Identifier: AGPL-3.0-or-later
// Finding quotations again in the typeset view (follow-up fix Round 17, for the widget „scripture-check“): pure
// functions without DOM so that a test can check them under Node (tests/quote_find.rs).
//
// The server supplies per passage the quotation text (`quote`, Typst source text) and its range in the source text
// (`quote_span`, bytes). The typeset view knows no source positions; therefore:
// - The quotation text is stripped of Typst markup (`#emph[…]`, `*`, `_`, `\`), as it stands in the typeset view.
// - The search is only for whole words (no letter and no digit directly before or after).
// - Same wording at several places: the quotations are assigned to the places found in the order of their
//   `quote_span` – first to the places found in quotation marks, otherwise to all. If the number does not fit, the
//   quotation stays unmarked (not unambiguous) instead of showing every place found in both colours.

/// Text with collapsed whitespace and the position of every character in the original text.
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

/// Quotation text without Typst markup: `#emph[x]`/`#strong[x]` → x, `\x` → x, `*` and `_` are dropped.
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

/// All places where `needle` is found in `haystack` as whole words (whitespace counts as one space): [start, end)
/// in the original text.
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

/// Does the place found [s, e) stand in quotation marks (whitespace in between does not count)?
function quoted(haystack: string, s: number, e: number): boolean {
  let a = s - 1;
  while (a >= 0 && /\s/.test(haystack[a])) a--;
  let b = e;
  while (b < haystack.length && /\s/.test(haystack[b])) b++;
  return a >= 0 && b < haystack.length && OPENING.includes(haystack[a]) && CLOSING.includes(haystack[b]);
}

/// A quotation as the widget assigns it.
export interface Placeable {
  quote: string | null;
  quote_span?: [number, number] | null;
}

/// Assigns to every quotation at most one place found in the typeset view (see the header of the file). Result in
/// the order of `marks`; `null` = without quotation or not found unambiguously. Two passages with the same
/// `quote_span` are the same quotation and get the same place found.
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
    // Same ranges = one quotation; without a range every passage counts for itself (order of the list).
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
