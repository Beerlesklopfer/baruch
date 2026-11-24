// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „bibles“ (Runde 15, Bibeln in der Literatur). Ohne JavaScript geht alles über die Seite
// /literatur/bibel (GET-Formulare, Feld „Stelle“, POST /literatur/bibel/act). Mit JavaScript:
// - Klick auf eine Versnummer setzt die Stelle („Ps 139,23“) ins Feld „Stelle“ des Formulars „Verse verwenden“
//   (Runde 19c: immer die deutsche Stelle, auch bei einem Modul in anderer Verszählung – der Server bildet jeden
//   Vers zurück und schreibt sie in `data-verse-ref`; ohne JavaScript schlägt der Link sie auf);
//   Umschalt-Klick erweitert sie bis zu diesem Vers („Ps 139,23-24“) und markiert die Verse.
// - Markierter Text in den Versen: die berührten Verse werden zur Stelle (Anfang bis Ende der Markierung).
// Der Text selbst kommt immer vom Server aus dem Modul (Feld `ref`), nie aus dem Browser.

import { register } from "./widgets.js";

/// „Ps 139,23“ → { book: "Ps", chapter: 139, verse: 23 }.
export function splitRef(reference: string): { book: string; chapter: number; verse: number } | null {
  const match = /^(.*\S)\s+(\d+),(\d+)$/.exec(reference.trim());
  if (!match) return null;
  return { book: match[1], chapter: Number(match[2]), verse: Number(match[3]) };
}

/// Stelle von `from` bis `to` (beide „Buch Kap,Vers“): gleiches Kapitel „Ps 139,23-24“, sonst „Ps 139,23-140,2“.
export function spanRef(from: string, to: string): string {
  const a = splitRef(from);
  const b = splitRef(to);
  if (!a || !b || a.book !== b.book) return from;
  const [first, last] = a.chapter * 1000 + a.verse <= b.chapter * 1000 + b.verse ? [a, b] : [b, a];
  if (first.chapter === last.chapter && first.verse === last.verse) return `${first.book} ${first.chapter},${first.verse}`;
  if (first.chapter === last.chapter) return `${first.book} ${first.chapter},${first.verse}-${last.verse}`;
  return `${first.book} ${first.chapter},${first.verse}-${last.chapter},${last.verse}`;
}

register("bibles", (root) => {
  const field = root.querySelector<HTMLInputElement>("input[data-bible-ref]");
  if (!field) return;
  let anchor: string | null = null;
  const verses = (): HTMLElement[] => [...root.querySelectorAll<HTMLElement>(".bible-verse[data-ref]")];
  const mark = (reference: string): void => {
    const span = splitRef(reference.split("-")[0] ?? "");
    const picked = new Set<string>();
    if (span) {
      const all = verses();
      const start = all.findIndex((v) => v.dataset.ref === `${span.book} ${span.chapter},${span.verse}`);
      const endRef = reference.includes("-") ? spanEnd(reference, span) : null;
      const end = endRef ? all.findIndex((v) => v.dataset.ref === endRef) : start;
      for (let i = Math.min(start, end); i >= 0 && i <= Math.max(start, end); i++) picked.add(all[i].dataset.ref ?? "");
    }
    for (const v of verses()) v.classList.toggle("picked", picked.has(v.dataset.ref ?? ""));
  };
  const spanEnd = (reference: string, start: { book: string; chapter: number }): string | null => {
    const tail = reference.split("-")[1] ?? "";
    if (/^\d+$/.test(tail)) return `${start.book} ${start.chapter},${tail}`;
    const m = /^(\d+),(\d+)$/.exec(tail);
    return m ? `${start.book} ${m[1]},${m[2]}` : null;
  };
  root.addEventListener("click", (event) => {
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[data-verse-ref]");
    if (!link) return;
    event.preventDefault();
    const reference = link.dataset.verseRef ?? "";
    const value = event.shiftKey && anchor ? spanRef(anchor, reference) : reference;
    if (!event.shiftKey) anchor = reference;
    field.value = value;
    mark(value);
  });
  root.addEventListener("mouseup", () => {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || selection.rangeCount === 0) return;
    const range = selection.getRangeAt(0);
    const verseOf = (node: Node): HTMLElement | null =>
      (node instanceof Element ? node : node.parentElement)?.closest<HTMLElement>(".bible-verse[data-ref]") ?? null;
    const first = verseOf(range.startContainer);
    const last = verseOf(range.endContainer);
    if (!first || !last || !root.contains(first) || !root.contains(last)) return;
    const value = spanRef(first.dataset.ref ?? "", last.dataset.ref ?? "");
    field.value = value;
    anchor = first.dataset.ref ?? null;
    mark(value);
  });
});
