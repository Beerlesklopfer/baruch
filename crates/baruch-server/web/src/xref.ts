// SPDX-License-Identifier: AGPL-3.0-or-later
// Cross-references in the editor (Round 30a, contract §3.3): dialog „Verweis“ (ribbon, `cmd = "xref"`), Ctrl+click
// and „Öffnen | Bearbeiten“.
//
// - Dialog: search in the tree through RPC `xref.targets {q}`, link text, checkbox „mit Seitenzahl“. Since Round 29c
//   the checkbox starts with the work setting `xref_with_page` (RPC `work.typeset_get`, Einstellungen → Werk → Satz →
//   Querverweise); only if it is unset does the choice remembered in the browser apply as before (localStorage
//   `baruch_xref_page`, only in try/catch), and only then does the browser remember a change. Inserting: in the open
//   typeset view through `sendWsCommand(root, "xref", JSON {key, text, page})` as a block `span.ws-xref`, otherwise as
//   Typst text (`#link(<kN>)[…]`, with page number `…, S.~#context counter(page).at(<kN>).first()]`) at the cursor
//   position in the source text.
// - Ctrl+click on a reference in the typeset view (`span.ws-xref`) or on `<kN>`/`#link(<kN>)` in the source text
//   jumps to the target (jump.ts history through an imagined `a[data-jump]`); a simple click on the block in the
//   typeset view opens „Öffnen | Bearbeiten“ (Bearbeiten: change target/text/page number in the dialog).

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { sendWsCommand } from "./ws_command.js";

interface Target {
  key: number;
  label: string;
  number: string;
  title: string;
  level: number;
  snippet: string;
}

const PAGE_KEY = "baruch_xref_page";

/// Typst escaping of a link text (like baruch_typst::escape_title, without trimming).
export function escapeText(text: string): string {
  let out = "";
  const chars = Array.from(text.replace(/[\r\n\u2028\u2029]/g, " "));
  chars.forEach((c, i) => {
    const comment = c === "/" && (chars[i + 1] === "/" || chars[i + 1] === "*");
    if ("\\#*_$@<>[]`~".includes(c) || comment) out += "\\";
    out += c;
  });
  return out;
}

/// Reference form like baruch_typst::xref::xref_snippet (`text` already Typst).
export function snippet(key: number, text: string, page: boolean): string {
  const label = `k${key}`;
  return page
    ? `#link(<${label}>)[${text}, S.~#context counter(page).at(<${label}>).first()]`
    : `#link(<${label}>)[${text}]`;
}

/// Key of the reference or of the jump mark at the position `at` in the source text (`<k12>` or `#link(<k12>)[…]`).
export function keyAt(text: string, at: number): number | null {
  const pattern = /#link\(<k(\d+)>\)\[|<k(\d+)>/g;
  for (const m of text.matchAll(pattern)) {
    const start = m.index ?? 0;
    let end = start + m[0].length;
    if (m[0].startsWith("#link")) {
      // Up to the closing bracket of the link text.
      let depth = 1;
      let i = end;
      while (i < text.length && depth > 0) {
        if (text[i] === "\\") i++;
        else if (text[i] === "[") depth++;
        else if (text[i] === "]") depth--;
        i++;
      }
      end = i;
    }
    if (at >= start && at <= end) return Number(m[1] ?? m[2]);
  }
  return null;
}

/// Initial value of the checkbox „mit Seitenzahl“: an existing reference (Bearbeiten) keeps its form, otherwise the
/// work setting `xref_with_page` applies, and only if it is unset (`null`/`undefined`) the choice in the browser.
export function initialPage(existing: boolean | undefined, setting: boolean | null | undefined, stored: boolean): boolean {
  if (existing !== undefined) return existing;
  if (typeof setting === "boolean") return setting;
  return stored;
}

/// Work setting `xref_with_page` (`null` = unset or not readable).
async function workSetting(work: number | null): Promise<boolean | null> {
  try {
    const answer = await call<{ settings?: { xref_with_page?: boolean } }>("work.typeset_get", work !== null ? { work } : {});
    const value = answer.settings?.xref_with_page;
    return typeof value === "boolean" ? value : null;
  } catch {
    return null;
  }
}

function readPage(): boolean {
  try {
    return localStorage.getItem(PAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function writePage(on: boolean): void {
  try {
    localStorage.setItem(PAGE_KEY, on ? "1" : "0");
  } catch {
    // Without storage the choice applies only this once.
  }
}

function workOf(element: Element | null): number | null {
  const root = element?.closest<HTMLElement>("[data-work]");
  const n = Number(root?.dataset.work ?? new URLSearchParams(window.location.search).get("work") ?? NaN);
  return Number.isFinite(n) ? n : null;
}

/// Jump to the outline item `key` (with jump history: through a link `data-jump` that jump.ts handles).
function jumpTo(work: number | null, key: number): void {
  const link = document.createElement("a");
  link.href = `/?${work !== null ? `work=${work}&` : ""}key=${key}`;
  link.dataset.jump = "";
  link.hidden = true;
  document.body.append(link);
  link.click();
  link.remove();
}

interface Choice {
  key: number;
  text: string;
  page: boolean;
}

/// Catalog text inside markup that is built as a string (text and attribute values).
function esc(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/// Open the dialog; `initial` = values of an existing reference (Bearbeiten).
function openDialog(work: number | null, initial: Partial<Choice>, done: (choice: Choice) => void): void {
  document.querySelector("dialog[data-xref-dialog]")?.remove();
  const dialog = document.createElement("dialog");
  dialog.className = "xref-dialog";
  dialog.dataset.xrefDialog = "";
  dialog.setAttribute("aria-label", t("widget.xref.dialog_label"));
  dialog.innerHTML =
    `<form method="dialog" class="xref-form"><h2>${esc(t("widget.xref.heading"))}</h2>` +
    `<label>${esc(t("widget.xref.find_target"))} <input type="search" data-xref-q placeholder="${esc(t("widget.xref.find_placeholder"))}" autocomplete="off"></label>` +
    `<ul class="xref-targets" data-xref-targets role="listbox" aria-label="${esc(t("widget.xref.targets_label"))}"></ul>` +
    `<label>${esc(t("widget.xref.link_text"))} <input type="text" data-xref-text maxlength="300"></label>` +
    `<label><input type="checkbox" data-xref-page> ${esc(t("widget.xref.with_page"))}</label>` +
    '<p class="xref-preview small" data-xref-preview></p>' +
    `<p><button type="submit" class="btn primary" data-xref-ok value="ok">${esc(t("widget.xref.insert"))}</button> ` +
    `<button type="submit" class="btn" value="cancel">${esc(t("widget.xref.cancel"))}</button></p></form>`;
  document.body.append(dialog);
  const q = dialog.querySelector<HTMLInputElement>("[data-xref-q]");
  const list = dialog.querySelector<HTMLElement>("[data-xref-targets]");
  const text = dialog.querySelector<HTMLInputElement>("[data-xref-text]");
  const page = dialog.querySelector<HTMLInputElement>("[data-xref-page]");
  const preview = dialog.querySelector<HTMLElement>("[data-xref-preview]");
  if (!q || !list || !text || !page || !preview) return;
  page.checked = initialPage(initial.page, null, readPage());
  // The work setting follows by RPC; as long as nobody has touched the checkbox, it applies.
  let touched = false;
  let fromWork = false;
  if (initial.page === undefined) {
    void workSetting(work).then((setting) => {
      if (setting === null || touched) return;
      fromWork = true;
      page.checked = initialPage(undefined, setting, readPage());
      refresh();
    });
  }
  let chosen: number | null = initial.key ?? null;
  if (initial.text) text.value = initial.text;
  const refresh = () => {
    preview.textContent = chosen === null ? t("widget.xref.choose_target") : snippet(chosen, escapeText(text.value || "…"), page.checked);
  };
  const load = async () => {
    try {
      const result = await call<{ targets: Target[] }>("xref.targets", { work, q: q.value, limit: 50 });
      list.replaceChildren();
      for (const t of result.targets) {
        const li = document.createElement("li");
        const button = document.createElement("button");
        button.type = "button";
        button.className = "btn xref-target";
        button.style.paddingLeft = `${0.4 + t.level * 0.8}em`;
        button.textContent = `${t.number} ${t.title}`.trim();
        button.setAttribute("role", "option");
        button.setAttribute("aria-selected", String(t.key === chosen));
        button.addEventListener("click", () => {
          chosen = t.key;
          if (!text.value) text.value = t.title;
          list.querySelectorAll("[aria-selected]").forEach((b) => b.setAttribute("aria-selected", "false"));
          button.setAttribute("aria-selected", "true");
          refresh();
        });
        li.append(button);
        list.append(li);
      }
    } catch (error) {
      preview.textContent = String((error as Error).message ?? error);
    }
  };
  let timer: number | undefined;
  q.addEventListener("input", () => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(() => void load(), 150);
  });
  text.addEventListener("input", refresh);
  page.addEventListener("change", () => {
    touched = true;
    // With a work setting the change applies only to this reference; without one the browser remembers it as before.
    if (!fromWork) writePage(page.checked);
    refresh();
  });
  dialog.addEventListener("close", () => {
    if (dialog.returnValue === "ok" && chosen !== null && text.value.trim()) {
      done({ key: chosen, text: text.value.trim(), page: page.checked });
    }
    dialog.remove();
  });
  refresh();
  void load();
  dialog.showModal();
  q.focus();
}

/// Dialog for a new reference at the cursor in the editor `root` (ribbon).
export function insertXref(root: HTMLElement): void {
  const work = workOf(root);
  const area = root.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  // Remember the cursor position in the source text before the dialog takes the focus.
  const start = area?.selectionStart ?? 0;
  const end = area?.selectionEnd ?? start;
  const selected = area && !area.hidden ? area.value.slice(start, end) : (window.getSelection()?.toString() ?? "");
  openDialog(work, { text: selected.trim() || undefined }, (choice) => {
    if (root.hasAttribute("data-ws-open")) {
      if (sendWsCommand(root, "xref", JSON.stringify(choice))) return;
    }
    if (!area || area.hidden) return;
    const typst = snippet(choice.key, escapeText(choice.text), choice.page);
    area.focus();
    area.setRangeText(typst, start, end, "end");
    area.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

if (typeof document !== "undefined") {
  // Ribbon (ribbon.ts): `baruch:xref-open` on the editor area.
  document.addEventListener("baruch:xref-open", (event) => {
    const root = event.target instanceof HTMLElement ? event.target : null;
    if (root) insertXref(root);
  });
  document.addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;
    // Typeset view: Ctrl+click jumps; a simple click shows „Öffnen | Bearbeiten“.
    const block = target.closest<HTMLElement>("[data-ws-sheet] .ws-xref");
    if (block) {
      const key = Number(block.dataset.key);
      if (!Number.isFinite(key)) return;
      if (event.ctrlKey || event.metaKey) {
        event.preventDefault();
        jumpTo(workOf(block), key);
        return;
      }
      showMenu(block, key);
      return;
    }
    // Source text: Ctrl+click on <kN> or #link(<kN>)[…].
    const area = target.closest<HTMLTextAreaElement>("div.wysiwyg textarea[name=body]");
    if (area && (event.ctrlKey || event.metaKey)) {
      const key = keyAt(area.value, area.selectionStart ?? 0);
      if (key !== null) {
        event.preventDefault();
        jumpTo(workOf(area), key);
      }
    }
    if (!target.closest(".xref-menu")) document.querySelector(".xref-menu")?.remove();
  });
}

/// Small menu „Öffnen | Bearbeiten“ at the reference block.
function showMenu(block: HTMLElement, key: number): void {
  document.querySelector(".xref-menu")?.remove();
  const menu = document.createElement("span");
  menu.className = "xref-menu";
  menu.contentEditable = "false";
  const open = document.createElement("button");
  open.type = "button";
  open.className = "btn small";
  open.textContent = t("widget.xref.open");
  open.title = t("widget.xref.open_title");
  const edit = document.createElement("button");
  edit.type = "button";
  edit.className = "btn small";
  edit.textContent = t("widget.xref.edit");
  open.addEventListener("click", () => {
    menu.remove();
    jumpTo(workOf(block), key);
  });
  edit.addEventListener("click", () => {
    menu.remove();
    const current = Array.from(block.childNodes)
      .filter((n) => !(n instanceof HTMLElement && n.classList.contains("ws-xref-page")))
      .map((n) => n.textContent ?? "")
      .join("");
    openDialog(workOf(block), { key, text: current, page: block.dataset.page === "1" }, (choice) => {
      block.dataset.key = String(choice.key);
      block.dataset.page = choice.page ? "1" : "0";
      block.title = t("widget.xref.xref_title", { key: `k${choice.key}` });
      block.replaceChildren(document.createTextNode(choice.text));
      if (choice.page) {
        const suffix = document.createElement("span");
        suffix.className = "ws-xref-page";
        suffix.contentEditable = "false";
        suffix.textContent = ", S. …";
        block.append(suffix);
      }
      // The typeset view sends the block anew on saving (data-dirty on the topmost block).
      const top = block.closest<HTMLElement>(".ws-block");
      if (top) top.dataset.dirty = "1";
      block.dispatchEvent(new Event("input", { bubbles: true }));
    });
  });
  menu.append(open, " | ", edit);
  // Not into the typeset view (its text would be saved along): fixed above the block in the document.
  const rect = block.getBoundingClientRect();
  menu.style.position = "fixed";
  menu.style.left = `${Math.round(rect.left)}px`;
  menu.style.top = `${Math.round(rect.bottom + 4)}px`;
  document.body.append(menu);
}
