// SPDX-License-Identifier: AGPL-3.0-or-later
// Search and replace in the editor (Round 30a, contract §3.1): search bar at the top right (fragments/search_bar.html)
// with an expandable replace row and a work sidebar with a tree of hits. Keys following VS Code through keyAction
// (search_keys.ts), listener on the document (Ctrl+F/H catches the search also in the source text).
//
// - In the section: RPC `search.find` with range=section (with „In Markierung“, Alt+L: range=selection and start/end
//   = the place selected in the source text when switching on, UTF-16) and `text` = unsaved editor text; hits in the
//   source text through the selection of the text field, in the typeset view through the CSS Custom Highlight API (no
//   DOM change).
// - Replacing in the section changes only the unsaved editor text, through the offsets from `search.preview` with
//   `text=`; saving is done as always by the human. With the typeset view open the view is afterwards switched to
//   the source text so that the change stays visible and saveable.
// - In the work (Ctrl+Shift+F/H): sidebar with hits per section (jump links `data-jump`); outside the
//   workspace page navigation to /suche.
// - `baruch:find {text, at}` (jump.ts): select the place in the source text (wysiwyg.ts marks the typeset view).

import { byteToUtf16, locate } from "./jump.js";
import { call } from "./rpc.js";
import { keyAction, type Action, type KeyContext } from "./search_keys.js";
import { register } from "./widgets.js";

interface Hit {
  work: number;
  key: number;
  number: string;
  title: string;
  rev: number;
  kind: string;
  start: number;
  end: number;
  u16_start: number;
  u16_end: number;
  line: number;
  before: string;
  matched: string;
  after: string;
  url: string;
}

interface FindResult {
  hits: Hit[];
  total: number;
  truncated: boolean;
  timed_out: boolean;
  range_label: string;
}

interface PreviewResult {
  sections: { key: number; hits: Hit[]; replacements: string[]; body_after: string }[];
}

interface Bar {
  root: HTMLElement;
  work: number;
  key: number;
  q: HTMLInputElement;
  r: HTMLInputElement;
  count: HTMLElement;
  replaceRow: HTMLElement;
  options: { case: boolean; word: boolean; re: boolean };
  /// „In Markierung“: the place selected in the source text when switching on (UTF-16), otherwise null.
  selection: { start: number; end: number } | null;
  /// Selection in the source text when opening the bar (before the hits move the selection).
  marked: { start: number; end: number } | null;
  hits: Hit[];
  index: number;
  timer: number | undefined;
}

let active: Bar | null = null;

function toast(message: string): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level: "info" } }));
}

/// Area of the editor around the bar.
function editorRoot(bar: Bar): HTMLElement | null {
  return bar.root.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
}

function area(bar: Bar): HTMLTextAreaElement | null {
  return editorRoot(bar)?.querySelector<HTMLTextAreaElement>("textarea[name=body]") ?? null;
}

function sheetOpen(bar: Bar): boolean {
  return editorRoot(bar)?.hasAttribute("data-ws-open") ?? false;
}

function highlightSupported(): boolean {
  return typeof CSS !== "undefined" && "highlights" in CSS && typeof (window as { Highlight?: unknown }).Highlight === "function";
}

type HighlightRegistry = Map<string, unknown>;

/// Text nodes of the typeset view with their start in the combined text.
function sheetText(sheet: HTMLElement): { text: string; nodes: { node: Text; start: number }[] } {
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) =>
      (node.parentElement?.closest(".ws-island-edit, .pv") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  const nodes: { node: Text; start: number }[] = [];
  let text = "";
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    nodes.push({ node: n as Text, start: text.length });
    text += (n as Text).data;
  }
  return { text, nodes };
}

function rangeOf(nodes: { node: Text; start: number }[], start: number, end: number): Range | null {
  const at = (pos: number) => {
    for (let i = nodes.length - 1; i >= 0; i--) {
      if (nodes[i].start <= pos) return { node: nodes[i].node, offset: Math.min(pos - nodes[i].start, nodes[i].node.data.length) };
    }
    return null;
  };
  const a = at(start);
  const b = at(end);
  if (!a || !b) return null;
  const range = document.createRange();
  range.setStart(a.node, a.offset);
  range.setEnd(b.node, b.offset);
  return range;
}

/// Mark hits in the typeset view: every hit is searched in the typeset view through its text (in order).
function markSheet(bar: Bar): void {
  if (!highlightSupported()) return;
  const registry = (CSS as unknown as { highlights: HighlightRegistry }).highlights;
  registry.delete("baruch-search");
  registry.delete("baruch-search-current");
  const sheet = editorRoot(bar)?.querySelector<HTMLElement>("[data-ws-sheet]");
  if (!sheet || !sheetOpen(bar) || bar.hits.length === 0) return;
  const { text, nodes } = sheetText(sheet);
  const Ctor = (window as unknown as { Highlight: new (...r: Range[]) => unknown }).Highlight;
  const all: Range[] = [];
  let current: Range | null = null;
  let from = 0;
  bar.hits.forEach((hit, i) => {
    const plain = hit.matched.replace(/\\(.)/g, "$1");
    const found = locate(text.slice(from), plain, 0);
    if (!found) return;
    const range = rangeOf(nodes, from + found[0], from + found[1]);
    from += found[1];
    if (!range) return;
    all.push(range);
    if (i === bar.index) current = range;
  });
  registry.set("baruch-search", new Ctor(...all));
  if (current) {
    registry.set("baruch-search-current", new Ctor(current));
    (current as Range).startContainer.parentElement?.scrollIntoView({ block: "center" });
  }
}

/// Show the current hit (source text: selection; typeset view: marking).
function show(bar: Bar): void {
  const n = bar.hits.length;
  bar.count.textContent = n === 0 ? "Keine Treffer" : `${bar.index + 1} von ${n}`;
  if (n === 0) {
    markSheet(bar);
    return;
  }
  const hit = bar.hits[bar.index];
  const field = area(bar);
  if (field && !field.hidden && !sheetOpen(bar)) {
    const focus = document.activeElement;
    field.setSelectionRange(hit.u16_start, hit.u16_end);
    // Without focus the text field shows no selection; the focus stays in the bar if it was there.
    field.focus();
    if (focus instanceof HTMLElement && bar.root.contains(focus)) focus.focus();
  }
  markSheet(bar);
}

/// Range of the search in the section: whole revision or (switched on) only the selection.
function rangeParams(bar: Bar): { range: string; start?: number; end?: number } {
  return bar.selection
    ? { range: "selection", start: bar.selection.start, end: bar.selection.end }
    : { range: "section" };
}

async function runFind(bar: Bar): Promise<void> {
  const q = bar.q.value;
  if (!q) {
    bar.hits = [];
    bar.index = 0;
    show(bar);
    return;
  }
  const field = area(bar);
  try {
    const result = await call<FindResult>("search.find", {
      work: bar.work,
      q,
      case: bar.options.case,
      word: bar.options.word,
      re: bar.options.re,
      ...rangeParams(bar),
      key: bar.key,
      text: field?.value,
      limit: 2000,
    });
    bar.hits = result.hits;
    bar.index = Math.min(bar.index, Math.max(0, result.hits.length - 1));
    show(bar);
    if (result.timed_out) toast("Die Suche brauchte länger als 2 s und wurde abgebrochen.");
  } catch (error) {
    bar.hits = [];
    bar.count.textContent = String((error as Error).message ?? error);
  }
}

function schedule(bar: Bar): void {
  if (bar.timer !== undefined) window.clearTimeout(bar.timer);
  bar.timer = window.setTimeout(() => void runFind(bar), 200);
}

function open(bar: Bar, replace: boolean): void {
  bar.root.hidden = false;
  active = bar;
  if (replace) setReplace(bar, true);
  const field = area(bar);
  bar.marked =
    field && !field.hidden && !sheetOpen(bar) && field.selectionStart !== field.selectionEnd
      ? { start: field.selectionStart, end: field.selectionEnd }
      : null;
  // Selected text becomes the search text (as in VS Code).
  const selected = window.getSelection()?.toString() ?? "";
  if (selected && !selected.includes("\n") && selected.length <= 200) bar.q.value = selected;
  // As in VS Code: a multi-line selection switches „In Markierung“ on (otherwise it stays as it was).
  if (bar.marked && field && field.value.slice(bar.marked.start, bar.marked.end).includes("\n")) {
    bar.selection = { ...bar.marked };
    bar.root.querySelector("[data-sb-toggle=sel]")?.setAttribute("aria-pressed", "true");
  }
  bar.q.focus();
  bar.q.select();
  schedule(bar);
}

function close(bar: Bar): void {
  bar.root.hidden = true;
  bar.hits = [];
  markSheet(bar);
  if (active === bar) active = null;
  if (sheetOpen(bar)) editorRoot(bar)?.querySelector<HTMLElement>("[data-ws-sheet]")?.focus();
  else area(bar)?.focus();
}

function setReplace(bar: Bar, on: boolean): void {
  bar.replaceRow.hidden = !on;
  const toggle = bar.root.querySelector<HTMLElement>("[data-sb-expand]");
  if (toggle) {
    toggle.setAttribute("aria-expanded", String(on));
    toggle.textContent = on ? "▾" : "▸";
  }
  if (on) bar.r.focus();
}

function step(bar: Bar, delta: number): void {
  const n = bar.hits.length;
  if (n === 0) return;
  bar.index = (bar.index + delta + n) % n;
  show(bar);
}

/// Replace in the unsaved editor text (offsets from `search.preview` with `text=`); `one`: only the current hit.
async function replaceInText(bar: Bar, one: boolean): Promise<void> {
  const field = area(bar);
  if (!field || !bar.q.value) return;
  // In the typeset view: first fetch the current state into the text field (as before saving).
  const wait: Promise<unknown>[] = [];
  editorRoot(bar)?.querySelector("form[data-widget=editor]")?.dispatchEvent(new CustomEvent("baruch:before-save", { detail: { wait } }));
  await Promise.all(wait);
  try {
    const result = await call<PreviewResult>("search.preview", {
      work: bar.work,
      q: bar.q.value,
      case: bar.options.case,
      word: bar.options.word,
      re: bar.options.re,
      ...rangeParams(bar),
      key: bar.key,
      text: field.value,
      replacement: bar.r.value,
    });
    const section = result.sections[0];
    if (!section || section.hits.length === 0) {
      toast("Nichts zu ersetzen.");
      return;
    }
    let text: string;
    if (one) {
      const i = Math.min(bar.index, section.hits.length - 1);
      const hit = section.hits[i];
      text = field.value.slice(0, hit.u16_start) + section.replacements[i] + field.value.slice(hit.u16_end);
    } else {
      text = section.body_after;
    }
    field.value = text;
    field.dispatchEvent(new Event("input", { bubbles: true }));
    // The selection referred to the old text.
    if (bar.selection) toggleSelection(bar, false);
    // Make it visible in the typeset view: switch to the source text (the text is still unsaved).
    if (sheetOpen(bar)) editorRoot(bar)?.querySelector<HTMLButtonElement>("[data-ws-mode=quelltext]")?.click();
    toast(one ? "Ersetzt (noch nicht gespeichert)." : `${section.hits.length} Stellen ersetzt (noch nicht gespeichert).`);
    await runFind(bar);
  } catch (error) {
    toast(String((error as Error).message ?? error));
  }
}

/// „In Markierung“ on/off: when switching on, the selection in the source text applies (otherwise it stays off, with
/// a notice). After "replace all" the length of the text changes: the selection is then switched off.
function toggleSelection(bar: Bar, on?: boolean): void {
  const want = on ?? bar.selection === null;
  if (want) {
    // The selection from opening the bar (the hits move the selection afterwards).
    if (!bar.marked) {
      toast("Erst im Quelltext Text markieren, dann die Suche öffnen und „In Markierung“ einschalten.");
      return;
    }
    bar.selection = { ...bar.marked };
  } else {
    bar.selection = null;
  }
  bar.root.querySelector("[data-sb-toggle=sel]")?.setAttribute("aria-pressed", String(bar.selection !== null));
  bar.index = 0;
  schedule(bar);
}

function toggle(bar: Bar, name: "case" | "word" | "re"): void {
  bar.options[name] = !bar.options[name];
  bar.root.querySelector(`[data-sb-toggle=${name}]`)?.setAttribute("aria-pressed", String(bar.options[name]));
  schedule(bar);
}

// ---- Work sidebar --------------------------------------------------------------------------------------------

function sidebar(): HTMLElement {
  let panel = document.querySelector<HTMLElement>("[data-search-sidebar]");
  if (panel) return panel;
  panel = document.createElement("aside");
  panel.className = "search-sidebar";
  panel.dataset.searchSidebar = "";
  panel.setAttribute("aria-label", "Im Werk suchen");
  panel.innerHTML =
    '<div class="ss-head"><strong>Im Werk suchen</strong> <button type="button" class="btn" data-ss-close aria-label="Schließen">✕</button></div>' +
    '<form class="ss-form" data-ss-form><input type="search" name="q" placeholder="Suchen" aria-label="Suchen" maxlength="500">' +
    '<input type="text" name="replacement" placeholder="Ersetzen" aria-label="Ersetzen" maxlength="2000" data-ss-replacement hidden>' +
    '<label class="small"><input type="checkbox" name="case" value="1"> Aa</label> <label class="small"><input type="checkbox" name="word" value="1"> Wort</label> ' +
    '<label class="small"><input type="checkbox" name="re" value="1"> .*</label> <button type="submit" class="btn">Suchen</button> ' +
    '<a class="small" data-ss-full href="/suche">Vorschau und Ersetzen …</a></form><p class="small muted" data-ss-summary></p><ul class="ss-tree" data-ss-tree></ul>';
  document.body.append(panel);
  panel.querySelector("[data-ss-close]")?.addEventListener("click", () => panel?.remove());
  panel.addEventListener("keydown", (event) => {
    if (event.key === "Escape") panel?.remove();
  });
  panel.querySelector<HTMLFormElement>("[data-ss-form]")?.addEventListener("submit", (event) => {
    event.preventDefault();
    void workSearch(panel as HTMLElement);
  });
  return panel;
}

function workAndKey(): { work: number | null; key: number | null } {
  const editor = document.querySelector<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
  const params = new URLSearchParams(window.location.search);
  const work = Number(editor?.dataset.work ?? params.get("work") ?? NaN);
  const key = Number(editor?.dataset.key ?? params.get("key") ?? NaN);
  return { work: Number.isFinite(work) ? work : null, key: Number.isFinite(key) ? key : null };
}

async function workSearch(panel: HTMLElement): Promise<void> {
  const form = panel.querySelector<HTMLFormElement>("[data-ss-form]");
  if (!form) return;
  const data = new FormData(form);
  const q = String(data.get("q") ?? "");
  const { work } = workAndKey();
  const tree = panel.querySelector<HTMLElement>("[data-ss-tree]");
  const summary = panel.querySelector<HTMLElement>("[data-ss-summary]");
  const full = panel.querySelector<HTMLAnchorElement>("[data-ss-full]");
  const params = new URLSearchParams({ q, range: "work", vorschau: "1" });
  if (work !== null) params.set("work", String(work));
  for (const name of ["case", "word", "re"]) if (data.get(name)) params.set(name, "1");
  const replacement = String(data.get("replacement") ?? "");
  if (replacement) params.set("replacement", replacement);
  if (full) full.href = `/suche?${params.toString()}`;
  if (!q || !tree || !summary) return;
  try {
    const result = await call<FindResult>("search.find", {
      work,
      q,
      case: !!data.get("case"),
      word: !!data.get("word"),
      re: !!data.get("re"),
      range: "work",
    });
    summary.textContent = `${result.total} Treffer · ${result.range_label}${result.timed_out ? " (abgebrochen nach 2 s)" : ""}`;
    tree.replaceChildren();
    let group: HTMLUListElement | null = null;
    let last = -1;
    for (const hit of result.hits) {
      if (hit.key !== last) {
        const li = document.createElement("li");
        const head = document.createElement("strong");
        head.textContent = `${hit.number} ${hit.title}`.trim();
        group = document.createElement("ul");
        li.append(head, group);
        tree.append(li);
        last = hit.key;
      }
      const item = document.createElement("li");
      const link = document.createElement("a");
      link.href = hit.url;
      link.dataset.jump = "";
      link.className = "jump";
      const mark = document.createElement("mark");
      mark.textContent = hit.matched;
      link.append(hit.before, mark, hit.after);
      item.append(link);
      group?.append(item);
    }
  } catch (error) {
    summary.textContent = String((error as Error).message ?? error);
  }
}

function openWork(replace: boolean): void {
  const panel = sidebar();
  const input = panel.querySelector<HTMLInputElement>("input[name=q]");
  const repl = panel.querySelector<HTMLInputElement>("[data-ss-replacement]");
  if (repl) repl.hidden = !replace;
  const selected = window.getSelection()?.toString() ?? "";
  if (input && selected && !selected.includes("\n") && selected.length <= 200) input.value = selected;
  (replace && repl && input?.value ? repl : input)?.focus();
  void workSearch(panel);
}

// ---- Keys ----------------------------------------------------------------------------------------------------

function context(target: EventTarget | null): KeyContext {
  const element = target instanceof Element ? target : null;
  const onWorkspace = document.querySelector("div.wysiwyg[data-widget=wysiwyg]") !== null;
  return {
    inEditor: !!element?.closest("div.wysiwyg[data-widget=wysiwyg] form[data-widget=editor]"),
    inSearch: !!element?.closest("[data-widget=search-bar]"),
    onWorkspace,
    inChat: !!element?.closest(".chat-input, [data-widget=chat] form, form[data-chat]"),
    searchOpen: active !== null && !active.root.hidden,
  };
}

function firstBar(): Bar | null {
  const root = document.querySelector<HTMLElement>("[data-widget=search-bar]");
  return root ? bars.get(root) ?? null : null;
}

const bars = new WeakMap<HTMLElement, Bar>();

function perform(action: Action): void {
  const bar = active ?? firstBar();
  switch (action) {
    case "find-section":
    case "replace-section":
      if (bar) open(bar, action === "replace-section");
      return;
    case "find-work":
    case "replace-work":
      if (document.querySelector("div.wysiwyg[data-widget=wysiwyg]")) openWork(action === "replace-work");
      else {
        const { work } = workAndKey();
        window.location.href = `/suche${work !== null ? `?work=${work}&range=work` : ""}`;
      }
      return;
    default:
      break;
  }
  if (!bar) return;
  switch (action) {
    case "next":
      step(bar, 1);
      return;
    case "prev":
      step(bar, -1);
      return;
    case "replace-one":
      void replaceInText(bar, true);
      return;
    case "replace-all":
      void replaceInText(bar, false);
      return;
    case "toggle-case":
      toggle(bar, "case");
      return;
    case "toggle-word":
      toggle(bar, "word");
      return;
    case "toggle-regex":
      toggle(bar, "re");
      return;
    case "toggle-selection":
      toggleSelection(bar);
      return;
    case "close":
      close(bar);
      return;
  }
}

if (typeof document !== "undefined") {
  document.addEventListener(
    "keydown",
    (event) => {
      const action = keyAction(event, context(event.target));
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      perform(action);
    },
    true,
  );
  // Select the place found from a jump in the source text (wysiwyg.ts marks the typeset view).
  document.addEventListener("baruch:find", (event) => {
    const detail = (event as CustomEvent<{ text?: unknown; at?: unknown }>).detail;
    if (typeof detail?.text !== "string") return;
    const field = document.querySelector<HTMLTextAreaElement>("div.wysiwyg[data-widget=wysiwyg] textarea[name=body]");
    if (!field || field.hidden) return;
    const near = typeof detail.at === "number" ? byteToUtf16(field.value, detail.at) : 0;
    const found = locate(field.value, detail.text, near);
    if (!found) return;
    field.focus();
    field.setSelectionRange(found[0], found[1]);
  });
}

register("search-bar", (root) => {
  const q = root.querySelector<HTMLInputElement>("[data-sb-q]");
  const r = root.querySelector<HTMLInputElement>("[data-sb-r]");
  const count = root.querySelector<HTMLElement>("[data-sb-count]");
  const replaceRow = root.querySelector<HTMLElement>("[data-sb-replace]");
  if (!q || !r || !count || !replaceRow) return;
  const bar: Bar = {
    root,
    work: Number(root.dataset.work),
    key: Number(root.dataset.key),
    q,
    r,
    count,
    replaceRow,
    options: { case: false, word: false, re: false },
    selection: null,
    marked: null,
    hits: [],
    index: 0,
    timer: undefined,
  };
  bars.set(root, bar);
  q.addEventListener("input", () => {
    bar.index = 0;
    schedule(bar);
  });
  root.querySelector("[data-sb-expand]")?.addEventListener("click", () => setReplace(bar, replaceRow.hidden));
  root.querySelectorAll<HTMLButtonElement>("[data-sb-toggle]").forEach((button) =>
    button.addEventListener("click", () => {
      if (button.dataset.sbToggle === "sel") toggleSelection(bar);
      else toggle(bar, button.dataset.sbToggle as "case" | "word" | "re");
    }),
  );
  root.querySelector("[data-sb-prev]")?.addEventListener("click", () => step(bar, -1));
  root.querySelector("[data-sb-next]")?.addEventListener("click", () => step(bar, 1));
  root.querySelector("[data-sb-close]")?.addEventListener("click", () => close(bar));
  root.querySelector("[data-sb-one]")?.addEventListener("click", () => void replaceInText(bar, true));
  root.querySelector("[data-sb-all]")?.addEventListener("click", () => void replaceInText(bar, false));
  // Mounted anew (after the redraw): an open search is lost, the bar stays closed.
  if (active && !active.root.isConnected) active = null;
});

// Page /suche: Alt+C/W/R toggle the checkboxes in the form when the focus is in the search field.
register("search-page", (root) => {
  root.addEventListener("keydown", (event) => {
    const e = event as KeyboardEvent;
    if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const name = { KeyC: "case", KeyW: "word", KeyR: "re" }[e.code];
    if (!name || !(e.target instanceof HTMLInputElement) || !e.target.matches("[data-search-q], [data-search-replacement]")) return;
    const box = root.querySelector<HTMLInputElement>(`input[type=checkbox][name=${name}]`);
    if (!box) return;
    e.preventDefault();
    box.checked = !box.checked;
  });
});
