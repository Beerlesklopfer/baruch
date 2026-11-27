// SPDX-License-Identifier: AGPL-3.0-or-later
// Context menu in the typeset view and in the source text field (Round 31c, ownership kontextmenue).
//
// Widget „context-menu“ on the group „Register und Glossar“ of the ribbon (`span.ribbon-index` in
// templates/fragments/edit_toolbar.html, inside `div.wysiwyg`). It attaches to the typeset view
// (`[data-ws-sheet]`) and the text field (`textarea[name=body]`) of the same editor:
// - Opening: right click, context menu key or Shift+F10 in the typeset view or text field, also the link
//   „Markierung …“ of the status line (`[data-ctx-open]`, without JavaScript a link to /werke/{id}/markierung).
// - Entries: „In Glossar aufnehmen“ (dialog, term prefilled → RPC glossary.add), „In Register aufnehmen“
//   (dialog with keyword, sub-keyword, index, see, see also → mark at the selection: in the typeset view through the
//   command `index_mark` to wysiwyg.ts, in the source text as `markSource` behind the word), „Im Glossar nachschlagen“
//   (RPC glossary.lookup, result as a popup); on a mark additionally „Registereintrag bearbeiten“ and „… entfernen“.
//   Saving happens only with „Speichern“ as a new revision (the editor is unsaved afterwards).
// - ARIA menu pattern: `role="menu"` with `role="menuitem"`, arrow up/down (wrapping), Home/End, Enter/Space
//   triggers, Escape or Tab closes, the focus goes back to the place the menu came from (`menuKey`).
// - Ribbon: the same three actions as buttons (`data-ctx-action`) and „Register“ (button `[data-index-toggle]` with
//   aria-expanded) with all index entries of the section – from the marks of the typeset view or of the text field
// (also unsaved). No import of wysiwyg.ts: commands to the typeset view only through `baruch:ws-command`
// (ws_command.ts).

import { call } from "./rpc.js";
import { register } from "./widgets.js";
import {
  BUILTIN_REGISTERS,
  cleanMark,
  findMarks,
  type IndexMark,
  markInSource,
  markedWord,
  markSource,
  registerTitle,
  sendWsCommand,
  validateMark,
} from "./ws_command.js";

/// Actions of the menu.
export type MenuAction = "glossary_add" | "index_add" | "glossary_lookup" | "index_edit" | "index_remove";

export interface MenuItem {
  action: MenuAction;
  label: string;
}

/// Entries of the context menu; on a mark „bearbeiten“ and „entfernen“ are added.
export function menuItems(onMark: boolean): MenuItem[] {
  const items: MenuItem[] = [
    { action: "glossary_add", label: "In Glossar aufnehmen …" },
    { action: "index_add", label: "In Register aufnehmen …" },
    { action: "glossary_lookup", label: "Im Glossar nachschlagen" },
  ];
  if (onMark) {
    items.push(
      { action: "index_edit", label: "Registereintrag bearbeiten …" },
      { action: "index_remove", label: "Registereintrag entfernen" },
    );
  }
  return items;
}

/// Does this key open the context menu? Context menu key or Shift+F10 (without Ctrl/Alt/Meta).
export function opensMenu(event: { key: string; shiftKey: boolean; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean }): boolean {
  if (event.ctrlKey || event.altKey || event.metaKey) return false;
  return event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
}

/// Keyboard in the open menu (ARIA menu pattern): new index, closing or triggering; `null` = the key does not belong
/// to the menu.
export function menuKey(
  key: string,
  index: number,
  count: number,
): { index: number } | { close: true } | { activate: true } | null {
  if (count <= 0) return key === "Escape" || key === "Tab" ? { close: true } : null;
  switch (key) {
    case "ArrowDown":
      return { index: (index + 1) % count };
    case "ArrowUp":
      return { index: (index - 1 + count) % count };
    case "Home":
      return { index: 0 };
    case "End":
      return { index: count - 1 };
    case "Escape":
    case "Tab":
      return { close: true };
    case "Enter":
    case " ":
      return { activate: true };
    default:
      return null;
  }
}

/// Mark from the fields of the dialog „In Register aufnehmen“ (trimmed; empty additions are missing) or the message
/// of `validateMark`.
export function markFromFields(fields: Record<string, string>): { mark: IndexMark } | { error: string } {
  const get = (name: string) => (fields[name] ?? "").trim();
  const mark = cleanMark({
    register: get("register") || "stichwort",
    term: get("term"),
    sub: get("sub"),
    see: get("see"),
    also: get("also"),
  });
  const problem = validateMark(mark);
  return problem ? { error: problem } : { mark };
}

/// Fields of the dialog „In Glossar aufnehmen“ → parameters of glossary.add (variants and Bible passages per line).
export function glossaryParams(fields: Record<string, string>): Record<string, unknown> | { error: string } {
  const get = (name: string) => (fields[name] ?? "").trim();
  const lines = (name: string) =>
    (fields[name] ?? "")
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line !== "");
  if (get("term") === "") return { error: "Begriff: darf nicht leer sein." };
  return {
    term: get("term"),
    short: get("short"),
    long: get("long"),
    variants: lines("variants"),
    bible_refs: lines("bible_refs"),
    source: get("source"),
  };
}

/// Mark from the data fields of a `span.ws-index-mark` (as baruch-wysiwyg writes them).
export function markFromData(data: Record<string, string | undefined>): IndexMark {
  return cleanMark({
    register: data.register ?? "",
    term: data.term ?? "",
    sub: data.sub,
    see: data.see,
    also: data.also,
    sort: data.sort,
  });
}

/// Line of the list „Register“: „Stichwortregister: Gnade › im Alltag (siehe auch: Huld)“.
export function entryLine(mark: IndexMark, titles: Record<string, string> = {}): string {
  let line = `${registerTitle(mark.register, titles)}: ${mark.term}`;
  if (mark.sub) line += ` › ${mark.sub}`;
  if (mark.see) line += ` (siehe: ${mark.see})`;
  if (mark.also) line += ` (siehe auch: ${mark.also})`;
  return line;
}

/// Address of a term on the glossary page.
export function termUrl(work: number, id: number): string {
  return `/werke/${work}/glossar#begriff-${id}`;
}

// ---- Operation in the browser ----------------------------------------------------------------------------------

interface Term {
  id: number;
  term: string;
  short: string;
  long: string;
  variants: string[];
  bible_refs: string[];
}

interface Register {
  slug: string;
  title: string;
}

/// What the menu hangs on: typeset view (with selection or mark) or text field (with selection or mark in the text).
type Target =
  | { kind: "sheet"; word: string; mark: HTMLElement | null }
  | { kind: "area"; word: string; start: number; end: number; mark: { start: number; end: number; mark: IndexMark } | null };

function toast(message: string, level: "info" | "warn" = "info"): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level } }));
}

function element<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, text?: string) {
  const el = document.createElement(tag);
  Object.assign(el, props);
  if (text !== undefined) el.textContent = text;
  return el;
}

/// A field of the dialog with a label.
function field(label: string, control: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement): HTMLLabelElement {
  const wrap = element("label", { className: "ctx-field small" });
  wrap.append(element("span", {}, label), control);
  return wrap;
}

if (typeof document !== "undefined") {
  register("context-menu", (group) => {
    const root = group.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
    if (!root) return;
    const sheet = root.querySelector<HTMLElement>("[data-ws-sheet]");
    const area = root.querySelector<HTMLTextAreaElement>("textarea[name=body]");
    if (!sheet || !area) return;
    const work = Number(root.dataset.work);
    let registers: Register[] | null = null;
    let lastRange: Range | null = null;
    let returnFocus: HTMLElement | null = null;
    let areaSelection: [number, number] = [0, 0];

    const sheetOpen = () => root.hasAttribute("data-ws-open");
    document.addEventListener("selectionchange", () => {
      const selection = document.getSelection();
      if (!selection || selection.rangeCount === 0 || !root.isConnected) return;
      const range = selection.getRangeAt(0);
      const at = range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement;
      if (at && sheet.contains(at) && !at.closest(".pv")) lastRange = range.cloneRange();
    });

    // ---- Determine the target ----
    const markBeside = (range: Range): HTMLElement | null => {
      const isMark = (n: Node | null | undefined): n is HTMLElement =>
        n instanceof HTMLElement && n.classList.contains("ws-index-mark");
      const node = range.startContainer;
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node as Text;
        if (range.startOffset === text.data.length && isMark(text.nextSibling)) return text.nextSibling;
        if (range.startOffset === 0 && isMark(text.previousSibling)) return text.previousSibling;
        return null;
      }
      const before = node.childNodes[range.startOffset - 1];
      const after = node.childNodes[range.startOffset];
      return isMark(before) ? before : isMark(after) ? after : null;
    };
    const target = (clicked: Element | null): Target => {
      if (sheetOpen()) {
        const onMark = clicked?.closest<HTMLElement>(".ws-index-mark") ?? null;
        const selection = document.getSelection();
        let range = selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
        const inside = (r: Range | null) => {
          const at = r ? (r.startContainer instanceof Element ? r.startContainer : r.startContainer.parentElement) : null;
          return !!at && sheet.contains(at);
        };
        if (!inside(range)) range = lastRange;
        let word = range ? range.toString().trim() : "";
        if (range && word === "" && range.startContainer.nodeType === Node.TEXT_NODE) {
          const text = range.startContainer as Text;
          word = markedWord(text.data, range.startOffset, range.startOffset).word;
        }
        return { kind: "sheet", word, mark: onMark ?? (range ? markBeside(range) : null) };
      }
      const start = area.selectionStart ?? 0;
      const end = area.selectionEnd ?? start;
      areaSelection = [start, end];
      const mark = findMarks(area.value).find((m) => m.start <= start && start <= m.end) ?? null;
      return { kind: "area", word: markedWord(area.value, start, end).word, start, end, mark };
    };

    // ---- Menu ----
    const menu = element("div", { className: "ctx-menu", hidden: true });
    menu.setAttribute("role", "menu");
    menu.setAttribute("aria-label", "Register und Glossar");
    root.append(menu);
    const items = () => Array.from(menu.querySelectorAll<HTMLButtonElement>("[role=menuitem]"));
    const focusItem = (index: number) => {
      items().forEach((item, k) => (item.tabIndex = k === index ? 0 : -1));
      items()[index]?.focus();
    };
    const closeMenu = (restore: boolean) => {
      if (menu.hidden) return;
      menu.hidden = true;
      menu.replaceChildren();
      if (restore) restoreFocus();
    };
    const restoreFocus = () => {
      const back = returnFocus;
      if (!back || !back.isConnected) return;
      back.focus();
      if (back === area) area.setSelectionRange(areaSelection[0], areaSelection[1]);
      else if (back === sheet && lastRange) {
        // If the cursor is already in the typeset view (e.g. behind a new mark), it stays there.
        const selection = document.getSelection();
        const anchor = selection && selection.rangeCount > 0 ? selection.getRangeAt(0).startContainer : null;
        if (!anchor || !sheet.contains(anchor)) {
          selection?.removeAllRanges();
          selection?.addRange(lastRange);
        }
      }
    };
    const openMenu = (t: Target, x: number, y: number) => {
      menu.replaceChildren();
      for (const item of menuItems(t.mark !== null)) {
        const button = element("button", { type: "button", className: "ctx-item", tabIndex: -1 }, item.label);
        button.setAttribute("role", "menuitem");
        button.dataset.ctxAction = item.action;
        button.addEventListener("click", () => {
          closeMenu(false);
          void run(item.action, t);
        });
        menu.append(button);
      }
      menu.hidden = false;
      const width = menu.offsetWidth || 240;
      const height = menu.offsetHeight || 160;
      menu.style.left = `${Math.max(4, Math.min(x, window.innerWidth - width - 4))}px`;
      menu.style.top = `${Math.max(4, Math.min(y, window.innerHeight - height - 4))}px`;
      focusItem(0);
    };
    menu.addEventListener("keydown", (event) => {
      const list = items();
      const index = list.indexOf(document.activeElement as HTMLButtonElement);
      const step = menuKey(event.key, Math.max(0, index), list.length);
      if (!step) return;
      event.preventDefault();
      if ("index" in step) focusItem(step.index);
      else if ("close" in step) closeMenu(true);
      else list[Math.max(0, index)]?.click();
    });
    menu.addEventListener("focusout", (event) => {
      const next = event.relatedTarget instanceof Node ? event.relatedTarget : null;
      if (!next || !menu.contains(next)) window.setTimeout(() => closeMenu(false), 0);
    });
    document.addEventListener("mousedown", (event) => {
      if (!menu.hidden && !(event.target instanceof Node && menu.contains(event.target))) closeMenu(false);
    });

    const caretPoint = (from: HTMLElement): [number, number] => {
      if (from === sheet && lastRange) {
        const rect = lastRange.getBoundingClientRect();
        if (rect.width || rect.height) return [rect.left, rect.bottom + 4];
      }
      const rect = from.getBoundingClientRect();
      return [rect.left + 16, rect.top + 24];
    };
    // Browsers often send `contextmenu` additionally after the context menu key or Shift+F10: then do not open twice.
    let keyOpened = 0;
    const onContext = (event: MouseEvent, from: HTMLElement) => {
      if (Date.now() - keyOpened < 500) {
        event.preventDefault();
        return;
      }
      returnFocus = from;
      const t = target(event.target instanceof Element ? event.target : null);
      event.preventDefault();
      openMenu(t, event.clientX, event.clientY);
    };
    const onKey = (event: KeyboardEvent, from: HTMLElement) => {
      if (!opensMenu(event)) return;
      event.preventDefault();
      keyOpened = Date.now();
      returnFocus = from;
      const [x, y] = caretPoint(from);
      openMenu(target(null), x, y);
    };
    sheet.addEventListener("contextmenu", (event) => onContext(event, sheet));
    area.addEventListener("contextmenu", (event) => onContext(event, area));
    sheet.addEventListener("keydown", (event) => onKey(event, sheet));
    area.addEventListener("keydown", (event) => onKey(event, area));
    // A double click on a mark opens „Registereintrag bearbeiten“.
    sheet.addEventListener("dblclick", (event) => {
      const mark = event.target instanceof Element ? event.target.closest<HTMLElement>(".ws-index-mark") : null;
      if (!mark || mark.closest(".pv")) return;
      event.preventDefault();
      returnFocus = sheet;
      void run("index_edit", { kind: "sheet", word: "", mark });
    });
    const opener = root.querySelector<HTMLAnchorElement>("[data-ctx-open]");
    opener?.addEventListener("mousedown", (event) => event.preventDefault());
    opener?.addEventListener("click", (event) => {
      event.preventDefault();
      returnFocus = sheetOpen() ? sheet : area;
      const t = target(null);
      const rect = opener.getBoundingClientRect();
      openMenu(t, rect.left, rect.top - 8 - 5 * 32);
    });

    // ---- Ribbon ----
    group.querySelectorAll<HTMLButtonElement>("[data-ctx-action]").forEach((button) => {
      button.addEventListener("mousedown", (event) => event.preventDefault());
      button.addEventListener("click", () => {
        returnFocus = sheetOpen() ? sheet : area;
        void run(button.dataset.ctxAction as MenuAction, target(null));
      });
    });
    // „Register“: button with aria-expanded, below it the list (Escape closes, focus back to the button).
    const list = group.querySelector<HTMLElement>("[data-index-list]");
    const toggle = list?.querySelector<HTMLButtonElement>("[data-index-toggle]");
    const entries = list?.querySelector<HTMLElement>("[data-index-entries]");
    const showList = (open: boolean) => {
      if (!toggle || !entries) return;
      entries.hidden = !open;
      toggle.setAttribute("aria-expanded", String(open));
      if (open) void fillList(entries).then(() => entries.querySelector<HTMLElement>("button")?.focus());
    };
    toggle?.addEventListener("mousedown", (event) => event.preventDefault());
    toggle?.addEventListener("click", () => showList(entries?.hidden ?? false));
    entries?.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        showList(false);
        toggle?.focus();
      }
    });

    // ---- Dialogs ----
    const dialog = (title: string): { box: HTMLDialogElement; form: HTMLFormElement; error: HTMLElement } => {
      const box = element("dialog", { className: "ctx-dialog" });
      box.setAttribute("aria-label", title);
      const form = element("form", { method: "dialog" });
      const error = element("p", { className: "ctx-error small", hidden: true });
      error.setAttribute("role", "alert");
      form.append(element("h3", {}, title), error);
      box.append(form);
      box.addEventListener("close", () => {
        box.remove();
        restoreFocus();
      });
      root.append(box);
      return { box, form, error };
    };
    const buttons = (form: HTMLFormElement, ok: string): HTMLButtonElement => {
      const row = element("div", { className: "ctx-buttons" });
      const submit = element("button", { type: "submit", className: "btn primary" }, ok);
      const cancel = element("button", { type: "button", className: "btn" }, "Abbrechen");
      cancel.addEventListener("click", () => form.closest("dialog")?.close());
      row.append(submit, cancel);
      form.append(row);
      return submit;
    };
    const fieldsOf = (form: HTMLFormElement): Record<string, string> => {
      const out: Record<string, string> = {};
      new FormData(form).forEach((value, key) => {
        if (typeof value === "string") out[key] = value;
      });
      return out;
    };
    const loadRegisters = async (): Promise<Register[]> => {
      if (registers) return registers;
      try {
        const answer = await call<{ registers: Register[] }>("index.registers", { work });
        registers = answer.registers;
      } catch {
        registers = BUILTIN_REGISTERS.map((r) => ({ ...r }));
      }
      return registers;
    };

    /// Dialog of a mark: `check` checks without consequences (the message stays in the dialog), `apply` takes effect
    /// only after closing – as long as the modal dialog is open, typeset view and text field are inert (no focus, no
    /// insertText there).
    const markDialog = async (
      title: string,
      ok: string,
      start: IndexMark,
      check: (mark: IndexMark) => string | null,
      apply: (mark: IndexMark) => void,
    ) => {
      const all = await loadRegisters();
      const { box, form, error } = dialog(title);
      const input = (name: string, value: string, required = false) => {
        const control = element("input", { type: "text", name, value, maxLength: 200, required });
        return control;
      };
      const select = element("select", { name: "register" });
      for (const r of all) select.append(element("option", { value: r.slug, selected: r.slug === start.register }, r.title));
      if (!all.some((r) => r.slug === start.register)) {
        select.append(element("option", { value: start.register, selected: true }, registerTitle(start.register)));
      }
      form.append(
        field("Stichwort", input("term", start.term, true)),
        field("Unterstichwort", input("sub", start.sub ?? "")),
        field("Register", select),
        field("siehe", input("see", start.see ?? "")),
        field("siehe auch", input("also", start.also ?? "")),
      );
      buttons(form, ok);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const read = markFromFields(fieldsOf(form));
        const mark = "error" in read ? null : cleanMark({ ...read.mark, sort: start.sort });
        const problem = mark ? check(mark) : (read as { error: string }).error;
        if (problem || !mark) {
          error.textContent = problem ?? "";
          error.hidden = false;
          return;
        }
        box.close();
        apply(mark);
      });
      box.showModal();
      form.querySelector<HTMLInputElement>("input[name=term]")?.select();
    };

    const replaceInArea = (start: number, end: number, text: string) => {
      area.focus();
      area.setSelectionRange(start, end);
      let done = false;
      try {
        done = document.execCommand("insertText", false, text);
      } catch {
        done = false;
      }
      if (!done) {
        area.setRangeText(text, start, end, "end");
        area.dispatchEvent(new Event("input", { bubbles: true }));
      }
      areaSelection = [start + text.length, start + text.length];
    };

    const run = async (action: MenuAction, t: Target): Promise<void> => {
      switch (action) {
        case "glossary_add": {
          const { box, form, error } = dialog("In Glossar aufnehmen");
          form.append(
            field("Begriff", element("input", { type: "text", name: "term", value: t.word, maxLength: 200, required: true })),
            field("Kurze Erklärung", element("input", { type: "text", name: "short", maxLength: 500 })),
            field("Ausführliche Erklärung", element("textarea", { name: "long", rows: 4 })),
            field("Varianten (je Zeile eine)", element("textarea", { name: "variants", rows: 2 })),
            field("Bibelstellen (je Zeile eine)", element("textarea", { name: "bible_refs", rows: 2 })),
            field("Quelle", element("input", { type: "text", name: "source", maxLength: 500 })),
          );
          const submit = buttons(form, "Aufnehmen");
          form.addEventListener("submit", (event) => {
            event.preventDefault();
            const params = glossaryParams(fieldsOf(form));
            if ("error" in params) {
              error.textContent = String(params.error);
              error.hidden = false;
              return;
            }
            submit.disabled = true;
            call<{ term: Term }>("glossary.add", { work, ...params })
              .then((answer) => {
                box.close();
                toast(`„${answer.term?.term ?? params.term}“ steht jetzt im Glossar.`);
              })
              .catch((e: Error) => {
                error.textContent = e.message;
                error.hidden = false;
                submit.disabled = false;
              });
          });
          box.showModal();
          form.querySelector<HTMLInputElement>("input[name=term]")?.select();
          return;
        }
        case "glossary_lookup": {
          if (t.word === "") {
            toast("Bitte ein Wort markieren oder den Cursor in ein Wort setzen.");
            restoreFocus();
            return;
          }
          const { box, form, error } = dialog(`Im Glossar: ${t.word}`);
          box.classList.add("ctx-lookup");
          const body = element("div", { className: "ctx-lookup-body" });
          body.setAttribute("aria-live", "polite");
          body.textContent = "Suche …";
          form.append(body);
          const close = element("button", { type: "submit", className: "btn" }, "Schließen");
          form.append(close);
          box.showModal();
          close.focus();
          try {
            const answer = await call<{ terms: Term[] }>("glossary.lookup", { work, word: t.word });
            body.replaceChildren();
            if (answer.terms.length === 0) {
              body.append(element("p", { className: "small" }, `„${t.word}“ steht nicht im Glossar.`));
            }
            for (const term of answer.terms) {
              const item = element("article", { className: "ctx-term" });
              const head = element("h4");
              head.append(element("a", { href: termUrl(work, term.id) }, term.term));
              item.append(head);
              if (term.variants.length) item.append(element("p", { className: "muted small" }, `auch: ${term.variants.join(", ")}`));
              if (term.short) item.append(element("p", { className: "small" }, term.short));
              if (term.long) item.append(element("p", { className: "small ctx-long" }, term.long));
              if (term.bible_refs.length) item.append(element("p", { className: "muted small" }, term.bible_refs.join("; ")));
              body.append(item);
            }
          } catch (e) {
            body.replaceChildren();
            error.textContent = (e as Error).message;
            error.hidden = false;
          }
          return;
        }
        case "index_add": {
          const inSource = (mark: IndexMark) => {
            const [start, end] = t.kind === "area" ? [t.start, t.end] : areaSelection;
            return markInSource(area.value, start, end, mark);
          };
          await markDialog(
            "In Register aufnehmen",
            "Marke setzen",
            { register: "stichwort", term: t.word },
            (mark) => {
              if (t.kind === "sheet" && sheetOpen()) return null;
              const put = inSource(mark);
              return "error" in put ? put.error : null;
            },
            (mark) => {
              if (t.kind === "sheet" && sendWsCommand(root, "index_mark", JSON.stringify({ op: "add", mark }))) return;
              const put = inSource(mark);
              if ("error" in put) {
                toast(put.error, "warn");
                return;
              }
              returnFocus = area;
              replaceInArea(put.at, put.at, put.insert);
            },
          );
          return;
        }
        case "index_edit":
        case "index_remove": {
          if (!t.mark) return;
          if (t.kind === "sheet") {
            const markEl = t.mark;
            const send = (op: "edit" | "remove", mark?: IndexMark) => {
              markEl.setAttribute("data-ctx-target", "1");
              const handled = sendWsCommand(root, "index_mark", JSON.stringify({ op, mark }));
              markEl.removeAttribute("data-ctx-target");
              return handled ? null : "Das Satzbild ist nicht offen.";
            };
            if (action === "index_remove") {
              const problem = send("remove");
              if (problem) toast(problem, "warn");
              restoreFocus();
              return;
            }
            await markDialog(
              "Registereintrag bearbeiten",
              "Übernehmen",
              markFromData(markEl.dataset),
              () => (sheetOpen() ? null : "Das Satzbild ist nicht offen."),
              (mark) => {
                const problem = send("edit", mark);
                if (problem) toast(problem, "warn");
              },
            );
            return;
          }
          const found = t.mark;
          if (action === "index_remove") {
            returnFocus = area;
            replaceInArea(found.start, found.end, "");
            return;
          }
          await markDialog(
            "Registereintrag bearbeiten",
            "Übernehmen",
            found.mark,
            () => null,
            (mark) => {
              returnFocus = area;
              replaceInArea(found.start, found.end, markSource(mark));
            },
          );
          return;
        }
      }
    };

    // ---- List „Register“ of the section ----
    const fillList = async (box: HTMLElement) => {
      const all = await loadRegisters();
      const titles = Object.fromEntries(all.map((r) => [r.slug, r.title]));
      box.replaceChildren();
      const add = (mark: IndexMark, jump: () => void) => {
        const button = element("button", { type: "button", className: "ctx-entry small" }, entryLine(mark, titles));
        button.addEventListener("click", jump);
        box.append(button);
      };
      if (sheetOpen()) {
        sheet.querySelectorAll<HTMLElement>(".ws-index-mark").forEach((el) => {
          if (el.closest(".pv")) return;
          add(markFromData(el.dataset), () => {
            el.scrollIntoView({ block: "center" });
            const range = document.createRange();
            range.setStartAfter(el);
            range.collapse(true);
            sheet.focus();
            const selection = document.getSelection();
            selection?.removeAllRanges();
            selection?.addRange(range);
          });
        });
      } else {
        for (const found of findMarks(area.value)) {
          add(found.mark, () => {
            area.focus();
            area.setSelectionRange(found.start, found.end);
          });
        }
      }
      if (!box.hasChildNodes()) box.append(element("p", { className: "muted small" }, "Keine Registereinträge in diesem Abschnitt."));
    };
  });
}
