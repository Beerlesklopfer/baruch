// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „scripture-check“ (Round 17c): „Zitate prüfen“ in the status line of the editor. Without JavaScript it is
// an ordinary link to the page /werke/{id}/pruefung (list). With JavaScript:
// - A click fetches the result of the wording check: without an unsaved change RPC `bible.marks` (shown
//   revision), otherwise `scripture.check` with the source text that the typeset view supplies through
//   `baruch:serialize-text` (contract Round 17 §2.6; fallback text field) – checking always happens on the server,
//   never here.
// - The list appears as an expander above the status line (passage, result, differences, module, licence).
// - In the typeset view the quotations are marked through the CSS Custom Highlight API (`bibel-stimmt`,
//   `bibel-weicht`): only Range objects over existing text nodes, **no change to the DOM** of the typeset view –
//   saving stays unchanged. Without the Highlight API there is only the list. Which place found belongs to which
//   quotation is decided by `placeQuotes` (quote_find.ts: whole words, Typst markup removed, order by `quote_span`);
//   a quotation not found unambiguously stays unmarked and is counted in the footer. Every input in the typeset view
//   deletes the marking (the places could have shifted).

import { t } from "./i18n.js";
import { placeQuotes } from "./quote_find.js";
import { call } from "./rpc.js";
import { register } from "./widgets.js";

/// A passage as `bible.marks` supplies it (or converted from `scripture.check`).
interface Mark {
  ref: string;
  state: string;
  label: string;
  quote: string | null;
  quote_span?: [number, number] | null;
  module_ref: string | null;
  diff_line: string | null;
  /// Round 19c: the wording matches only without whitespace (hyphenation remains in the module).
  module_spacing?: boolean;
}

interface Marks {
  marks: Mark[];
  counts: { stimmt: number; weicht_ab: number; andere: number };
  bible: { module?: string; versification?: string; license?: string; problem?: string } | null;
  notice?: string | null;
  /// Round 19c: module with a conspicuous number of word fragments.
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
  stimmt: "widget.scripture_check.state_matches",
  weicht_ab: "widget.scripture_check.state_differs",
  ohne_zitat: "widget.scripture_check.state_no_quote",
  ungeprueft: "widget.scripture_check.state_unchecked",
  fehler: "widget.scripture_check.state_error",
};

/// `wording_code` → state (like `bibles::mark_state` on the server).
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

/// Text nodes of the typeset view with their position in the combined text (proposals `.pv` do not count).
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

/// Range over [start, end) of the combined text.
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

/// Can the browser highlight ranges (CSS Custom Highlight API)?
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

/// Mark quotations in the typeset view (only Range objects, the DOM stays unchanged). Number of the marked and of
/// the not unambiguously found quotations.
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

/// Source text of the editor: through `baruch:serialize-text` (typeset view), otherwise the text field.
async function sourceText(form: HTMLFormElement): Promise<string> {
  const wait: Promise<string>[] = [];
  form.dispatchEvent(new CustomEvent("baruch:serialize-text", { detail: { wait } }));
  if (wait.length > 0) {
    try {
      return await wait[0];
    } catch {
      // fallback below
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
  panel.setAttribute("aria-label", t("widget.scripture_check.panel_label"));
  root.append(panel);

  const show = (result: Marks, painted: number, missed: number): void => {
    panel.replaceChildren();
    const head = element("p", undefined, "small");
    const c = result.counts;
    head.textContent = t("widget.scripture_check.counts", { matches: c.stimmt, differs: c.weicht_ab, others: c.andere });
    panel.append(head);
    const bible = result.bible;
    if (bible && bible.module && !bible.problem) {
      panel.append(element("p", t("widget.scripture_check.checked_with", { module: bible.module, versification: bible.versification ?? "?" }), "muted tiny"));
    } else if (bible && bible.problem) {
      panel.append(element("p", t("widget.scripture_check.bible_unreadable", { problem: bible.problem }), "notice tiny"));
    } else {
      panel.append(element("p", t("widget.scripture_check.no_bible"), "notice tiny"));
    }
    const notice = result.notice ?? bible?.license;
    if (notice) panel.append(element("p", t("widget.scripture_check.license", { notice }), "muted tiny"));
    if (result.module_note) panel.append(element("p", result.module_note, "notice tiny check-module-note"));
    const list = element("ul");
    for (const mark of result.marks) {
      const item = element("li");
      const state = element("span", mark.state in STATE_LABEL ? t(STATE_LABEL[mark.state]) : mark.state, "check-state");
      state.dataset.state = mark.state;
      item.append(element("strong", mark.ref), document.createTextNode(" "), state);
      if (mark.quote) item.append(element("div", t("widget.scripture_check.quoted", { quote: mark.quote }), "small"));
      if (mark.diff_line) item.append(element("div", mark.diff_line, "tiny"));
      if (mark.module_ref) item.append(element("div", t("widget.scripture_check.read_as", { ref: mark.module_ref }), "muted tiny"));
      if (mark.state === "fehler" || mark.state === "ohne_zitat" || mark.module_spacing) {
        item.append(element("div", mark.label, "muted tiny"));
      }
      list.append(item);
    }
    if (result.marks.length === 0) list.append(element("li", t("widget.scripture_check.none_found"), "muted"));
    panel.append(list);
    const foot = element("p", undefined, "tiny");
    const page = element("a", t("widget.scripture_check.open_page"));
    page.href = link.href;
    foot.append(page);
    if (!highlightApi()) foot.append(document.createTextNode(` · ${t("widget.scripture_check.old_browser")}`));
    else {
      if (painted > 0) foot.append(document.createTextNode(` · ${t("widget.scripture_check.painted", { count: painted })}`));
      if (missed > 0) foot.append(document.createTextNode(` · ${t("widget.scripture_check.missed", { count: missed })}`));
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
      panel.replaceChildren(element("p", t("widget.scripture_check.failed", { error: String((error as Error).message ?? error) }), "notice small"));
      panel.hidden = false;
    } finally {
      link.removeAttribute("aria-busy");
    }
  };

  link.addEventListener("click", (event) => {
    // With Shift/Ctrl it stays the link to the page.
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
