// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „scripture-check“ (Runde 17c): „Zitate prüfen“ in der Statuszeile des Editors. Ohne JavaScript ist es
// ein gewöhnlicher Link auf die Seite /werke/{id}/pruefung (Liste). Mit JavaScript:
// - Klick holt das Ergebnis der Wortlautprüfung: ohne ungespeicherte Änderung RPC `bible.marks` (gezeigte
//   Fassung), sonst `scripture.check` mit dem Quelltext, den das Satzbild über `baruch:serialize-text` liefert
//   (Vertrag Runde 17 §2.6; Rückfall Textfeld) – geprüft wird immer auf dem Server, nie hier.
// - Die Liste erscheint als Aufklapper über der Statuszeile (Stelle, Ergebnis, Unterschiede, Modul, Lizenz).
// - Im Satzbild werden die Zitate über die CSS Custom Highlight API markiert (`bibel-stimmt`, `bibel-weicht`):
//   nur Range-Objekte über vorhandene Textknoten, **keine Änderung am DOM** des Satzbilds – Speichern bleibt
//   unverändert. Ohne Highlight-API gibt es nur die Liste. Welche Fundstelle zu welchem Zitat gehört, entscheidet
//   `placeQuotes` (quote_find.ts: ganze Wörter, Typst-Auszeichnung entfernt, Reihenfolge nach `quote_span`); ein
//   nicht eindeutig gefundenes Zitat bleibt unmarkiert und wird in der Fußzeile gezählt. Jede Eingabe im Satzbild löscht die Markierung (die
//   Stellen könnten sich verschoben haben).

import { placeQuotes } from "./quote_find.js";
import { call } from "./rpc.js";
import { register } from "./widgets.js";

/// Eine Stelle, wie `bible.marks` sie liefert (bzw. aus `scripture.check` umgerechnet).
interface Mark {
  ref: string;
  state: string;
  label: string;
  quote: string | null;
  quote_span?: [number, number] | null;
  module_ref: string | null;
  diff_line: string | null;
  /// Runde 19c: Wortlaut stimmt nur ohne Leerraum (Trennreste im Modul).
  module_spacing?: boolean;
}

interface Marks {
  marks: Mark[];
  counts: { stimmt: number; weicht_ab: number; andere: number };
  bible: { module?: string; versification?: string; license?: string; problem?: string } | null;
  notice?: string | null;
  /// Runde 19c: Modul mit auffällig vielen Wortfragmenten.
  module_note?: string | null;
}

interface CheckedRef {
  normalized: string | null;
  input: string;
  wording: string;
  wording_code: string;
  quote: string | null;
  quote_span?: [number, number] | null;
  module_ref?: string | null;
  diff_line: string | null;
  module_spacing?: boolean;
}

const STATE_LABEL: Record<string, string> = {
  stimmt: "stimmt",
  weicht_ab: "weicht ab",
  ohne_zitat: "ohne Zitat",
  ungeprueft: "ungeprüft",
  fehler: "nicht prüfbar",
};

/// `wording_code` → Zustand (wie `bibles::mark_state` auf dem Server).
export function stateOf(code: string): string {
  switch (code) {
    case "match":
      return "stimmt";
    case "differs":
      return "weicht_ab";
    case "no_quote":
      return "ohne_zitat";
    case "unchecked":
      return "ungeprueft";
    default:
      return "fehler";
  }
}

/// Textknoten des Satzbilds mit ihrer Stelle im zusammengesetzten Text (Vorschläge `.pv` zählen nicht).
function textNodes(sheet: HTMLElement): { node: Text; start: number }[] {
  const out: { node: Text; start: number }[] = [];
  let at = 0;
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) =>
      (node.parentElement?.closest(".pv") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    out.push({ node: text, start: at });
    at += text.data.length;
  }
  return out;
}

/// Range über [start, end) des zusammengesetzten Texts.
function rangeOf(nodes: { node: Text; start: number }[], start: number, end: number): Range | null {
  const locate = (offset: number, edge: "start" | "end"): [Text, number] | null => {
    for (const { node, start: s } of nodes) {
      const e = s + node.data.length;
      if (edge === "start" ? offset < e : offset <= e) return [node, Math.max(0, offset - s)];
    }
    return null;
  };
  const a = locate(start, "start");
  const b = locate(end, "end");
  if (!a || !b) return null;
  const range = document.createRange();
  range.setStart(a[0], a[1]);
  range.setEnd(b[0], b[1]);
  return range;
}

/// Kann der Browser Bereiche hervorheben (CSS Custom Highlight API)?
function highlightApi(): boolean {
  return typeof CSS !== "undefined" && "highlights" in CSS && typeof (window as { Highlight?: unknown }).Highlight === "function";
}

type HighlightRegistry = Map<string, unknown>;
type HighlightCtor = new (...ranges: Range[]) => unknown;

function clearHighlights(): void {
  if (!highlightApi()) return;
  const registry = (CSS as unknown as { highlights: HighlightRegistry }).highlights;
  registry.delete("bibel-stimmt");
  registry.delete("bibel-weicht");
}

/// Zitate im Satzbild markieren (nur Range-Objekte, das DOM bleibt unverändert). Zahl der markierten und der
/// nicht eindeutig gefundenen Zitate.
function paint(sheet: HTMLElement, marks: Mark[]): { painted: number; missed: number } {
  clearHighlights();
  if (!highlightApi()) return { painted: 0, missed: 0 };
  const nodes = textNodes(sheet);
  const whole = nodes.map((n) => n.node.data).join("");
  const good: Range[] = [];
  const bad: Range[] = [];
  const checked = marks.filter((m) => m.quote && (m.state === "stimmt" || m.state === "weicht_ab"));
  const places = placeQuotes(whole, checked);
  let missed = 0;
  checked.forEach((mark, i) => {
    const place = places[i];
    const range = place ? rangeOf(nodes, place[0], place[1]) : null;
    if (range) (mark.state === "stimmt" ? good : bad).push(range);
    else missed++;
  });
  const Ctor = (window as unknown as { Highlight: HighlightCtor }).Highlight;
  const registry = (CSS as unknown as { highlights: HighlightRegistry }).highlights;
  if (good.length > 0) registry.set("bibel-stimmt", new Ctor(...good));
  if (bad.length > 0) registry.set("bibel-weicht", new Ctor(...bad));
  return { painted: good.length + bad.length, missed };
}

/// Quelltext des Editors: über `baruch:serialize-text` (Satzbild), sonst Textfeld.
async function sourceText(form: HTMLFormElement): Promise<string> {
  const wait: Promise<string>[] = [];
  form.dispatchEvent(new CustomEvent("baruch:serialize-text", { detail: { wait } }));
  if (wait.length > 0) {
    try {
      return await wait[0];
    } catch {
      // Rückfall unten
    }
  }
  return form.querySelector<HTMLTextAreaElement>("textarea[name=body]")?.value ?? "";
}

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, cls?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (text !== undefined) e.textContent = text;
  if (cls) e.className = cls;
  return e;
}

register("scripture-check", (root) => {
  const link = root.querySelector<HTMLAnchorElement>("a[data-check-link]");
  const editor = root.closest<HTMLElement>("[data-widget=wysiwyg]") ?? document.querySelector<HTMLElement>("[data-widget=wysiwyg]");
  const form = editor?.querySelector<HTMLFormElement>("form[data-widget=editor]") ?? null;
  const sheet = editor?.querySelector<HTMLElement>("[data-ws-sheet]") ?? null;
  if (!link || !form) return;
  const work = Number(root.dataset.work);
  const key = Number(root.dataset.key);
  const rev = Number(root.dataset.rev);
  const panel = element("div", undefined, "scripture-check-panel");
  panel.hidden = true;
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "Ergebnis der Zitatprüfung");
  root.append(panel);

  const show = (result: Marks, painted: number, missed: number): void => {
    panel.replaceChildren();
    const head = element("p", undefined, "small");
    const c = result.counts;
    head.textContent = `${c.stimmt} stimmt · ${c.weicht_ab} weicht ab · ${c.andere} andere`;
    panel.append(head);
    const bible = result.bible;
    if (bible && bible.module && !bible.problem) {
      panel.append(element("p", `Geprüft von Baruch mit ${bible.module} (Versifikation ${bible.versification ?? "?"})`, "muted tiny"));
    } else if (bible && bible.problem) {
      panel.append(element("p", `Bibel nicht lesbar: ${bible.problem}`, "notice tiny"));
    } else {
      panel.append(element("p", "Das Werk hat keine Bibel: Der Wortlaut bleibt ungeprüft.", "notice tiny"));
    }
    const notice = result.notice ?? bible?.license;
    if (notice) panel.append(element("p", `Lizenz (laut conf): ${notice}`, "muted tiny"));
    if (result.module_note) panel.append(element("p", result.module_note, "notice tiny check-module-note"));
    const list = element("ul");
    for (const mark of result.marks) {
      const item = element("li");
      const state = element("span", STATE_LABEL[mark.state] ?? mark.state, "check-state");
      state.dataset.state = mark.state;
      item.append(element("strong", mark.ref), document.createTextNode(" "), state);
      if (mark.quote) item.append(element("div", `„${mark.quote}“`, "small"));
      if (mark.diff_line) item.append(element("div", mark.diff_line, "tiny"));
      if (mark.module_ref) item.append(element("div", `gelesen: ${mark.module_ref}`, "muted tiny"));
      if (mark.state === "fehler" || mark.state === "ohne_zitat" || mark.module_spacing) {
        item.append(element("div", mark.label, "muted tiny"));
      }
      list.append(item);
    }
    if (result.marks.length === 0) list.append(element("li", "Keine Bibelstelle gefunden.", "muted"));
    panel.append(list);
    const foot = element("p", undefined, "tiny");
    const page = element("a", "Als Seite öffnen");
    page.href = link.href;
    foot.append(page);
    if (!highlightApi()) foot.append(document.createTextNode(" · Markierung im Satzbild braucht einen neueren Browser."));
    else {
      if (painted > 0) foot.append(document.createTextNode(` · ${painted} Zitat(e) im Satzbild markiert`));
      if (missed > 0) foot.append(document.createTextNode(` · ${missed} Zitat(e) im Satzbild nicht eindeutig gefunden (nur in der Liste)`));
    }
    panel.append(foot);
    panel.hidden = false;
  };

  const check = async (): Promise<void> => {
    link.setAttribute("aria-busy", "true");
    try {
      let result: Marks;
      if (editor?.querySelector<HTMLElement>("[data-widget=editor]")?.dataset.dirty === "1") {
        const text = await sourceText(form);
        const checked = await call<{ refs: CheckedRef[]; bible: Marks["bible"]; module_note?: string | null }>(
          "scripture.check",
          { work, text },
        );
        const marks = checked.refs.map((r) => ({
          ref: r.normalized ?? r.input,
          state: stateOf(r.wording_code),
          label: r.wording,
          quote: r.quote,
          quote_span: r.quote_span ?? null,
          module_ref: r.module_ref ?? null,
          diff_line: r.diff_line,
          module_spacing: r.module_spacing ?? false,
        }));
        const count = (s: string) => marks.filter((m) => m.state === s).length;
        result = {
          marks,
          counts: { stimmt: count("stimmt"), weicht_ab: count("weicht_ab"), andere: marks.length - count("stimmt") - count("weicht_ab") },
          bible: checked.bible,
          module_note: checked.module_note ?? null,
        };
      } else {
        result = await call<Marks>("bible.marks", { work, key, rev });
      }
      const { painted, missed } = sheet ? paint(sheet, result.marks) : { painted: 0, missed: 0 };
      show(result, painted, missed);
    } catch (error) {
      panel.replaceChildren(element("p", `Prüfung nicht möglich: ${String((error as Error).message ?? error)}`, "notice small"));
      panel.hidden = false;
    } finally {
      link.removeAttribute("aria-busy");
    }
  };

  link.addEventListener("click", (event) => {
    // Mit Umschalt/Strg bleibt es der Link auf die Seite.
    if (event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    if (!panel.hidden) {
      panel.hidden = true;
      clearHighlights();
      return;
    }
    void check();
  });
  panel.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      panel.hidden = true;
      link.focus();
    }
  });
  sheet?.addEventListener("input", () => clearHighlights());
});
