// SPDX-License-Identifier: AGPL-3.0-or-later
// Works with JavaScript. Without JavaScript everything goes through the GET form of the work selection (button
// „Öffnen“ in <noscript>), the pages /werke and /werke/neu and the forms to POST /werke/import
// or /werke/act.
//
// - Widget „work-select“ (choice in the header): a change opens the chosen work at once
//   (`location.assign`) and remembers it in the cookie `baruch_work`; the server sets the cookie anyway
//   with every answer of the page `/`.
// - Widget „work-import“ (<dialog> in the header): the link „Importieren…“ (/werke/neu) opens
//   the dialog with the same forms.
// - Widget „work-upload“ (upload form): prefills the title field from `#set document(title: …)` (Markdown: `title:`
//   in the YAML header, Round 21b; ZIP: nothing) of the
//   chosen file. The server stays authoritative; an empty field means "title from the file".

import { leaveEditor, register, store } from "./widgets.js";

/// Title from the first `#set document(… title: "…" | […] …)` outside comments; otherwise `null`.
/// Roughly like `baruch_typst::parse` (raw blocks are not considered); only a suggestion.
export function documentTitle(source: string): string | null {
  const code = source
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/(^|[^:])\/\/[^\n]*/g, "$1");
  const call = /#set\s+document\s*\(/.exec(code);
  if (!call) return null;
  const rest = code.slice(call.index + call[0].length);
  const quoted = /^[^)]*?\btitle\s*:\s*"((?:[^"\\]|\\.)*)"/.exec(rest);
  if (quoted) return quoted[1].replace(/\\(.)/g, "$1").trim() || null;
  const content = /^[^)]*?\btitle\s*:\s*\[([^\]]*)\]/.exec(rest);
  if (content) return content[1].trim() || null;
  return null;
}

/// Title from the YAML header of a Markdown file (`title: …` between two `---` lines at the very beginning);
/// otherwise `null`. Only a suggestion for the title field. Round 23a: in step with `markdown_import::convert`
/// (`front_matter`, proven by a test that sets both on the same files): lines that begin with whitespace or
/// `-` do not count; the key may have whitespace before the colon; **one** pair of quotation marks
/// on the outside drops away (double, otherwise single); an empty value does not count; if several `title:` stand in
/// the header, the last one with a value applies.
export function markdownTitle(source: string): string | null {
  const head = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/.exec(source);
  if (!head) return null;
  let title: string | null = null;
  for (const line of head[1].split(/\r?\n/)) {
    if (/^[ \t-]/.test(line)) continue;
    const colon = line.indexOf(":");
    if (colon < 0 || line.slice(0, colon).trim() !== "title") continue;
    let value = line.slice(colon + 1).trim();
    if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) value = value.slice(1, -1);
    else if (value.length >= 2 && value.startsWith("'") && value.endsWith("'")) value = value.slice(1, -1);
    value = value.trim();
    if (value !== "") title = value;
  }
  return title;
}

register("work-select", (element) => {
  if (!(element instanceof HTMLSelectElement)) return;
  const select = element;
  let shown = select.value;
  select.addEventListener("change", () => {
    const id = select.value;
    if (!/^\d+$/.test(id) || id === shown) return;
    if (!leaveEditor()) {
      select.value = shown;
      return;
    }
    shown = id;
    store.set("work", id);
    window.location.assign(`/?work=${id}`);
  });
});

register("work-import", (element) => {
  if (!(element instanceof HTMLDialogElement)) return;
  const dialog = element;
  document.querySelectorAll<HTMLAnchorElement>("a[data-work-import-open]").forEach((link) => {
    link.addEventListener("click", (event) => {
      if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      dialog.showModal();
      dialog.querySelector<HTMLInputElement>("input[type=file]")?.focus();
    });
  });
});

register("work-upload", (form) => {
  const file = form.querySelector<HTMLInputElement>("input[type=file][name=file]");
  const title = form.querySelector<HTMLInputElement>("input[name=title][data-from-file]");
  if (!file || !title) return;
  // Titles typed in by hand stay; only an empty field or one prefilled from here is filled.
  title.addEventListener("input", () => {
    delete title.dataset.filled;
  });
  file.addEventListener("change", async () => {
    const chosen = file.files?.[0];
    if (!chosen || (title.value !== "" && title.dataset.filled !== "1")) return;
    // The browser does not read a ZIP: the server takes the title from the main file or the archive name.
    if (/\.zip$/i.test(chosen.name)) return;
    try {
      const head = await chosen.slice(0, 256 * 1024).text();
      const found = /\.(md|markdown)$/i.test(chosen.name) ? markdownTitle(head) : documentTitle(head);
      title.value = found ?? "";
      title.dataset.filled = "1";
      title.placeholder = found === null ? "aus dem Dateinamen" : "";
    } catch {
      // Reading in the browser failed: the server takes the title from the file.
    }
  });
});
