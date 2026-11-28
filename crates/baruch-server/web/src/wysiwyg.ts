// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „wysiwyg“: the section in the typeset view of the work template (contenteditable) with the toggle
// „Satzbild | Quelltext“. The server splits the text into blocks (RPC wysiwyg.parse) and supplies the
// HTML; islands (Typst that cannot be represented) stand there with contenteditable=false. Before saving
// (event `baruch:before-save` on the editor form, triggered by outline.ts) wysiwyg.serialize rebuilds
// the text: a block without input is sent as `keep` and is preserved byte for byte,
// only blocks with input are generated anew. What is submitted is always the text field.
//
// The typeset-view diff of the proposals has been rendered by the server since Round 7 (fragments/proposals.html,
// `pv-satz`); the former widget „wysiwyg-diff“ is dropped, the RPC method wysiwyg.diff stays.
//
// Proposals at their place (Round 8): the server puts every piece (hunk) of an open proposal
// as <details class="pv" data-pv-gap="g"> into the reading flow `section.pv-flow` below the form. With the
// typeset view open the widget puts every piece as a non-editable sibling (contenteditable
// = false) before the block `[data-b=g]` of the typeset view (at the end: behind it) and hides the reading flow; in
// the source text the pieces come back to their place in the reading flow. editsOf, blocksIn and markSelection
// skip `.pv`: saving thereby changes nothing. The buttons „Übernehmen“/„Verwerfen“ point through the
// attribute `form` to forms in the reading flow (no forms in the editor form).
//
// Scenes and work orders (`#szene[…]`, `#arbeit[…]`): their body consists of blocks of their own
// (`data-n` = index in the body, assigned on opening). If only one paragraph in it is edited, a
// body instruction `{replaces, body: [...]}` goes to the server; the other blocks stay byte-identical.
//
// Headings (Round 7, „Absatz wird Titel“): a heading is always one line. A simple
// line break in it (Shift+Enter, `<br>` of the browser, a line end in the text) ends it; what
// stands behind it goes to the server as a paragraph of its own (the server splits likewise). Shift+Enter and
// pasting several lines in a heading begin a new paragraph; Backspace at the beginning
// of a paragraph or Del at the end of a heading do not pull a paragraph into the heading. Before
// saving the title rules of the server apply (empty, line break, over 200 characters) with the same
// message; the server stays authoritative.
//
// Parts and „ohne Nummer“ (Round 14): the marker lines `#heading(level: 1, numbering: none)[T]
// <baruch-part>` or `… <baruch-unnumbered>` come as `div.ws-block.ws-part[data-kind=part]` or
// `div.ws-block.ws-unnumbered[data-kind=unnumbered][data-level]` with the title as plain text. They
// behave like headings (one line, Enter begins a paragraph behind it, same
// title rules); when changed, `{kind: "part", title}` or `{kind: "unnumbered", level, title}` goes to the
// server, which builds the marker (the comment of the line stays). The paragraph format cannot be
// chosen for them: part and „ohne Nummer“ are changed by the outline or the source text.
//
// Images (Round 7): an island with an image comes as `div.ws-block.ws-figure[data-kind=island]` or
// `span.ws-island.ws-inline-image`; it is an island like any other (not editable, `keep`,
// deletable as a whole, raw text in `data-raw`).
//
// The widget sits on `div.wysiwyg`, which comprises the toolbar (`[data-edit-toolbar]`), the sheet
// (`div.editor-body` with the form and the typeset view `[data-ws-wrap]`) and the status line (note
// `[data-ws-hint]`, „Speichern“ with `form="editor-form"`) (wish 2026-11-06). Without JavaScript
// toolbar, typeset view and note stay `hidden`, there is only the text field. Round 33: after mounting
// band.ts moves the toolbar into the ribbon (docking place); what reaches into the bar afterwards uses
// `editToolbar` (widgets.ts) instead of `root.querySelector`. The default is the
// typeset view (also after every redraw through ui.render), unless the cookie `baruch_editor` says
// `quelltext` explicitly. With `#fundstelle` in the address on loading the page the first editor starts
// in the source text (the marking of the place found lies in the text field). With the typeset view open the area
// carries `data-ws-open`; the sheet fills the height of the editor and scrolls alone (app.css, wysiwyg.css).
//
// Typeset view as the default (Round 8, cause in docs/WYSIWYG.md): formerly the widget first switched to the
// source text when mounting (`show` with `open = false`: toggle „Quelltext“ pressed, text field
// visible) and opened the typeset view only after two RPC answers (wysiwyg.parse, wysiwyg.style);
// a screenshot after loading therefore showed the source text. Now the server supplies the typeset view
// of the shown revision ready-made (`[data-ws-sheet][data-ws-ready]`) and `aria-pressed` matching the
// cookie; `adoptSheet` opens it at once when mounting, without an intermediate state. If it is missing (text that
// the text field supplies differently), `openSheet` opens through RPC as before, without switching first.

// Round 18b („konstrukte“): numbered lists `1.` (kind `enum`, `<ol start>`), term lists (kind `terms`,
// `<dl>`), nested lists (list in the `<li>`, `depths`), forced break (`span.ws-linebreak`, t
// `linebreak`), links (`a.ws-link`, text editable) and references (`button.ws-ref`, not editable) are
// typeset view instead of island. An island opens a source text field in place by double click; when changed it goes
// as `{replaces, raw}` (block island) or in the newly generated paragraph (island in the sentence) to the server. The
// ribbon (ribbon.ts) sends commands only through `baruch:ws-command`, the image tools ask for the cursor position
// through `baruch:ws-caret` (ws_command.ts). A `<style id="ws-style" data-work="W">` supplied by the server is
// taken over without RPC.
//
// Round 31c („kontextmenue“): index marks `#metadata((…))<baruch-idx>` come as an empty, non-typeable
// `span.ws-index-mark` (fields in data-*, description in title/aria-label, sign from the style) and go back as
// `{t: "index_mark", mark}`; the server writes them canonically. Setting, changing and removing come as the command
// `index_mark` (ws_command.ts) from the context menu or ribbon (context_menu.ts); no import between the two.

import { t } from "./i18n.js";
import { markdownHeading } from "./markdown.js";
import { call } from "./rpc.js";
import { editToolbar, register, store, submitButton } from "./widgets.js";
import {
  citeKey,
  cleanMark,
  type IndexMark,
  type IndexMarkCommand,
  markLabel,
  markSource,
  scriptureSource,
  validateMark,
  WS_CARET,
  WS_COMMAND,
  type WsCaretDetail,
  type WsCommandDetail,
  type WsCommandName,
} from "./ws_command.js";

/// Work language of the typeset view around `element` (`data-work-lang` of fragments/editor.html, Round 39b, contract
/// §2.4); without the attribute German. Decides the words of the Typst snippets this widget inserts.
function workLang(element: Element | null): "de" | "en" {
  const holder = element?.closest<HTMLElement>("[data-work-lang]") ?? document.querySelector<HTMLElement>("[data-work-lang]");
  return holder?.dataset.workLang === "en" ? "en" : "de";
}

type Inline =
  | { t: "text"; v: string }
  | { t: "soft" }
  | { t: "linebreak" }
  | { t: "emph" | "strong" | "footnote" | "quote"; c: Inline[] }
  | { t: "label"; v: string }
  | { t: "island"; raw: string }
  | { t: "link"; url: string; c?: Inline[] }
  | { t: "xref"; key: number; c: Inline[]; page?: boolean }
  | { t: "ref"; target: string; supplement?: string }
  | { t: "index_mark"; mark: IndexMark };

type Term = { term: Inline[]; desc: Inline[] };

type BlockKind =
  | { kind: "heading"; level: number; inlines: Inline[]; label?: string }
  | { kind: "paragraph"; inlines: Inline[] }
  | { kind: "quote"; block: boolean; attribution: Inline[] | null; body: Block[] }
  | { kind: "list"; ordered: boolean; items: Inline[][]; depths?: number[] }
  | { kind: "enum"; start: number; items: Inline[][]; depths?: number[] }
  | { kind: "terms"; items: Term[] }
  | { kind: "scene" | "work"; body: Block[]; args?: string }
  | { kind: "part"; title: string }
  | { kind: "unnumbered"; level: number; title: string }
  | { kind: "island" };

type Block = BlockKind & { raw?: string };

type Edit =
  | { keep: number }
  | { replaces: number | null; block: BlockKind }
  | { replaces: number; body: Edit[] }
  | { replaces: number; raw: string };

interface Parsed {
  html: string;
}

interface Style {
  css: string;
}

const COOKIE = "editor";

/// `#fundstelle` applies only to the editor that is created on loading the page; after a redraw
/// (click in the outline, ui.render) the old address still stands there until workspace.ts replaces it.
let fundstelle = window.location.hash === "#fundstelle";

function notify(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast";
  toast.setAttribute("role", "alert");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 5000);
}

// ---- Appearance following the work template (once per work) -------------------------------
/// Running requests `wysiwyg.style` per work (only while they run; afterwards the element stands in the document).
const styling = new Map<number, Promise<void>>();

/// Only `keep` stays: all other `<style id="ws-style">` (of other works or outdated) are dropped.
function dropOtherStyles(keep: HTMLStyleElement | null): void {
  document.querySelectorAll<HTMLStyleElement>("style#ws-style").forEach((element) => {
    if (element !== keep) element.remove();
  });
}

/// Style of the work template for `work`. Round 18b: if the server already supplies it in the HTML (`<style
/// id="ws-style" data-work="W">`, typeset view per work), it is taken over without RPC; otherwise through
/// `wysiwyg.style` as before.
function applyStyle(work: number): Promise<void> {
  const own = document.querySelector<HTMLStyleElement>(`style#ws-style[data-work="${work}"]`);
  if (own) {
    dropOtherStyles(own);
    return Promise.resolve();
  }
  const running = styling.get(work);
  if (running) return running;
  const pending = call<Style>("wysiwyg.style", { work })
    .then((style) => {
      const element = document.createElement("style");
      element.id = "ws-style";
      element.dataset.work = String(work);
      element.textContent = style.css;
      dropOtherStyles(null);
      document.head.append(element);
    })
    .catch(() => {})
    .finally(() => styling.delete(work));
  styling.set(work, pending);
  return pending;
}

// ---- DOM → blocks ---------------------------------------------------------------------------
function pushText(out: Inline[], text: string): void {
  if (text === "") return;
  const last = out[out.length - 1];
  if (last && last.t === "text") last.v += text;
  else out.push({ t: "text", v: text });
}

/// Line ends in the sense of Typst (like `baruch_typst::is_newline`); the server replaces them as well.
const NEWLINES = /[\n\r\v\f\u0085\u2028\u2029]/g;

// ---- Title rules (like `baruch_store::check_title`, the server stays authoritative) ---------
const MAX_TITLE_CHARS = 200;

/// Message if a title violates the rules of the server, otherwise `null`.
function titleProblem(title: string): string | null {
  const trimmed = title.trim();
  if (trimmed === "") return t("widget.wysiwyg.title_empty");
  if (/[\n\r\v\f\u0085\u2028\u2029]/.test(trimmed)) return t("widget.wysiwyg.title_newline");
  const count = [...trimmed].length;
  if (count > MAX_TITLE_CHARS) return t("widget.wysiwyg.title_long", { max: MAX_TITLE_CHARS, count });
  return null;
}

/// Visible text of a sequence (for the title rules; islands with their raw text).
function plainText(inlines: Inline[]): string {
  return inlines
    .map((inline) => {
      switch (inline.t) {
        case "text":
          return inline.v;
        case "soft":
        case "linebreak":
          return " ";
        case "label":
        case "index_mark":
          return "";
        case "island":
          return inline.raw;
        case "link":
          return inline.c ? plainText(inline.c) : inline.url;
        case "xref":
          return plainText(inline.c);
        case "ref":
          return `@${inline.target}`;
        default:
          return plainText(inline.c);
      }
    })
    .join("");
}

/// Split a heading at the first simple break, at top level as in emph/strong (arbitrarily
/// deep): before it the title, behind it (if anything is left) a paragraph; a split
/// emphasis stands with its front part in the title and with the rear part in the paragraph. Footnotes and
/// quotations stay whole. Like `split_heading` in the server; breaks at the beginning do not count.
function splitHeading(inlines: Inline[]): { head: Inline[]; rest: Inline[] | null } {
  const { head, rest } = cutAtBreak(trimFront(inlines));
  if (rest === null) return { head, rest: null };
  const tail = trimBack(trimFront(rest));
  return { head: trimBack(head), rest: tail.length > 0 ? tail : null };
}

/// Split at the first break that would stand in the heading line (top level or in emph/strong).
function cutAtBreak(inlines: Inline[]): { head: Inline[]; rest: Inline[] | null } {
  const head: Inline[] = [];
  for (let k = 0; k < inlines.length; k++) {
    const inline = inlines[k];
    if (inline.t === "soft") return { head, rest: inlines.slice(k + 1) };
    if (inline.t !== "emph" && inline.t !== "strong") {
      head.push(inline);
      continue;
    }
    const inner = cutAtBreak(inline.c);
    if (inner.rest === null) {
      head.push({ t: inline.t, c: inner.head });
      continue;
    }
    if (inner.head.length > 0) head.push({ t: inline.t, c: inner.head });
    const rest: Inline[] = inner.rest.length > 0 ? [{ t: inline.t, c: inner.rest }] : [];
    return { head, rest: rest.concat(inlines.slice(k + 1)) };
  }
  return { head, rest: null };
}

/// Remove breaks, empty texts, empty emphases and whitespace at the beginning (also in emph/strong).
function trimFront(inlines: Inline[]): Inline[] {
  const list = inlines.slice();
  while (list.length > 0) {
    const first = list[0];
    if (first.t === "soft") list.shift();
    else if (first.t === "text") {
      const v = first.v.replace(/^[ \t]+/, "");
      if (v === "") list.shift();
      else {
        list[0] = { t: "text", v };
        break;
      }
    } else if (first.t === "emph" || first.t === "strong") {
      const c = trimFront(first.c);
      if (c.length === 0) list.shift();
      else {
        list[0] = { t: first.t, c };
        break;
      }
    } else break;
  }
  return list;
}

/// Like `trimFront`, but at the end.
function trimBack(inlines: Inline[]): Inline[] {
  const list = inlines.slice();
  while (list.length > 0) {
    const k = list.length - 1;
    const last = list[k];
    if (last.t === "soft") list.pop();
    else if (last.t === "text") {
      const v = last.v.replace(/[ \t]+$/, "");
      if (v === "") list.pop();
      else {
        list[k] = { t: "text", v };
        break;
      }
    } else if (last.t === "emph" || last.t === "strong") {
      const c = trimBack(last.c);
      if (c.length === 0) list.pop();
      else {
        list[k] = { t: last.t, c };
        break;
      }
    } else break;
  }
  return list;
}

/// Round 31c: mark from a `span.ws-index-mark` (fields in `data-register`, `data-term`, `data-sub`, `data-see`,
/// `data-also`, `data-sort`, as baruch-wysiwyg writes them); invalid → `null` (the mark is not dropped silently: the
/// context menu checks beforehand with `validateMark`).
function markOf(element: HTMLElement): IndexMark | null {
  const d = element.dataset;
  const mark = cleanMark({
    register: d.register ?? "",
    term: d.term ?? "",
    sub: d.sub,
    see: d.see,
    also: d.also,
    sort: d.sort,
  });
  return validateMark(mark) === null ? mark : null;
}

/// Round 31c: write fields, description and `data-raw` of a mark to a (new or existing) element –
/// the same form as `render_index_mark` in baruch-wysiwyg.
function fillMark(element: HTMLElement, mark: IndexMark): void {
  const clean = cleanMark(mark);
  element.className = "ws-index-mark";
  element.contentEditable = "false";
  element.setAttribute("role", "img");
  element.tabIndex = -1;
  const label = markLabel(clean);
  element.title = label;
  element.setAttribute("aria-label", label);
  for (const key of ["register", "term", "sub", "see", "also", "sort"] as const) {
    const value = clean[key];
    if (value) element.dataset[key] = value;
    else delete element.dataset[key];
  }
  element.dataset.raw = markSource(clean);
  element.textContent = "";
}

/// Mark all blocks around `node` (topmost and nested) as edited.
function dirtyAround(node: Node): void {
  for (let el = node instanceof HTMLElement ? node : node.parentElement; el; el = el.parentElement) {
    if (el.classList.contains("ws-block")) el.dataset.dirty = "1";
    if (el.hasAttribute("data-ws-sheet")) break;
  }
}

/// Running text of an element; unknown elements (span, font, div of the browser) count only with their content.
/// `heading`: the label behind the heading (`data-heading-label`) does not belong to the title.
function inlinesOf(element: Node, heading = false): Inline[] {
  const out: Inline[] = [];
  const walk = (node: Node, into: Inline[]) => {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === Node.TEXT_NODE) {
        // The browser also sets non-breaking spaces itself; real ones stand in span.ws-nbsp.
        const raw = (child.textContent ?? "").replace(/\u00a0/g, " ");
        if (heading) {
          // In a heading a line end ends it (Round 7) instead of appending text.
          raw.split(NEWLINES).forEach((piece, k) => {
            if (k > 0) into.push({ t: "soft" });
            pushText(into, piece);
          });
        } else pushText(into, raw.replace(NEWLINES, " "));
        continue;
      }
      if (!(child instanceof HTMLElement)) continue;
      const tag = child.tagName;
      const list = (t: "emph" | "strong" | "footnote" | "quote") => {
        const c: Inline[] = [];
        walk(child, c);
        if (c.length > 0) into.push({ t, c });
      };
      if (heading && child.dataset.headingLabel !== undefined) continue;
      // The source text field of an island (Round 18b) is a control, not text.
      if (child.classList.contains("ws-island-edit")) continue;
      // Round 31c: index mark (empty element, fields in data-*); checked before the island.
      if (child.classList.contains("ws-index-mark")) {
        const mark = markOf(child);
        if (mark) into.push({ t: "index_mark", mark });
      } else if (child.classList.contains("ws-island")) into.push({ t: "island", raw: child.dataset.raw ?? "" });
      else if (child.classList.contains("ws-linebreak")) into.push({ t: "linebreak" });
      else if (child.classList.contains("ws-ref")) {
        const ref: Extract<Inline, { t: "ref" }> = { t: "ref", target: child.dataset.target ?? "" };
        if (child.dataset.supplement !== undefined) ref.supplement = child.dataset.supplement;
        into.push(ref);
      } else if (child.classList.contains("ws-xref")) {
        // Round 30a: cross-reference; the page number addition (span.ws-xref-page) is display, not text.
        const key = Number(child.dataset.key);
        const c: Inline[] = [];
        const shown = child.cloneNode(true) as HTMLElement;
        shown.querySelectorAll(".ws-xref-page").forEach((p) => p.remove());
        walk(shown, c);
        if (c.length > 0 && Number.isFinite(key)) into.push({ t: "xref", key, c, page: child.dataset.page === "1" });
      } else if (child.classList.contains("ws-link")) {
        const url = child.dataset.url ?? "";
        if (child.dataset.bare !== undefined) into.push({ t: "link", url });
        else {
          // A link without text (everything deleted) is dropped with its address.
          const c: Inline[] = [];
          walk(child, c);
          if (c.length > 0) into.push({ t: "link", url, c });
        }
      }
      else if (child.classList.contains("ws-label")) into.push({ t: "label", v: child.dataset.v ?? "" });
      else if (child.classList.contains("ws-soft")) {
        into.push({ t: "soft" });
        pushText(into, (child.textContent ?? "").replace(/^ /, "").replace(/ /g, " "));
      } else if (child.classList.contains("ws-nbsp")) {
        const text = child.textContent ?? "";
        pushText(into, text === " " ? " " : text.replace(/ /g, " "));
      } else if (child.classList.contains("ws-footnote")) list("footnote");
      else if (child.classList.contains("ws-quote") || tag === "Q") list("quote");
      else if (tag === "EM" || tag === "I" || child.style.fontStyle === "italic") list("emph");
      else if (tag === "STRONG" || tag === "B" || child.style.fontWeight === "bold" || child.style.fontWeight === "700") {
        list("strong");
      } else if (tag === "BR") into.push({ t: "soft" });
      else walk(child, into);
    }
  };
  walk(element, out);
  // Breaks at the edge and double ones are dropped, whitespace at the edge too.
  const clean: Inline[] = [];
  for (const inline of out) {
    if (inline.t === "soft" && (clean.length === 0 || clean[clean.length - 1].t === "soft")) continue;
    clean.push(inline);
  }
  while (clean.length > 0 && clean[clean.length - 1].t === "soft") clean.pop();
  const first = clean[0];
  if (first && first.t === "text") first.v = first.v.replace(/^[ \t]+/, "");
  const last = clean[clean.length - 1];
  if (last && last.t === "text") last.v = last.v.replace(/[ \t]+$/, "");
  return clean.filter((inline) => inline.t !== "text" || inline.v !== "");
}

/// Blocks that are one line with a title: heading, part, „ohne Nummer“ (Round 14).
function isTitleKind(kind: string | undefined): boolean {
  return kind === "heading" || kind === "part" || kind === "unnumbered";
}

/// Title of a part or of a marker „ohne Nummer“: plain text, line ends and non-breaking
/// spaces become spaces.
function markerTitle(element: HTMLElement): string {
  return (element.textContent ?? "").replace(/\u00a0/g, " ").replace(NEWLINES, " ").trim();
}

function isBlock(node: Node): node is HTMLElement {
  return node instanceof HTMLElement && node.classList.contains("ws-block");
}

/// A piece of a proposal (details.pv) that stands in the typeset view: not part of the text.
function isProposal(node: Node): boolean {
  return node instanceof HTMLElement && node.classList.contains("pv");
}

/// Kind of a block from its element (also nested; islands with their raw text).
function kindOf(element: HTMLElement): Block | null {
  const kind = element.dataset.kind;
  const body = (): Block[] => {
    const holder = element.querySelector<HTMLElement>(":scope > .ws-body");
    return holder ? blocksIn(holder) : [];
  };
  switch (kind) {
    case "heading": {
      const label = element.querySelector<HTMLElement>(":scope > [data-heading-label]")?.dataset.v;
      const heading: Block = { kind: "heading", level: Number(element.dataset.level ?? "1"), inlines: inlinesOf(element, true) };
      if (label) heading.label = label;
      return heading;
    }
    case "list":
    case "enum": {
      // Round 18b: nested items (list in the `<li>`) with depth; without nesting no `depths`.
      const list = element.querySelector<HTMLElement>(":scope > ul, :scope > ol");
      const items: Inline[][] = [];
      const depths: number[] = [];
      if (list) listItems(list, 0, items, depths);
      if (items.length === 0) return null;
      const nested = depths.some((d) => d > 0);
      if (kind === "enum") {
        const start = Number(element.dataset.start ?? list?.getAttribute("start") ?? "1");
        const block: Extract<BlockKind, { kind: "enum" }> = { kind: "enum", start: Number.isFinite(start) ? start : 1, items };
        if (nested) block.depths = depths;
        return block;
      }
      const block: Extract<BlockKind, { kind: "list" }> = { kind: "list", ordered: element.dataset.ordered === "true", items };
      if (nested) block.depths = depths;
      return block;
    }
    case "terms": {
      // Round 18b: term list as `<dl>`; every `<dt>` begins an entry, `<dd>` is its explanation.
      const items: Term[] = [];
      for (const child of Array.from(element.querySelector(":scope > dl")?.children ?? [])) {
        if (child.tagName === "DT") items.push({ term: inlinesOf(child), desc: [] });
        else if (child.tagName === "DD" && items.length > 0) items[items.length - 1].desc.push(...inlinesOf(child));
      }
      const kept = items.filter((item) => item.term.length > 0);
      return kept.length > 0 ? { kind: "terms", items: kept } : null;
    }
    case "quote": {
      const attribution = element.querySelector<HTMLElement>(":scope > .ws-attribution");
      return {
        kind: "quote",
        block: element.dataset.block === "true",
        attribution: attribution ? inlinesOf(attribution) : null,
        body: body(),
      };
    }
    case "scene":
    case "work": {
      const args = element.querySelector<HTMLElement>(":scope > .ws-args")?.dataset.raw;
      const block: Block = { kind, body: body() };
      if (args !== undefined) block.args = args;
      return block;
    }
    case "part":
      return { kind: "part", title: markerTitle(element) };
    case "unnumbered":
      return { kind: "unnumbered", level: Number(element.dataset.level ?? "1"), title: markerTitle(element) };
    case "island":
      return { kind: "island", raw: element.dataset.raw ?? "" };
    default: {
      const inlines = inlinesOf(element);
      return inlines.length > 0 ? { kind: "paragraph", inlines } : null;
    }
  }
}

/// Items of a list in order with their depth (Round 18b): per `<li>` the text without nested lists, then
/// the nested lists one level deeper (also a list directly in the list, as browsers indent).
function listItems(list: Element, depth: number, items: Inline[][], depths: number[]): void {
  for (const child of Array.from(list.children)) {
    if (child.tagName === "UL" || child.tagName === "OL") {
      listItems(child, depth + 1, items, depths);
      continue;
    }
    if (child.tagName !== "LI") continue;
    const own = child.cloneNode(true) as HTMLElement;
    own.querySelectorAll(":scope > ul, :scope > ol").forEach((nested) => nested.remove());
    const inlines = inlinesOf(own);
    if (inlines.length > 0) {
      items.push(inlines);
      depths.push(depth);
    }
    for (const nested of Array.from(child.children)) {
      if (nested.tagName === "UL" || nested.tagName === "OL") listItems(nested, depth + 1, items, depths);
    }
  }
}

/// Nested blocks of a body; loose texts (inserted by the browser) become paragraphs.
function blocksIn(holder: HTMLElement): Block[] {
  const out: Block[] = [];
  for (const child of Array.from(holder.childNodes)) {
    if (isProposal(child)) continue;
    const block = isBlock(child) ? kindOf(child) : looseParagraph(child);
    if (block) out.push(block);
  }
  return out;
}

function looseParagraph(node: Node): Block | null {
  const holder = document.createElement("div");
  holder.append(node.cloneNode(true));
  const inlines = inlinesOf(holder);
  return inlines.length > 0 ? { kind: "paragraph", inlines } : null;
}

/// Number the body blocks of a scene/work order (`data-n`, index in the body as on the server).
function numberBodies(sheet: HTMLElement): void {
  sheet.querySelectorAll<HTMLElement>(".ws-body").forEach((body) => {
    let n = 0;
    for (const child of Array.from(body.children)) {
      if (isBlock(child)) child.dataset.n = String(n++);
    }
  });
}

/// Instructions for wysiwyg.serialize: `keep` for every block without input, otherwise the new block; for
/// an edited scene/work order instructions for its body (only the edited paragraph becomes new).
/// A second block with the same index (copied by the browser) counts as new. `holder`: the typeset view
/// (index `data-b`) or a body (`data-n`).
function editsOf(holder: HTMLElement, nested = false, upTo: Node | null = null): Edit[] {
  const edits: Edit[] = [];
  const seen = new Set<number>();
  for (const child of Array.from(holder.childNodes)) {
    // `upTo` (Round 18b, cursor position): only the blocks up to and including this one.
    if (upTo && child !== upTo && upTo.compareDocumentPosition(child) & Node.DOCUMENT_POSITION_FOLLOWING) break;
    // Proposals in the typeset view do not belong to the text.
    if (isProposal(child)) continue;
    if (!isBlock(child)) {
      const block = looseParagraph(child);
      if (block) edits.push({ replaces: null, block });
      continue;
    }
    const mark = nested ? child.dataset.n : child.dataset.b;
    const index = mark === undefined ? null : Number(mark);
    const fresh = index === null || seen.has(index);
    if (index !== null) seen.add(index);
    // Round 18b: an island changed in place goes verbatim as `{replaces, raw}`.
    if (!fresh && child.dataset.kind === "island" && child.dataset.edited === "1") {
      edits.push({ replaces: index as number, raw: child.dataset.raw ?? "" });
      continue;
    }
    if (!fresh && (child.dataset.dirty !== "1" || child.dataset.kind === "island")) {
      edits.push({ keep: index as number });
      continue;
    }
    const inner = child.querySelector<HTMLElement>(":scope > .ws-body");
    if (!fresh && inner && (child.dataset.kind === "scene" || child.dataset.kind === "work")) {
      edits.push({ replaces: index as number, body: editsOf(inner, true) });
      continue;
    }
    const block = kindOf(child);
    if (!block || block.kind === "island") continue;
    if (block.kind === "heading") {
      edits.push(...headingEdits(block, fresh ? null : index));
      continue;
    }
    const kind: Block = { ...block };
    delete kind.raw;
    edits.push({ replaces: fresh ? null : index, block: kind });
  }
  return edits;
}

/// A heading block: the heading up to the first break, behind it (if present) a paragraph.
function headingEdits(block: Block & { kind: "heading" }, replaces: number | null): Edit[] {
  const { head, rest } = splitHeading(block.inlines);
  const heading: Extract<BlockKind, { kind: "heading" }> = { kind: "heading", level: block.level, inlines: head };
  if (block.label) heading.label = block.label;
  const edits: Edit[] = [{ replaces, block: heading }];
  if (rest) edits.push({ replaces: null, block: { kind: "paragraph", inlines: rest } });
  return edits;
}

/// Title rules for all headings in the typeset view (levels 1–3 are outline items); `null` = good.
function headingProblem(sheet: HTMLElement): string | null {
  for (const child of Array.from(sheet.children)) {
    if (!(child instanceof HTMLElement) || !isBlock(child)) continue;
    if (child.dataset.kind === "part" || child.dataset.kind === "unnumbered") {
      const problem = titleProblem(markerTitle(child));
      if (problem) return problem;
      continue;
    }
    if (child.dataset.kind !== "heading") continue;
    if (Number(child.dataset.level ?? "1") > 3) continue;
    const { head } = splitHeading(inlinesOf(child, true));
    const problem = titleProblem(plainText(head));
    if (problem) return problem;
  }
  return null;
}

/// Pasted text → paragraphs (separated at empty lines, simple line ends become spaces).
function pastedParagraphs(text: string): string[] {
  return text
    .split(/\n[ \t]*\n\s*/)
    .map((part) => part.replace(NEWLINES, " ").trim())
    .filter((part) => part !== "");
}

// ---- Widget -----------------------------------------------------------------------------------
interface State {
  form: HTMLFormElement;
  area: HTMLTextAreaElement;
  sheet: HTMLElement;
  work: number;
  /// Text from which the typeset view is built (the indexes `data-b` refer to it).
  parsed: string;
  /// Typeset view open?
  open: boolean;
  /// Input in the typeset view since the last transfer into the text field?
  touched: boolean;
  /// Proposals put into the typeset view with their place (comment) in the reading flow.
  placed: { item: HTMLElement; home: Comment }[];
}

const states = new WeakMap<HTMLFormElement, State>();

/// Typeset view mounted last with its format choice (for `selectionchange`, once per page).
let current: { state: State; format: HTMLSelectElement | null } | null = null;

document.addEventListener("selectionchange", () => {
  if (!current || !current.format || !current.state.open || !current.state.sheet.isConnected) return;
  const block = topBlock(current.state.sheet, document.getSelection()?.anchorNode ?? null);
  if (!block) return;
  current.format.value = isTitleKind(block.dataset.kind) && block.dataset.kind !== "part" ? (block.dataset.level ?? "1") : "paragraph";
});

/// Markdown heading in the typeset view (user decision 2026-11-07): a paragraph whose text begins with `# `
/// to `###### ` becomes a heading of this level (4–6 → 3, rule in `markdown.ts`); the
/// hash signs and a closing sequence ` ##` are dropped, the cursor stays at its place in the title.
/// `true` if converted.
function convertMarkdownHeading(state: State, block: HTMLElement): boolean {
  if (block.dataset.kind !== "paragraph" || block.closest("li")) return false;
  const found = markdownHeading((block.textContent ?? "").replace(/\n/g, " "));
  if (!found) return false;
  const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT);
  const first = walker.nextNode() as Text | null;
  const prefix = first ? /^#{1,6} /.exec(first.data) : null;
  if (!first || !prefix) return false;
  const selection = document.getSelection();
  const caret = selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
  const caretOffset = caret && caret.startContainer === first ? caret.startOffset : null;
  first.data = first.data.slice(prefix[0].length);
  let last: Text = first;
  for (let node = walker.nextNode(); node; node = walker.nextNode()) last = node as Text;
  const closing = /[ \t]+#+\s*$/.exec(last.data);
  if (closing) last.data = last.data.slice(0, closing.index);
  if (caretOffset !== null && selection) {
    const place = document.createRange();
    place.setStart(first, Math.max(0, Math.min(first.data.length, caretOffset - prefix[0].length)));
    place.collapse(true);
    selection.removeAllRanges();
    selection.addRange(place);
  }
  block.dataset.kind = "heading";
  block.dataset.level = String(found.level);
  block.dataset.dirty = "1";
  markTouched(state);
  return true;
}

/// Transfer the typeset view into the text field (wysiwyg.serialize), then `input` on the text field. Before that
/// paragraphs with a Markdown heading are converted (for whoever saves without Enter).
async function sync(state: State): Promise<void> {
  if (!state.open || !state.touched) return;
  for (const child of Array.from(state.sheet.children)) {
    if (child instanceof HTMLElement && isBlock(child)) convertMarkdownHeading(state, child);
  }
  const problem = headingProblem(state.sheet);
  if (problem) throw new Error(problem);
  const result = await call<{ body: string }>("wysiwyg.serialize", {
    work: state.work,
    body: state.parsed,
    edits: editsOf(state.sheet),
  });
  state.touched = false;
  state.area.value = result.body;
  state.area.dispatchEvent(new Event("input", { bubbles: true }));
}

/// Source text of the open typeset view like `sync`, but without consequences (contract Round 17 §2.6): no converting
/// of Markdown headings in the typeset view, text field and `touched` stay as they are. Without input since the
/// last transfer the source text already stands in the text field.
async function serializeText(state: State): Promise<string> {
  if (!state.touched) return state.area.value;
  const result = await call<{ body: string }>("wysiwyg.serialize", {
    work: state.work,
    body: state.parsed,
    edits: editsOf(state.sheet),
  });
  return result.body;
}

/// Topmost block in which a node lies.
function topBlock(sheet: HTMLElement, node: Node | null): HTMLElement | null {
  let current: Node | null = node;
  while (current && current.parentNode !== sheet) current = current.parentNode;
  return current && isBlock(current) ? current : null;
}

/// Mark all blocks that the selection touches as edited: topmost and nested ones (in the
/// body of a scene) so that only these are generated anew.
function markSelection(state: State): void {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  state.sheet.querySelectorAll<HTMLElement>(".ws-block").forEach((block) => {
    // Blocks in the typeset view of a proposal (details.pv) are not the text.
    if (block.closest(".pv")) return;
    if (range.intersectsNode(block)) block.dataset.dirty = "1";
  });
}

/// Does the (empty) cursor stand at the beginning or end of the block?
function caretAt(block: HTMLElement, range: Range, edge: "start" | "end"): boolean {
  if (!range.collapsed) return false;
  const probe = document.createRange();
  probe.selectNodeContents(block);
  if (edge === "start") probe.setEnd(range.startContainer, range.startOffset);
  else probe.setStart(range.endContainer, range.endOffset);
  return probe.toString().replace(NEWLINES, "") === "";
}

/// Neighbouring block before or behind; pieces of the proposals (`details.pv`) in between do not count (Round 18b:
/// otherwise the protection below did not take hold when a proposal stood between heading and paragraph).
function blockNeighbour(block: HTMLElement, backward: boolean): HTMLElement | null {
  let node = backward ? block.previousElementSibling : block.nextElementSibling;
  while (node && isProposal(node)) node = backward ? node.previousElementSibling : node.nextElementSibling;
  return node instanceof HTMLElement ? node : null;
}

/// Backspace at the beginning of a paragraph behind a heading or Del at the end of a heading
/// before a paragraph: the browser would pull the paragraph into the heading (Round 7). Instead only
/// an empty paragraph is dropped; otherwise everything stays as it is. `true` = intercepted.
function guardHeadingMerge(state: State, event: InputEvent): boolean {
  if (event.inputType !== "deleteContentBackward" && event.inputType !== "deleteContentForward") return false;
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return false;
  const range = selection.getRangeAt(0);
  const block = topBlock(state.sheet, range.startContainer);
  if (!block) return false;
  const backward = event.inputType === "deleteContentBackward";
  const neighbour = blockNeighbour(block, backward);
  if (!neighbour || !isBlock(neighbour)) return false;
  const [heading, other] = backward ? [neighbour, block] : [block, neighbour];
  if (!isTitleKind(heading.dataset.kind) || isTitleKind(other.dataset.kind) || other.dataset.kind === "island") {
    return false;
  }
  if (!caretAt(block, range, backward ? "start" : "end")) return false;
  event.preventDefault();
  if ((other.textContent ?? "").trim() === "") {
    other.remove();
    markTouched(state);
    const caret = document.createRange();
    caret.selectNodeContents(heading);
    caret.collapse(false);
    selection.removeAllRanges();
    selection.addRange(caret);
  }
  return true;
}

function markTouched(state: State): void {
  state.touched = true;
  state.form.dataset.dirty = "1";
  const save = submitButton(state.form);
  if (save) {
    save.disabled = false;
    save.textContent = save.dataset.dirty ?? save.textContent;
  }
}

/// Enter: split the block at the cursor (new paragraph without index). In lists the browser makes a
/// new item.
function splitBlock(state: State): void {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  const start = range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement;
  const block = start?.closest<HTMLElement>(".ws-block");
  if (!block || !state.sheet.contains(block)) return;
  range.deleteContents();
  const tail = document.createRange();
  tail.setStart(range.startContainer, range.startOffset);
  // The label behind a heading stays with it.
  const label = block.querySelector<HTMLElement>(":scope > [data-heading-label]");
  if (label && range.comparePoint(label, 0) > 0) tail.setEndBefore(label);
  else tail.setEnd(block, block.childNodes.length);
  const rest = tail.extractContents();
  const fresh = document.createElement("div");
  fresh.className = "ws-block";
  fresh.dataset.kind = "paragraph";
  fresh.dataset.dirty = "1";
  fresh.append(rest);
  // An empty block needs a placeholder, otherwise the browser does not put the cursor into it.
  if (!fresh.textContent) fresh.append(document.createElement("br"));
  block.after(fresh);
  const top = topBlock(state.sheet, block);
  if (top) top.dataset.dirty = "1";
  block.dataset.dirty = "1";
  const caret = document.createRange();
  caret.setStart(fresh, 0);
  caret.collapse(true);
  selection.removeAllRanges();
  selection.addRange(caret);
}

/// Shift+Enter: simple line break (a space in the typesetting, a new line in the source text).
function insertSoft(): void {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  range.deleteContents();
  const soft = document.createElement("span");
  soft.className = "ws-soft";
  soft.textContent = " ";
  range.insertNode(soft);
  range.setStartAfter(soft);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

/// Paragraph format of the block at the cursor (only paragraph and heading).
function setFormat(state: State, value: string): void {
  const selection = document.getSelection();
  const block = topBlock(state.sheet, selection?.anchorNode ?? null);
  if (block && (block.dataset.kind === "part" || block.dataset.kind === "unnumbered")) {
    notify(t("widget.wysiwyg.format_part"));
    return;
  }
  if (!block || (block.dataset.kind !== "paragraph" && block.dataset.kind !== "heading")) {
    notify(t("widget.wysiwyg.format_only"));
    return;
  }
  if (value === "paragraph") {
    block.dataset.kind = "paragraph";
    delete block.dataset.level;
  } else {
    block.dataset.kind = "heading";
    block.dataset.level = value;
  }
  block.dataset.dirty = "1";
  markTouched(state);
}

/// Reading flow with the proposals of this editor (section.pv-flow next to the form), if there.
function flowOf(state: State): HTMLElement | null {
  return state.form.parentElement?.querySelector<HTMLElement>(":scope > section.pv-flow") ?? null;
}

/// First block of the typeset view with `data-b` ≥ `gap` (new blocks without index do not count).
function blockFrom(sheet: HTMLElement, gap: number): HTMLElement | null {
  for (const child of Array.from(sheet.children)) {
    if (child instanceof HTMLElement && isBlock(child) && child.dataset.b !== undefined && Number(child.dataset.b) >= gap) {
      return child;
    }
  }
  return null;
}

/// Put pieces of the proposals from the reading flow at their place in the typeset view: before the block
/// `[data-b=gap]` (at the end behind it), in the order of the server (at the same place one below the other
/// by persona, within that by number), not editable. The reading flow is hidden.
function placeProposals(state: State): void {
  const flow = flowOf(state);
  if (!flow) return;
  const items = Array.from(flow.querySelectorAll<HTMLElement>("details.pv[data-pv-gap]"));
  for (const item of items) {
    const home = document.createComment("pv");
    item.before(home);
    state.placed.push({ item, home });
    item.contentEditable = "false";
    const target = blockFrom(state.sheet, Number(item.dataset.pvGap));
    if (target) target.before(item);
    else state.sheet.append(item);
  }
  flow.hidden = true;
}

/// Pieces back to their place in the reading flow (before a new typeset view and in the source text); show the reading
/// flow.
function returnProposals(state: State): void {
  for (const { item, home } of state.placed) {
    item.removeAttribute("contenteditable");
    home.replaceWith(item);
  }
  state.placed = [];
  const flow = flowOf(state);
  if (flow) flow.hidden = false;
}

/// Open a typeset view whose HTML already stands in the sheet (from the server or from wysiwyg.parse).
function sheetReady(state: State, root: HTMLElement): void {
  numberBodies(state.sheet);
  state.parsed = state.area.value;
  state.touched = false;
  state.open = true;
  show(state, root);
  placeProposals(state);
}

async function openSheet(state: State, root: HTMLElement): Promise<boolean> {
  try {
    const [parsed] = await Promise.all([
      call<Parsed>("wysiwyg.parse", { work: state.work, body: state.area.value }),
      applyStyle(state.work),
    ]);
    returnProposals(state);
    state.sheet.innerHTML = parsed.html;
    sheetReady(state, root);
    return true;
  } catch (error) {
    notify(String((error as Error).message ?? error));
    return false;
  }
}

/// When mounting: open the finished typeset view of the server (`data-ws-ready`, Round 8) at once without
/// showing the source text first; the style of the work template follows. `false` if it is missing.
function adoptSheet(state: State, root: HTMLElement): boolean {
  if (!state.sheet.hasAttribute("data-ws-ready")) return false;
  state.sheet.removeAttribute("data-ws-ready");
  sheetReady(state, root);
  void applyStyle(state.work);
  return true;
}

function show(state: State, root: HTMLElement): void {
  // Round 33: after docking the bar stands in the ribbon (band.ts), no longer under `root`.
  const bar = editToolbar(root);
  bar?.querySelectorAll<HTMLButtonElement>("[data-ws-mode]").forEach((button) => {
    button.setAttribute("aria-pressed", String((button.dataset.wsMode === "satz") === state.open));
  });
  const tools = bar?.querySelector<HTMLElement>("[data-ws-tools]");
  if (tools) tools.hidden = !state.open;
  const toolsOff = bar?.querySelector<HTMLElement>("[data-ws-source-note]");
  if (toolsOff) toolsOff.hidden = state.open;
  const hint = root.querySelector<HTMLElement>("[data-ws-hint]");
  if (hint) hint.hidden = !state.open;
  const wrap = root.querySelector<HTMLElement>("[data-ws-wrap]");
  if (wrap) wrap.hidden = !state.open;
  // Only the open typeset view grows (wysiwyg.css); in the source text the text field takes the room.
  root.toggleAttribute("data-ws-open", state.open);
  state.area.hidden = state.open;
}

// ---- Round 18b: commands of the ribbon, cursor position, islands in place ----------------------

/// Selection last seen in the typeset view per form (a click into the ribbon takes the focus).
const lastRanges = new WeakMap<State, Range>();

/// Does the node lie in the text of the typeset view (not in a proposal, not in the source text field of an island)?
function inSheetText(state: State, node: Node | null): boolean {
  const element = node instanceof Element ? node : (node?.parentElement ?? null);
  return !!element && state.sheet.contains(element) && !element.closest(".pv, .ws-island-edit");
}

/// Selection in the typeset view: the current one, otherwise the one last seen (restored), otherwise `null`.
function sheetRange(state: State): Range | null {
  const selection = document.getSelection();
  if (selection && selection.rangeCount > 0 && inSheetText(state, selection.getRangeAt(0).startContainer)) {
    return selection.getRangeAt(0);
  }
  const last = lastRanges.get(state);
  if (!last || !inSheetText(state, last.startContainer)) return null;
  state.sheet.focus();
  selection?.removeAllRanges();
  selection?.addRange(last);
  return last;
}

/// Put a new element at the end of a block or at the selection and place the cursor into it.
function placeCaret(node: Node, select: boolean): void {
  const selection = document.getSelection();
  if (!selection) return;
  const range = document.createRange();
  if (select) range.selectNodeContents(node);
  else {
    range.setStartAfter(node);
    range.collapse(true);
  }
  selection.removeAllRanges();
  selection.addRange(range);
}

/// Inline element at the selection: the selection goes into it, otherwise `placeholder` (selected for overwriting).
function wrapInline(range: Range, element: HTMLElement, placeholder: string): void {
  if (range.collapsed) element.textContent = placeholder;
  else element.append(range.extractContents());
  range.insertNode(element);
  placeCaret(element, range.collapsed || element.textContent === placeholder);
}

/// New block (without index) of kind `kind`.
function newBlock(kind: string): HTMLElement {
  const block = document.createElement("div");
  block.className = "ws-block";
  block.dataset.kind = kind;
  block.dataset.dirty = "1";
  return block;
}

/// Follow-up fix R31: does the same mark already stand in the chain of marks directly at `at` (like `mark_word` in the
/// server)?
export function sameMarkBehind(at: Range, mark: IndexMark): boolean {
  const node = at.startContainer;
  const offset = at.startOffset;
  let next: Node | null =
    node.nodeType === Node.TEXT_NODE
      ? offset >= (node as Text).data.length
        ? node.nextSibling
        : null
      : (node.childNodes[offset] ?? null);
  const raw = markSource(cleanMark(mark));
  while (next) {
    if (next.nodeType === Node.TEXT_NODE && (next as Text).data === "") {
      next = next.nextSibling;
      continue;
    }
    if (!(next instanceof HTMLElement) || !next.classList.contains("ws-index-mark")) return false;
    if (next.dataset.raw === raw) return true;
    next = next.nextSibling;
  }
  return false;
}

/// Round 31c: end of the selected word as an empty position – a selection without whitespace at the end, without a
/// selection the end of the word at the cursor. The mark stands there ("directly behind the selected word").
function wordEnd(range: Range): Range {
  const end = document.createRange();
  let node = range.endContainer;
  let offset = range.endOffset;
  if (node.nodeType === Node.TEXT_NODE) {
    const data = (node as Text).data;
    if (range.collapsed) {
      while (offset < data.length && /[\p{L}\p{N}\p{M}_'’-]/u.test(data[offset])) offset++;
    } else {
      while (offset > 0 && /\s/.test(data[offset - 1])) offset--;
    }
  }
  end.setStart(node, offset);
  end.collapse(true);
  return end;
}

/// Round 31c: command `index_mark` (`arg` = JSON {op, mark}, ws_command.ts). `add` puts a new mark behind the
/// selected word (not in a heading, part, „ohne Nummer“, island, reference, label or another mark), `edit`
/// and `remove` act on the mark with `data-ctx-target` (set by the context menu). The block is generated anew, the
/// editor is unsaved afterwards; saving is done with „Speichern“ as a new revision.
function runIndexMark(state: State, arg?: string): void {
  let command: IndexMarkCommand;
  try {
    command = JSON.parse(arg ?? "{}") as IndexMarkCommand;
  } catch {
    notify(t("widget.wysiwyg.mark_unreadable"));
    return;
  }
  const mark = cleanMark(command.mark ?? { register: "", term: "" });
  if (command.op === "edit" || command.op === "remove") {
    const target = state.sheet.querySelector<HTMLElement>(".ws-index-mark[data-ctx-target]");
    state.sheet.querySelectorAll("[data-ctx-target]").forEach((el) => el.removeAttribute("data-ctx-target"));
    if (!target || target.closest(".pv")) {
      notify(t("widget.wysiwyg.mark_gone"));
      return;
    }
    if (command.op === "edit") {
      const problem = validateMark(mark);
      if (problem) {
        notify(problem);
        return;
      }
      fillMark(target, mark);
      dirtyAround(target);
    } else {
      dirtyAround(target);
      target.remove();
    }
    markTouched(state);
    return;
  }
  const problem = validateMark(mark);
  if (problem) {
    notify(problem);
    return;
  }
  const range = sheetRange(state);
  const top = range ? topBlock(state.sheet, range.startContainer) : null;
  if (!range || !top) {
    notify(t("widget.wysiwyg.select_word"));
    return;
  }
  const at = range.endContainer instanceof Element ? range.endContainer : range.endContainer.parentElement;
  if (isTitleKind(top.dataset.kind) || top.dataset.kind === "island") {
    notify(t("widget.wysiwyg.mark_heading"));
    return;
  }
  if (at?.closest(".ws-island, .ws-index-mark, .ws-ref, .ws-label, .ws-linebreak")) {
    notify(t("widget.wysiwyg.mark_here"));
    return;
  }
  const end = wordEnd(range);
  if (sameMarkBehind(end, mark)) {
    notify(t("widget.wysiwyg.mark_duplicate"));
    return;
  }
  const element = document.createElement("span");
  fillMark(element, mark);
  end.insertNode(element);
  placeCaret(element, false);
  dirtyAround(element);
  markTouched(state);
}

/// Carry out a command of the ribbon in the open typeset view (contract R18-S1, `baruch:ws-command`). Every command
/// counts as done as long as the typeset view is open – even if it does not work at that place (then with a
/// message), so that the sender does not write into the hidden text field.
function runCommand(state: State, cmd: WsCommandName, arg?: string): void {
  if (cmd === "index_mark") {
    runIndexMark(state, arg);
    return;
  }
  const range = sheetRange(state);
  const top = range ? topBlock(state.sheet, range.startContainer) : null;
  const textual = (block: HTMLElement | null) =>
    !!block && ["paragraph", "heading", "list", "enum", "terms", "quote"].includes(block.dataset.kind ?? "");
  const inlineTarget = range && top && (textual(top) || top.dataset.kind === "scene" || top.dataset.kind === "work");
  const done = () => {
    if (top) top.dataset.dirty = "1";
    markSelection(state);
    markTouched(state);
  };
  switch (cmd) {
    case "footnote":
    case "quote":
    case "break":
    case "xref":
    case "scripture":
    case "cite":
    case "link": {
      if (!range || !inlineTarget) {
        notify(t("widget.wysiwyg.cursor_first"));
        return;
      }
      if (cmd === "scripture") {
        // Round 19b: Bible passage following the work template as an island `#fn[Stelle]` (changeable only as a whole,
        // as in the typesetting).
        let choice: { fn?: unknown; text?: unknown } = {};
        try {
          choice = JSON.parse(arg ?? "{}");
        } catch {
          choice = {};
        }
        const fn = typeof choice.fn === "string" ? choice.fn : "";
        const text = typeof choice.text === "string" ? choice.text : (window.prompt(t("widget.wysiwyg.prompt_scripture")) ?? "");
        if (text.trim() === "") return;
        const raw = scriptureSource(fn, text);
        if (!raw) {
          notify(t("widget.wysiwyg.scripture_refused"));
          return;
        }
        range.deleteContents();
        const island = document.createElement("span");
        island.className = "ws-island";
        island.contentEditable = "false";
        island.dataset.raw = raw;
        island.title = t("widget.wysiwyg.island");
        island.textContent = raw;
        range.insertNode(island);
        placeCaret(island, false);
      } else if (cmd === "cite") {
        // Round 19b: literature reference `@schlüssel` like a reference from the source text (button.ws-ref).
        const key = citeKey(arg ?? window.prompt(t("widget.wysiwyg.prompt_cite")) ?? "");
        if (!key) {
          if (arg !== undefined) notify(t("widget.wysiwyg.cite_invalid"));
          return;
        }
        range.deleteContents();
        const ref = document.createElement("button");
        ref.type = "button";
        ref.className = "ws-ref";
        ref.contentEditable = "false";
        ref.tabIndex = -1;
        ref.dataset.target = key;
        ref.title = t("widget.wysiwyg.ref_title", { target: `<${key}>` });
        ref.textContent = `@${key}`;
        range.insertNode(ref);
        placeCaret(ref, false);
      } else if (cmd === "break") {
        range.deleteContents();
        const brk = document.createElement("span");
        brk.className = "ws-linebreak";
        brk.contentEditable = "false";
        brk.dataset.linebreak = "1";
        brk.title = t("widget.wysiwyg.linebreak");
        brk.append("↵", document.createElement("br"));
        range.insertNode(brk);
        placeCaret(brk, false);
      } else if (cmd === "xref") {
        // Round 30a: cross-reference from the dialog (xref.ts), `arg` = JSON {key, text, page}.
        let choice: { key?: unknown; text?: unknown; page?: unknown } = {};
        try {
          choice = JSON.parse(arg ?? "{}");
        } catch {
          choice = {};
        }
        if (typeof choice.key !== "number" || typeof choice.text !== "string") return;
        const xref = document.createElement("span");
        xref.className = "ws-xref";
        xref.dataset.key = String(choice.key);
        xref.dataset.page = choice.page === true ? "1" : "0";
        xref.title = t("widget.wysiwyg.xref_title", { key: `k${choice.key}` });
        wrapInline(range, xref, choice.text);
        if (choice.page === true) {
          const suffix = document.createElement("span");
          suffix.className = "ws-xref-page";
          suffix.contentEditable = "false";
          // Round 39b: page abbreviation of the work language (book catalog `book.xref.page_abbr`).
          suffix.textContent = `, ${workLang(xref) === "en" ? "p." : "S."}\u00a0…`;
          xref.append(suffix);
        }
      } else if (cmd === "link") {
        const url = (arg ?? window.prompt(t("widget.wysiwyg.prompt_link")) ?? "").trim();
        if (!url) return;
        const link = document.createElement("a");
        link.className = "ws-link";
        link.dataset.url = url;
        link.title = t("widget.wysiwyg.link_title", { url });
        wrapInline(range, link, url);
      } else if (cmd === "quote" && range.collapsed && top) {
        // Round 23a (finding of the user): without a selection a quotation block is created as a block of its own
        // behind the block at the cursor, below it a paragraph „– “ for the reference (both new, so with empty lines in
        // the source text). With a selection it stays a quotation in the running text (`#quote[…]`).
        const box = newBlock("quote");
        box.dataset.block = "true";
        const body = document.createElement("div");
        body.className = "ws-body";
        const paragraph = newBlock("paragraph");
        paragraph.textContent = arg ?? t("widget.wysiwyg.placeholder_quote");
        body.append(paragraph);
        box.append(body);
        const reference = newBlock("paragraph");
        // Round 39b: an English work gets the em dash usual before a source in English books.
        reference.textContent = workLang(top) === "en" ? "—\u00a0" : "–\u00a0";
        top.after(box, reference);
        placeCaret(paragraph, true);
        markTouched(state);
        return;
      } else {
        const element = document.createElement(cmd === "footnote" ? "span" : "q");
        element.className = cmd === "footnote" ? "ws-footnote" : "ws-quote";
        wrapInline(range, element, arg ?? (cmd === "footnote" ? t("widget.wysiwyg.placeholder_footnote") : t("widget.wysiwyg.placeholder_quote")));
      }
      done();
      return;
    }
    case "list":
    case "enum":
    case "terms": {
      if (!top || (top.dataset.kind !== "paragraph" && top.dataset.kind !== "list" && top.dataset.kind !== "enum")) {
        notify(t("widget.wysiwyg.list_from_paragraph"));
        return;
      }
      if (top.dataset.kind !== "paragraph") {
        // List ↔ numbered list: only the kind and the list element change, the items stay.
        if (cmd === "terms") {
          notify(t("widget.wysiwyg.list_not_terms"));
          return;
        }
        const old = top.querySelector<HTMLElement>(":scope > ul, :scope > ol");
        const fresh = document.createElement(cmd === "enum" ? "ol" : "ul");
        if (old) {
          fresh.append(...Array.from(old.childNodes));
          old.replaceWith(fresh);
        }
        top.dataset.kind = cmd;
        if (cmd === "enum") top.dataset.start = "1";
        else top.dataset.ordered = "false";
        done();
        return;
      }
      const content = Array.from(top.childNodes);
      top.textContent = "";
      top.dataset.kind = cmd;
      if (cmd === "terms") {
        const dl = document.createElement("dl");
        const dt = document.createElement("dt");
        dt.textContent = arg ?? t("widget.wysiwyg.placeholder_term");
        const dd = document.createElement("dd");
        dd.append(...content);
        dl.append(dt, dd);
        top.append(dl);
        placeCaret(dt, true);
      } else {
        const list = document.createElement(cmd === "enum" ? "ol" : "ul");
        const li = document.createElement("li");
        li.append(...content);
        list.append(li);
        top.append(list);
        if (cmd === "enum") top.dataset.start = "1";
        else top.dataset.ordered = "false";
        placeCaret(li, false);
      }
      done();
      return;
    }
    case "scene":
    case "work": {
      if (top && (top.dataset.kind === "scene" || top.dataset.kind === "work")) {
        notify(t("widget.wysiwyg.scene_exists"));
        return;
      }
      const box = newBlock(cmd);
      const body = document.createElement("div");
      body.className = "ws-body";
      box.append(body);
      if (top && textual(top) && top.dataset.kind !== "heading") {
        // The block at the cursor moves into the body (new there, without index); the old block is dropped.
        top.before(box);
        delete top.dataset.b;
        top.dataset.dirty = "1";
        body.append(top);
      } else {
        const paragraph = newBlock("paragraph");
        paragraph.append(document.createElement("br"));
        body.append(paragraph);
        if (top) top.after(box);
        else state.sheet.append(box);
        placeCaret(paragraph, true);
      }
      markTouched(state);
      return;
    }
  }
}

/// Source text of the open typeset view (like `baruch:serialize-text`) and the UTF-16 position of the line start
/// behind the topmost block at the cursor (contract R18-S1, `baruch:ws-caret`). The position is the length of the
/// text that the same instructions yield up to and including this block (with its empty lines); without a cursor or
/// behind the last block the end.
async function caretPlace(state: State): Promise<{ body: string; offset: number }> {
  const body = await serializeText(state);
  const range = sheetRange(state);
  const block = range ? topBlock(state.sheet, range.startContainer) : null;
  if (!block) return { body, offset: body.length };
  const prefix = await call<{ body: string }>("wysiwyg.serialize", {
    work: state.work,
    body: state.parsed,
    edits: editsOf(state.sheet, false, block),
  });
  const head = prefix.body;
  const fits = body.startsWith(head) && (head.endsWith("\n") || head.length === body.length);
  return { body, offset: fits ? head.length : body.length };
}

/// Source text field of an island in place (double click): accepting sets `data-raw` and marks the island
/// (`data-edited`), cancelling or Escape restores it. Keys in the field do not go to the typeset view.
function editIsland(state: State, island: HTMLElement): void {
  if (island.querySelector(".ws-island-edit") || island.closest(".pv")) return;
  const saved = Array.from(island.childNodes);
  const raw = (island.dataset.raw ?? "").replace(/\s+$/, "");
  const editor = document.createElement(island.tagName === "SPAN" ? "span" : "div");
  editor.className = "ws-island-edit";
  editor.contentEditable = "false";
  const area = document.createElement("textarea");
  area.value = raw;
  area.setAttribute("aria-label", t("widget.wysiwyg.island_source"));
  area.rows = Math.min(12, Math.max(2, raw.split("\n").length + 1));
  const buttons = document.createElement("span");
  buttons.className = "ws-island-buttons";
  const ok = document.createElement("button");
  ok.type = "button";
  ok.className = "btn small";
  ok.textContent = t("widget.wysiwyg.apply");
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "btn small";
  cancel.textContent = t("widget.wysiwyg.cancel");
  buttons.append(ok, cancel);
  editor.append(area, buttons);
  island.replaceChildren(editor);
  const close = (apply: boolean) => {
    if (!island.contains(editor)) return;
    const value = area.value.replace(/\s+$/, "");
    if (apply && value !== raw) {
      island.dataset.raw = island.tagName === "SPAN" ? value : `${value}\n`;
      island.dataset.edited = "1";
      island.dataset.dirty = "1";
      // The island shows its new raw text (an image only after saving as an image again).
      const code = document.createElement("code");
      code.className = "ws-raw";
      code.textContent = value;
      island.replaceChildren(island.tagName === "SPAN" ? document.createTextNode(value) : code);
      island.classList.remove("ws-inline-image", "ws-figure");
      // Islands in the sentence belong to their paragraph; it is generated anew with the new island.
      if (island.tagName === "SPAN") {
        let holder = island.parentElement?.closest<HTMLElement>(".ws-block") ?? null;
        while (holder) {
          holder.dataset.dirty = "1";
          holder = holder.parentElement?.closest<HTMLElement>(".ws-block") ?? null;
        }
      }
      const top = topBlock(state.sheet, island);
      if (top) top.dataset.dirty = "1";
      markTouched(state);
    } else island.replaceChildren(...saved);
  };
  for (const type of ["keydown", "keyup", "keypress", "beforeinput", "input", "paste"]) {
    area.addEventListener(type, (event) => event.stopPropagation());
  }
  area.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close(false);
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      close(true);
    }
  });
  // Whoever clicks elsewhere or saves accepts (blur comes before submitting).
  area.addEventListener("change", () => close(true));
  ok.addEventListener("click", () => close(true));
  cancel.addEventListener("mousedown", (event) => event.preventDefault());
  cancel.addEventListener("click", () => close(false));
  area.focus();
}

/// Enter in a term list: a new entry (term and explanation) behind the current one; `true` = done.
function newTerm(state: State): boolean {
  const anchor = document.getSelection()?.anchorNode ?? null;
  const cell = (anchor instanceof Element ? anchor : anchor?.parentElement)?.closest<HTMLElement>("dt, dd");
  if (!cell || !state.sheet.contains(cell)) return false;
  const dd = cell.tagName === "DT" ? (cell.nextElementSibling?.tagName === "DD" ? cell.nextElementSibling : cell) : cell;
  const dt = document.createElement("dt");
  dt.append(document.createElement("br"));
  const desc = document.createElement("dd");
  dd.after(dt, desc);
  const top = topBlock(state.sheet, cell);
  if (top) top.dataset.dirty = "1";
  placeCaret(dt, true);
  markTouched(state);
  return true;
}

/// Round 30a: mark the text `find` in the typeset view (highlight „baruch-find“) and scroll to it; Typst escapes and
/// whitespace runs do not count. Without the Highlight API only scroll to it.
function markFound(sheet: HTMLElement, find: string): void {
  const plain = find.replace(/\\(.)/g, "$1").replace(/[_*]/g, "").trim();
  if (!plain) return;
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT);
  const nodes: { node: Text; start: number }[] = [];
  let all = "";
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    nodes.push({ node: n as Text, start: all.length });
    all += (n as Text).data;
  }
  const words = plain.split(/\s+/).map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const match = new RegExp(words.join("\\s+")).exec(all.replace(/\u00a0/g, " "));
  if (!match) return;
  const point = (pos: number) => {
    for (let i = nodes.length - 1; i >= 0; i--) {
      if (nodes[i].start <= pos) return { node: nodes[i].node, offset: Math.min(pos - nodes[i].start, nodes[i].node.data.length) };
    }
    return null;
  };
  const a = point(match.index);
  const b = point(match.index + match[0].length);
  if (!a || !b) return;
  const range = document.createRange();
  range.setStart(a.node, a.offset);
  range.setEnd(b.node, b.offset);
  a.node.parentElement?.scrollIntoView({ block: "center" });
  const registry = typeof CSS !== "undefined" && "highlights" in CSS ? (CSS as unknown as { highlights: Map<string, unknown> }).highlights : null;
  const Ctor = (window as unknown as { Highlight?: new (...r: Range[]) => unknown }).Highlight;
  if (registry && Ctor) registry.set("baruch-find", new Ctor(range));
}

register("wysiwyg", (root) => {
  // The widget comprises toolbar, sheet (with the form) and status line (editor.html).
  const form = root.querySelector<HTMLFormElement>("form[data-widget=editor]");
  const area = form?.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  const sheet = root.querySelector<HTMLElement>("[data-ws-sheet]");
  if (!form || !area || !sheet) return;
  const state: State = {
    form,
    area,
    sheet,
    work: Number(root.dataset.work),
    parsed: area.value,
    open: false,
    touched: false,
    placed: [],
  };
  states.set(form, state);
  // Default typeset view; source text only after an explicit choice (cookie) or for the place found on loading.
  const source = fundstelle || store.get(COOKIE) === "quelltext";
  fundstelle = false;
  // The toolbar (fragments/edit_toolbar.html) is hidden without JavaScript.
  const toolbar = root.querySelector<HTMLElement>("[data-edit-toolbar]");
  if (toolbar) toolbar.hidden = false;
  // Typeset view from the server: open right away. Otherwise show the source text only if it is wanted; the typeset
  // view through RPC opens below without switching to the source text first.
  if (source || !adoptSheet(state, root)) {
    if (source) show(state, root);
    else state.area.hidden = true;
  }

  root.querySelectorAll<HTMLButtonElement>("[data-ws-mode]").forEach((button) => {
    button.addEventListener("click", async () => {
      const satz = button.dataset.wsMode === "satz";
      if (satz === state.open) return;
      // Explicit choice: only it sets the cookie.
      store.set(COOKIE, satz ? "satz" : "quelltext");
      if (satz) {
        await openSheet(state, root);
        state.sheet.focus();
        return;
      }
      try {
        await sync(state);
      } catch (error) {
        notify(String((error as Error).message ?? error));
        return;
      }
      state.open = false;
      returnProposals(state);
      show(state, root);
      state.area.focus();
    });
  });

  root.querySelectorAll<HTMLButtonElement>("[data-ws-cmd]").forEach((button) => {
    // The selection in the typeset view is kept on the click.
    button.addEventListener("mousedown", (event) => event.preventDefault());
    button.addEventListener("click", () => {
      markSelection(state);
      document.execCommand(button.dataset.wsCmd ?? "", false);
      markTouched(state);
    });
  });
  const format = root.querySelector<HTMLSelectElement>("[data-ws-format]");
  format?.addEventListener("change", () => {
    setFormat(state, format.value);
    state.sheet.focus();
  });
  current = { state, format };

  // Round 18b: commands of the ribbon (ws_command.ts). Without bubbles on the area; done only with the typeset view
  // open.
  root.addEventListener(WS_COMMAND, (event) => {
    const detail = (event as CustomEvent<WsCommandDetail>).detail;
    if (!detail || !state.open || !state.sheet.isConnected) return;
    detail.handled = true;
    runCommand(state, detail.cmd, detail.arg);
  });
  // Cursor position for images (ws_command.ts): exactly one promise, even if the event arrives at the form.
  const answered = new WeakSet<Event>();
  const onCaret = (event: Event) => {
    const detail = (event as CustomEvent<WsCaretDetail>).detail;
    if (answered.has(event) || !state.open || !Array.isArray(detail?.wait)) return;
    answered.add(event);
    detail.wait.push(caretPlace(state));
  };
  root.addEventListener(WS_CARET, onCaret);
  form.addEventListener(WS_CARET, onCaret);
  // Round 30a: mark the place found of a jump (jump.ts, `baruch:find {text, at}`) in the typeset view – through the CSS
  // Custom Highlight API, without changing the DOM; search.ts takes care of the source text. If the typeset view is
  // not open yet (it loads through RPC), there is a short wait.
  const onFind = (event: Event) => {
    const detail = (event as CustomEvent<{ text?: unknown }>).detail;
    if (typeof detail?.text !== "string" || !root.isConnected) return;
    const text = detail.text;
    let tries = 0;
    const attempt = () => {
      if (!root.isConnected) return;
      if (!state.open) {
        if (++tries < 30 && state.area.hidden) window.setTimeout(attempt, 100);
        return;
      }
      markFound(state.sheet, text);
    };
    attempt();
  };
  document.addEventListener("baruch:find", onFind);
  document.addEventListener("selectionchange", () => {
    const selection = document.getSelection();
    if (state.open && selection && selection.rangeCount > 0 && inSheetText(state, selection.getRangeAt(0).startContainer)) {
      lastRanges.set(state, selection.getRangeAt(0).cloneRange());
    }
  });
  // Islands in place: a double click opens the source text field (also images; not islands in proposals).
  sheet.addEventListener("dblclick", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const island = target?.closest<HTMLElement>(".ws-island, .ws-block[data-kind=island]");
    if (!island || !sheet.contains(island) || island.closest(".pv") || target?.closest(".ws-island-edit")) return;
    event.preventDefault();
    editIsland(state, island);
  });
  // A reference is a button: the click shows the target (label in the typeset view) but changes nothing.
  sheet.addEventListener("click", (event) => {
    const ref = event.target instanceof Element ? event.target.closest<HTMLElement>("button.ws-ref") : null;
    if (!ref || !sheet.contains(ref)) return;
    event.preventDefault();
    const target = ref.dataset.target ?? "";
    const label = Array.from(sheet.querySelectorAll<HTMLElement>(".ws-label")).find((l) => l.dataset.v === target);
    if (label) label.scrollIntoView({ block: "center" });
    else notify(t("widget.wysiwyg.ref_elsewhere", { target: `<${target}>` }));
  });

  sheet.addEventListener("beforeinput", (event) => {
    if (guardHeadingMerge(state, event as InputEvent)) return;
    markSelection(state);
  });
  sheet.addEventListener("input", () => {
    markSelection(state);
    markTouched(state);
  });
  sheet.addEventListener("paste", (event) => {
    // Paste text only; foreign HTML would not be Typst. Empty lines separate paragraphs: every further
    // piece begins a new paragraph as with Enter (in lists it stays an item).
    event.preventDefault();
    const text = (event.clipboardData?.getData("text/plain") ?? "").replace(/\r\n?/g, "\n");
    markSelection(state);
    const parts = pastedParagraphs(text);
    const anchor = document.getSelection()?.anchorNode ?? null;
    const inList = (anchor?.parentElement ?? null)?.closest("li");
    const inHeading = isTitleKind(topBlock(state.sheet, anchor)?.dataset.kind);
    if (inHeading && NEWLINES.test(text.trim())) {
      // Several lines in a heading: they belong below it, never into the heading line.
      NEWLINES.lastIndex = 0;
      for (const part of text.split(/\n\s*\n|\n/).map((p) => p.replace(NEWLINES, " ").trim()).filter((p) => p !== "")) {
        splitBlock(state);
        document.execCommand("insertText", false, part);
      }
      markTouched(state);
      return;
    }
    NEWLINES.lastIndex = 0;
    if (parts.length < 2 || inList) {
      document.execCommand("insertText", false, text.replace(NEWLINES, " "));
      return;
    }
    document.execCommand("insertText", false, parts[0]);
    for (const part of parts.slice(1)) {
      splitBlock(state);
      document.execCommand("insertText", false, part);
    }
    markTouched(state);
  });
  sheet.addEventListener("keydown", (event) => {
    // Ctrl+S saves through the listener on the document (panes.ts), the focus may be anywhere.
    if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === "i" || event.key.toLowerCase() === "b")) {
      markSelection(state);
      markTouched(state);
      return;
    }
    if (event.key !== "Enter" || event.isComposing) return;
    const inList = (document.getSelection()?.anchorNode?.parentElement ?? null)?.closest("li");
    markSelection(state);
    markTouched(state);
    if (inList && !event.shiftKey) return;
    event.preventDefault();
    // Round 18b: in a term list Enter begins a new entry.
    if (!event.shiftKey && newTerm(state)) return;
    // Enter completes a Markdown heading (`# Titel` → heading), then a paragraph as always.
    const current = topBlock(state.sheet, document.getSelection()?.anchorNode ?? null);
    if (!event.shiftKey && current) convertMarkdownHeading(state, current);
    // In a heading Shift+Enter also begins a new paragraph (Round 7).
    const inHeading = isTitleKind(topBlock(state.sheet, document.getSelection()?.anchorNode ?? null)?.dataset.kind);
    if (event.shiftKey && !inHeading) insertSoft();
    else splitBlock(state);
  });

  // Before saving: typeset view into the text field (contract Round 5, 3b; triggered by outline.ts).
  form.addEventListener("baruch:before-save", (event) => {
    const detail = (event as CustomEvent<{ wait?: Promise<unknown>[] }>).detail;
    if (state.open && state.touched && Array.isArray(detail?.wait)) detail.wait.push(sync(state));
  });

  // Complete source text without saving (contract Round 17 §2.6, e.g. for the live count in
  // timing.ts): only with the typeset view open does the widget put a promise into `wait`; text field and `touched`
  // stay unchanged. If nobody is responsible, `wait` stays empty (fallback at the trigger).
  form.addEventListener("baruch:serialize-text", (event) => {
    const detail = (event as CustomEvent<{ wait?: Promise<string>[] }>).detail;
    if (state.open && Array.isArray(detail?.wait)) detail.wait.push(serializeText(state));
  });

  if (!source && !state.open) {
    void openSheet(state, root).then((opened) => {
      // If opening fails, the source text stays (show the text field again).
      if (!opened) show(state, root);
    });
  }
});

// Fallback in case nobody triggers `baruch:before-save`: before every other listener (window, capture
// phase) transfer the typeset view and then submit again.
window.addEventListener(
  "submit",
  (event) => {
    const form = event.target instanceof HTMLFormElement ? event.target : null;
    const state = form ? states.get(form) : undefined;
    if (!form || !state || !state.open || !state.touched) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    // Round 22: the triggering button („Als neue Fassung speichern“) goes along on submitting again.
    const submitter = (event as SubmitEvent).submitter;
    sync(state)
      .then(() => {
        if (submitter instanceof HTMLButtonElement && submitter.form === form) form.requestSubmit(submitter);
        else form.requestSubmit();
      })
      .catch((error) => notify(String((error as Error).message ?? error)));
  },
  true,
);
