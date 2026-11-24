// SPDX-License-Identifier: AGPL-3.0-or-later
// Querverweise im Editor (Runde 30a, Vertrag §3.3): Dialog „Verweis“ (Menüband, `cmd = "xref"`), Strg+Klick und
// „Öffnen | Bearbeiten“.
//
// - Dialog: Suche im Baum über RPC `xref.targets {q}`, Linktext, Häkchen „mit Seitenzahl“. Seit Runde 29c startet
//   das Häkchen mit der Werk-Einstellung `xref_with_page` (RPC `work.typeset_get`, Einstellungen → Werk → Satz →
//   Querverweise); nur wenn sie ungesetzt ist, gilt wie bisher die im Browser gemerkte Wahl (localStorage
//   `baruch_xref_page`, nur in try/catch), und nur dann merkt sich der Browser eine Änderung. Einfügen: im offenen Satzbild über
//   `sendWsCommand(root, "xref", JSON {key, text, page})` als Block `span.ws-xref`, sonst als Typst-Text
//   (`#link(<kN>)[…]`, mit Seitenzahl `…, S.~#context counter(page).at(<kN>).first()]`) an der Cursorstelle im
//   Quelltext.
// - Strg+Klick auf einen Verweis im Satzbild (`span.ws-xref`) bzw. auf `<kN>`/`#link(<kN>)` im Quelltext springt zum
//   Ziel (jump.ts-Verlauf über einen gedachten `a[data-jump]`); ein einfacher Klick auf den Block im Satzbild öffnet
//   „Öffnen | Bearbeiten“ (Bearbeiten: Ziel/Text/Seitenzahl im Dialog ändern).

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

/// Typst-Maskierung eines Linktexts (wie baruch_typst::escape_title, ohne Trimmen).
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

/// Verweis-Form wie baruch_typst::xref::xref_snippet (`text` schon Typst).
export function snippet(key: number, text: string, page: boolean): string {
  const label = `k${key}`;
  return page
    ? `#link(<${label}>)[${text}, S.~#context counter(page).at(<${label}>).first()]`
    : `#link(<${label}>)[${text}]`;
}

/// Schlüssel des Verweises bzw. der Sprungmarke an der Stelle `at` im Quelltext (`<k12>` oder `#link(<k12>)[…]`).
export function keyAt(text: string, at: number): number | null {
  const pattern = /#link\(<k(\d+)>\)\[|<k(\d+)>/g;
  for (const m of text.matchAll(pattern)) {
    const start = m.index ?? 0;
    let end = start + m[0].length;
    if (m[0].startsWith("#link")) {
      // Bis zur schließenden Klammer des Linktexts.
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

/// Anfangswert des Häkchens „mit Seitenzahl“: ein vorhandener Verweis (Bearbeiten) behält seine Form, sonst gilt die
/// Werk-Einstellung `xref_with_page`, und nur wenn sie ungesetzt ist (`null`/`undefined`), die Wahl im Browser.
export function initialPage(existing: boolean | undefined, setting: boolean | null | undefined, stored: boolean): boolean {
  if (existing !== undefined) return existing;
  if (typeof setting === "boolean") return setting;
  return stored;
}

/// Werk-Einstellung `xref_with_page` (`null` = ungesetzt oder nicht lesbar).
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
    // Ohne Speicher gilt die Wahl nur für dieses Mal.
  }
}

function workOf(element: Element | null): number | null {
  const root = element?.closest<HTMLElement>("[data-work]");
  const n = Number(root?.dataset.work ?? new URLSearchParams(window.location.search).get("work") ?? NaN);
  return Number.isFinite(n) ? n : null;
}

/// Zum Gliederungspunkt `key` springen (mit Sprungverlauf: über einen Link `data-jump`, den jump.ts behandelt).
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

/// Dialog öffnen; `initial` = Werte eines vorhandenen Verweises (Bearbeiten).
function openDialog(work: number | null, initial: Partial<Choice>, done: (choice: Choice) => void): void {
  document.querySelector("dialog[data-xref-dialog]")?.remove();
  const dialog = document.createElement("dialog");
  dialog.className = "xref-dialog";
  dialog.dataset.xrefDialog = "";
  dialog.setAttribute("aria-label", "Querverweis einfügen");
  dialog.innerHTML =
    '<form method="dialog" class="xref-form"><h2>Querverweis</h2>' +
    '<label>Ziel suchen <input type="search" data-xref-q placeholder="Titel oder Nummer" autocomplete="off"></label>' +
    '<ul class="xref-targets" data-xref-targets role="listbox" aria-label="Gliederungspunkte"></ul>' +
    '<label>Linktext <input type="text" data-xref-text maxlength="300"></label>' +
    '<label><input type="checkbox" data-xref-page> mit Seitenzahl („S. n“)</label>' +
    '<p class="xref-preview small" data-xref-preview></p>' +
    '<p><button type="submit" class="btn primary" data-xref-ok value="ok">Einfügen</button> ' +
    '<button type="submit" class="btn" value="cancel">Abbrechen</button></p></form>';
  document.body.append(dialog);
  const q = dialog.querySelector<HTMLInputElement>("[data-xref-q]");
  const list = dialog.querySelector<HTMLElement>("[data-xref-targets]");
  const text = dialog.querySelector<HTMLInputElement>("[data-xref-text]");
  const page = dialog.querySelector<HTMLInputElement>("[data-xref-page]");
  const preview = dialog.querySelector<HTMLElement>("[data-xref-preview]");
  if (!q || !list || !text || !page || !preview) return;
  page.checked = initialPage(initial.page, null, readPage());
  // Die Werk-Einstellung kommt per RPC nach; solange niemand das Häkchen berührt hat, gilt sie.
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
    preview.textContent = chosen === null ? "Bitte ein Ziel wählen." : snippet(chosen, escapeText(text.value || "…"), page.checked);
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
    // Mit Werk-Einstellung gilt die Änderung nur für diesen Verweis; ohne merkt sie sich der Browser wie bisher.
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

/// Dialog für einen neuen Verweis am Cursor im Editor `root` (Menüband).
export function insertXref(root: HTMLElement): void {
  const work = workOf(root);
  const area = root.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  // Cursorstelle im Quelltext merken, bevor der Dialog den Fokus nimmt.
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
  // Menüband (ribbon.ts): `baruch:xref-open` am Editor-Bereich.
  document.addEventListener("baruch:xref-open", (event) => {
    const root = event.target instanceof HTMLElement ? event.target : null;
    if (root) insertXref(root);
  });
  document.addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;
    // Satzbild: Strg+Klick springt; einfacher Klick zeigt „Öffnen | Bearbeiten“.
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
    // Quelltext: Strg+Klick auf <kN> bzw. #link(<kN>)[…].
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

/// Kleines Menü „Öffnen | Bearbeiten“ am Verweis-Block.
function showMenu(block: HTMLElement, key: number): void {
  document.querySelector(".xref-menu")?.remove();
  const menu = document.createElement("span");
  menu.className = "xref-menu";
  menu.contentEditable = "false";
  const open = document.createElement("button");
  open.type = "button";
  open.className = "btn small";
  open.textContent = "Öffnen";
  open.title = "Zum Ziel springen (Strg+Klick)";
  const edit = document.createElement("button");
  edit.type = "button";
  edit.className = "btn small";
  edit.textContent = "Bearbeiten";
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
      block.title = `Querverweis auf k${choice.key} (Strg+Klick: springen)`;
      block.replaceChildren(document.createTextNode(choice.text));
      if (choice.page) {
        const suffix = document.createElement("span");
        suffix.className = "ws-xref-page";
        suffix.contentEditable = "false";
        suffix.textContent = ", S. …";
        block.append(suffix);
      }
      // Das Satzbild schickt den Block beim Speichern neu (data-dirty am obersten Block).
      const top = block.closest<HTMLElement>(".ws-block");
      if (top) top.dataset.dirty = "1";
      block.dispatchEvent(new Event("input", { bubbles: true }));
    });
  });
  menu.append(open, " | ", edit);
  // Nicht ins Satzbild (sein Text würde mitgespeichert): fest über dem Block im Dokument.
  const rect = block.getBoundingClientRect();
  menu.style.position = "fixed";
  menu.style.left = `${Math.round(rect.left)}px`;
  menu.style.top = `${Math.round(rect.bottom + 4)}px`;
  document.body.append(menu);
}
