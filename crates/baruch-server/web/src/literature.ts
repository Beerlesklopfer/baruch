// SPDX-License-Identifier: AGPL-3.0-or-later
// Literature with JavaScript. Without JavaScript everything goes through the pages /literatur and /quelle and
// the form to POST /literatur/act.
//
// - Widget „literature“ (<dialog> in the header): the button „Literatur“ (link to
//   /literatur) and Alt+L open the popup. Content through `literature.render`; the search is kept across
//   closing and opening and stands as `lq` in the address. A work hit closes the
//   popup and opens the section in the workspace. The switch „Suche nach“ (Round 12, radio
//   `lmode` = fts | rag) goes along to `literature.render` and stands as `lmode=rag` in the address;
//   „Dieselbe Suche im Wortlaut (FTS)“ loads the verbatim search in the popup.
// - Place found: if `#fundstelle` stands in the address (work hit), the first search word is marked in the
//   text field of the editor and jumped to. Round 18c: if the typeset view is open (work hit from the
//   popup), the CSS Custom Highlight API marks the place found in the typeset view (without a DOM change); without
//   the API or without a hit in the typeset view the marking stays in the (hidden) text field as before.
// - Widget „source“ (document view /quelle): context menu on selected text with Hervorheben,
//   Zitieren, An Persona geben (Round 15: RPC `literature.to_persona` with the persona and question from the form,
//   then to the section) and Ähnliches suchen. Text and place found (quotation, position, character range) go the
//   same ways as the form. Round 15: also in the full text of a linked source – start and end
//   count there from `data-offset` of the part (like the number fields without JavaScript).
// - Section: quoting names it through the fixed key `key` (`data-key` of the document view),
//   never through the position; the position `node` in addresses is display only.

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { mount, register } from "./widgets.js";

/// Lower-case letters without diacritics (like the search: „langmutig“ hits „langmütig“).
function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

function toast(message: string): void {
  const element = document.createElement("div");
  element.className = "toast";
  element.setAttribute("role", "alert");
  element.textContent = message;
  document.body.append(element);
  setTimeout(() => element.remove(), 6000);
}

function errorText(error: unknown): string {
  return String((error as Error)?.message ?? error);
}

/// Field from the current address as a number.
function numberParam(name: string): number | undefined {
  const value = new URL(window.location.href).searchParams.get(name);
  return value === null || value === "" || Number.isNaN(Number(value)) ? undefined : Number(value);
}

/// Fixed key of the open section: from the address, otherwise from the editor form.
function keyParam(): number | undefined {
  const fromUrl = numberParam("key");
  if (fromUrl !== undefined) return fromUrl;
  const field = document.querySelector<HTMLInputElement>("form.editor-form input[name=key]");
  return field && field.value.trim() !== "" && !Number.isNaN(Number(field.value)) ? Number(field.value) : undefined;
}

function lqParam(): string | undefined {
  const lq = new URL(window.location.href).searchParams.get("lq");
  return lq === null || lq.trim() === "" ? undefined : lq;
}

/// Kind of search (Round 12): `rag` = meaning, otherwise verbatim (`fts`, default, not in the address).
type SearchMode = "fts" | "rag";

function modeOf(value: string | null | undefined): SearchMode {
  return value === "rag" ? "rag" : "fts";
}

function lmodeParam(): SearchMode {
  return modeOf(new URL(window.location.href).searchParams.get("lmode"));
}

// ---- Place found in the editor -----------------------------------------------------------

/// First occurrence of `word` in `text`, regardless of case and accents.
/// Result as [start, end) in UTF-16 units of `text` (like `setSelectionRange`).
export function findFolded(text: string, word: string): [number, number] | null {
  const needle = fold(word);
  if (needle === "") return null;
  let folded = "";
  const origin: number[] = [];
  let i = 0;
  for (const ch of text) {
    const f = fold(ch);
    for (let k = 0; k < f.length; k++) origin.push(i);
    folded += f;
    i += ch.length;
  }
  origin.push(i);
  const at = folded.indexOf(needle);
  if (at < 0) return null;
  const last = origin[at + needle.length - 1];
  const end = last + (text.codePointAt(last)! > 0xffff ? 2 : 1);
  return [origin[at], end];
}

/// Can the browser highlight ranges (CSS Custom Highlight API)?
function highlightApi(): boolean {
  return typeof CSS !== "undefined" && "highlights" in CSS && typeof (window as { Highlight?: unknown }).Highlight === "function";
}

const HIGHLIGHT = "literatur-fundstelle";

/// Mark the place found in the open typeset view (Round 18c): through the CSS Custom Highlight API (`::highlight(
/// literatur-fundstelle)`, static/sheet.css) – only a Range object, the DOM of the typeset view stays unchanged (it
/// is saved). The search runs in the text of the blocks, without the inserted proposals (`.pv`). `true` if one
/// of the search words is found and marked.
function markInSheet(sheet: HTMLElement, words: string[]): boolean {
  if (!highlightApi()) return false;
  const nodes: { node: Text; start: number }[] = [];
  let whole = "";
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => (node.parentElement?.closest(".pv") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    nodes.push({ node: text, start: whole.length });
    whole += text.data;
  }
  const locate = (offset: number, end: boolean): [Text, number] | null => {
    for (const { node, start } of nodes) {
      const stop = start + node.data.length;
      if (end ? offset <= stop : offset < stop) return [node, offset - start];
    }
    return null;
  };
  const registry = (CSS as unknown as { highlights: Map<string, unknown> }).highlights;
  for (const word of words) {
    const found = findFolded(whole, word.replace(/[^\p{L}\p{N}]/gu, ""));
    if (!found) continue;
    const a = locate(found[0], false);
    const b = locate(found[1], true);
    if (!a || !b) continue;
    const range = document.createRange();
    range.setStart(a[0], a[1]);
    range.setEnd(b[0], b[1]);
    const Ctor = (window as unknown as { Highlight: new (...ranges: Range[]) => unknown }).Highlight;
    registry.set(HIGHLIGHT, new Ctor(range));
    a[0].parentElement?.scrollIntoView({ block: "center" });
    // Typing in the typeset view takes the marking away (otherwise the place shifts underneath it).
    sheet.addEventListener("input", () => registry.delete(HIGHLIGHT), { once: true });
    return true;
  }
  return false;
}

/// Marks the place found of the search `lq` in the editor and scrolls there: with the typeset view open in the
/// typeset view (Round 18c), otherwise in the text field. If the typeset view is still opening (through RPC), the
/// marking happens after opening.
function markFundstelle(lq: string | undefined): boolean {
  if (!lq) return false;
  const area = document.querySelector<HTMLTextAreaElement>("#pane-editor textarea[name=body]");
  if (!area) return false;
  const words = lq.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w));
  const root = area.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
  const sheet = root?.querySelector<HTMLElement>("[data-ws-sheet]");
  if (root && sheet && root.hasAttribute("data-ws-open")) {
    if (markInSheet(sheet, words)) return true;
  } else if (root && sheet && area.hidden) {
    // The typeset view is opening right now (wysiwyg.ts through RPC): mark only afterwards, wait at most 10 s.
    const observer = new MutationObserver(() => {
      if (!root.hasAttribute("data-ws-open")) return;
      observer.disconnect();
      markInSheet(sheet, words);
    });
    observer.observe(root, { attributes: true, attributeFilter: ["data-ws-open"] });
    setTimeout(() => observer.disconnect(), 10000);
    return true;
  }
  for (const word of words) {
    const found = findFolded(area.value, word.replace(/[^\p{L}\p{N}]/gu, ""));
    if (!found) continue;
    area.focus({ preventScroll: true });
    area.setSelectionRange(found[0], found[1]);
    const lines = area.value.slice(0, found[0]).split("\n").length - 1;
    const height = parseFloat(getComputedStyle(area).lineHeight) || 20;
    area.scrollTop = Math.max(0, lines * height - area.clientHeight / 3);
    area.scrollIntoView({ block: "nearest" });
    area.classList.add("fundstelle");
    return true;
  }
  return false;
}

/// After the redraw of the editor (ui.render) mark the place found once.
function markAfterRender(lq: string | undefined): void {
  const pane = document.getElementById("pane-editor");
  if (!pane) return;
  const observer = new MutationObserver(() => {
    observer.disconnect();
    markFundstelle(lq);
  });
  observer.observe(pane, { childList: true });
  setTimeout(() => observer.disconnect(), 10000);
}

/// Open a work hit in the workspace: through a `data-nav` link in the widget „workspace“
/// (without reloading); if it is missing (another page), navigate in the ordinary way.
function openWorkHit(href: string): void {
  const url = new URL(href, window.location.href);
  const lq = url.searchParams.get("lq") ?? undefined;
  const workspace = document.querySelector<HTMLElement>("[data-widget=workspace]");
  if (!workspace || url.pathname !== "/") {
    window.location.assign(url.toString());
    return;
  }
  url.hash = "";
  const link = document.createElement("a");
  link.href = url.toString();
  link.dataset.nav = "";
  link.hidden = true;
  workspace.append(link);
  markAfterRender(lq);
  link.click();
  link.remove();
}

// ---- Popup -------------------------------------------------------------------------------

interface RenderedLiterature {
  html: string;
  lq: string | null;
  lmode?: string;
}

register("literature", (element) => {
  const dialog = element as HTMLDialogElement;
  const content = dialog.querySelector<HTMLElement>(".literature-content");
  const opener = document.querySelector<HTMLAnchorElement>("[data-literature-open]");
  if (!content || typeof dialog.showModal !== "function") return;
  // Search of the popup; the default is that of the address. Kept across closing and opening.
  let query: string | undefined = lqParam();
  let mode: SearchMode = lmodeParam();
  let loaded = false;

  const load = async (lq: string | undefined, lmode: SearchMode): Promise<void> => {
    try {
      const result = await call<RenderedLiterature>("literature.render", {
        work: numberParam("work"),
        node: numberParam("node"),
        key: keyParam(),
        rev: numberParam("rev"),
        lq: lq ?? "",
        lmode,
      });
      content.innerHTML = result.html;
      mount(content);
      query = result.lq ?? undefined;
      mode = modeOf(result.lmode);
      loaded = true;
      // Search and kind of search stand in the address so that navigation and reloading keep them.
      const url = new URL(window.location.href);
      if (query) url.searchParams.set("lq", query);
      else url.searchParams.delete("lq");
      if (mode === "rag") url.searchParams.set("lmode", "rag");
      else url.searchParams.delete("lmode");
      history.replaceState(history.state, "", url.toString());
    } catch (error) {
      toast(errorText(error));
    }
  };

  const open = (): void => {
    if (!dialog.open) dialog.showModal();
    if (!loaded) void load(query, mode);
    content.querySelector<HTMLInputElement>('input[name="lq"]')?.focus();
  };

  opener?.addEventListener("click", (event) => {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    open();
  });
  document.addEventListener("keydown", (event) => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.key.toLowerCase() !== "l") return;
    event.preventDefault();
    if (dialog.open) dialog.close();
    else open();
  });
  // Search form in the popup: through literature.render instead of the page /literatur.
  content.addEventListener("submit", (event) => {
    const form = (event.target as Element).closest<HTMLFormElement>("form.library-search");
    if (!form) return;
    event.preventDefault();
    const data = new FormData(form);
    const lq = String(data.get("lq") ?? "");
    void load(lq.trim() === "" ? undefined : lq, modeOf(String(data.get("lmode") ?? "")));
  });
  // „Dieselbe Suche im Wortlaut (FTS)“ is a link to /literatur; in the popup through literature.render.
  content.addEventListener("click", (event) => {
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[data-fts-link]");
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    void load(query, "fts");
  });
  // Work hit: popup closed, section in the workspace; sources open the document view.
  content.addEventListener("click", (event) => {
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[data-hit=work]");
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    dialog.close();
    openWorkHit(link.href);
  });
});

// ---- Context menu in the document view ---------------------------------------------------

/// Selection inside a quotation: quotation id, position and character range (Unicode characters from 0).
interface Picked {
  text: string;
  quote?: number;
  locator: string;
  start?: number;
  end?: number;
}

function codePoints(text: string): number {
  return Array.from(text).length;
}

function picked(root: HTMLElement): Picked | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  const text = selection.toString();
  if (text.trim() === "" || !root.contains(range.commonAncestorContainer)) return null;
  const node = range.commonAncestorContainer;
  const element = node instanceof Element ? node : node.parentElement;
  // Full text (Round 15): range from the start of the part plus `data-offset` (characters from the start of the
  // document).
  const full = element?.closest<HTMLElement>(".fulltext-text");
  if (full && full.contains(range.startContainer) && full.contains(range.endContainer)) {
    const base = Number(full.dataset.offset ?? "0") || 0;
    const at = (container: Node, offset: number): number => {
      const upto = document.createRange();
      upto.selectNodeContents(full);
      upto.setEnd(container, offset);
      return base + codePoints(upto.toString());
    };
    const start = at(range.startContainer, range.startOffset);
    const end = at(range.endContainer, range.endOffset);
    return { text: range.toString(), locator: "", start, end: Math.max(start, end) };
  }
  const quote = element?.closest<HTMLElement>("[data-quote]");
  const body = quote?.querySelector<HTMLElement>(".source-text");
  if (!quote || !body || !body.contains(range.startContainer) || !body.contains(range.endContainer)) {
    return { text, locator: "" };
  }
  // Positions from the DOM range, not from `selection.toString()`: the selection collapses line breaks and
  // whitespace runs according to CSS, `Range.toString()` supplies the text of the text nodes literally.
  // Start and end therefore both count from the start of the quotation, and `text` is the literal
  // excerpt; the server additionally matches whitespace-robustly and stores the quotation excerpt.
  const offset = (container: Node, at: number): number => {
    const upto = document.createRange();
    upto.selectNodeContents(body);
    upto.setEnd(container, at);
    return codePoints(upto.toString());
  };
  const start = offset(range.startContainer, range.startOffset);
  const end = offset(range.endContainer, range.endOffset);
  return {
    text: range.toString(),
    quote: Number(quote.dataset.quote),
    locator: quote.dataset.locator ?? "",
    start,
    end: Math.max(start, end),
  };
}

register("source", (root) => {
  const work = root.dataset.work ? Number(root.dataset.work) : undefined;
  const node = root.dataset.node ? Number(root.dataset.node) : undefined;
  const key = root.dataset.key ? Number(root.dataset.key) : undefined;
  const title = Number(root.dataset.title);
  const lq = root.dataset.lq;
  const lmode = modeOf(root.dataset.lmode);
  const form = document.querySelector<HTMLFormElement>("form[data-source-form]");
  const place = (): string => {
    const query = new URLSearchParams();
    if (work !== undefined) query.set("work", String(work));
    // Key form (Round 8): the position only if there is no key.
    if (key !== undefined) query.set("key", String(key));
    else if (node !== undefined) query.set("node", String(node));
    return query.toString();
  };
  const withLq = (url: string): string =>
    (lq ? `${url}&lq=${encodeURIComponent(lq)}` : url) + (lmode === "rag" ? "&lmode=rag" : "");

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.setAttribute("role", "menu");
  menu.hidden = true;
  const kinds = form?.querySelector<HTMLSelectElement>("select[name=kind]");
  const kind = document.createElement("select");
  kind.setAttribute("aria-label", t("widget.literature.kind_label"));
  if (kinds) kind.innerHTML = kinds.innerHTML;
  else kind.append(new Option(t("widget.literature.kind_core"), "core"));
  kind.value = "core";
  menu.append(kind);
  const entries: Array<[string, string]> = [
    ["highlight", t("widget.literature.highlight")],
    ["cite", t("widget.literature.cite")],
    ["persona", t("widget.literature.persona")],
    ["similar", t("widget.literature.similar")],
  ];
  for (const [action, label] of entries) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn";
    button.dataset.action = action;
    button.setAttribute("role", "menuitem");
    button.textContent = label;
    menu.append(button);
  }
  document.body.append(menu);
  let current: Picked | null = null;

  const close = (): void => {
    menu.hidden = true;
    current = null;
  };

  const add = async (picked: Picked, as: string): Promise<number> => {
    const result = await call<{ highlight: { id: number } }>("highlight.add", {
      work,
      title,
      quote: picked.quote,
      locator: picked.locator,
      start: picked.start,
      end: picked.end,
      text: picked.text,
      kind: as,
    });
    return result.highlight.id;
  };

  const run = async (action: string, picked: Picked): Promise<void> => {
    switch (action) {
      case "highlight": {
        const id = await add(picked, kind.value);
        const quote = picked.quote !== undefined ? `&quote=${picked.quote}` : "";
        window.location.assign(withLq(`/quelle?title=${title}&${place()}${quote}&highlight=${id}`) + `#hervorhebung-${id}`);
        break;
      }
      case "cite": {
        // Without a section no quotation (formerly section 0); check before creating the highlight.
        if (key === undefined) {
          toast(t("widget.literature.no_section_cite"));
          break;
        }
        const id = await add(picked, "quote");
        const result = await call<{ node: number; key: number; rev: number }>("highlight.cite", { work, highlight: id, key });
        window.location.assign(withLq(`/?work=${work ?? ""}&key=${result.key}&rev=${result.rev}`));
        break;
      }
      case "persona": {
        // Without a section no answer in the section chat; the server says the same on the notice page.
        if (key === undefined) {
          toast(t("widget.literature.no_section_persona"));
          break;
        }
        const field = (name: string): string => {
          const element = form?.elements.namedItem(name);
          return element instanceof HTMLInputElement || element instanceof HTMLSelectElement ? element.value : "";
        };
        const label = root.dataset.sourceLabel ?? t("widget.literature.source");
        const source = picked.locator ? `${label}, ${picked.locator}` : label;
        toast(t("widget.literature.answering"));
        await call("literature.to_persona", {
          work,
          key,
          persona: field("persona"),
          question: field("question"),
          text: picked.text,
          source,
          title,
        });
        window.location.assign(withLq(`/?work=${work ?? ""}&key=${key}`));
        break;
      }
      case "similar": {
        const words = Array.from(picked.text.matchAll(/[\p{L}\p{N}]{3,}/gu), (m) => m[0]);
        const longest = [...new Set(words)].sort((a, b) => b.length - a.length).slice(0, 3);
        const chosen = words.filter((w, i) => longest.includes(w) && words.indexOf(w) === i);
        const tail = lmode === "rag" ? "&lmode=rag" : "";
        window.location.assign(`/literatur?${place()}&lq=${encodeURIComponent(chosen.join(" "))}${tail}`);
        break;
      }
    }
  };

  root.addEventListener("contextmenu", (event) => {
    const found = picked(root);
    if (!found) return;
    event.preventDefault();
    current = found;
    // Fill the form below right along (also for working without the menu).
    if (form) {
      const set = (name: string, value: string) => {
        const field = form.elements.namedItem(name);
        if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement || field instanceof HTMLSelectElement) {
          field.value = value;
        }
      };
      set("text", found.text);
      set("quote", found.quote !== undefined ? String(found.quote) : "");
      set("locator", found.locator);
      set("start", found.start !== undefined ? String(found.start) : "");
      set("end", found.end !== undefined ? String(found.end) : "");
    }
    menu.hidden = false;
    const x = Math.min(event.clientX, window.innerWidth - menu.offsetWidth - 8);
    const y = Math.min(event.clientY, window.innerHeight - menu.offsetHeight - 8);
    menu.style.left = `${Math.max(0, x) + window.scrollX}px`;
    menu.style.top = `${Math.max(0, y) + window.scrollY}px`;
    menu.querySelector<HTMLButtonElement>("button")?.focus();
  });
  menu.addEventListener("click", (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>("button[data-action]");
    if (!button || !current) return;
    const chosen = current;
    close();
    run(button.dataset.action ?? "", chosen).catch((error) => toast(errorText(error)));
  });
  document.addEventListener("pointerdown", (event) => {
    if (!menu.hidden && !menu.contains(event.target as Node)) close();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !menu.hidden) close();
  });
});

// Work hit from /literatur or the popup: mark the place found after loading.
if (window.location.hash === "#fundstelle") {
  markFundstelle(lqParam());
}
