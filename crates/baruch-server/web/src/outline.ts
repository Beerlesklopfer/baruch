// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „outline“ (left column): the only outline of the user interface (contract Round 5).
//
// - Drag and drop: upper third of the row = before, middle = below, lower third = after.
//   Moving keeps the level (before/after only next to an item of the same level, below only
//   under an item exactly one level higher); target and level stand below the outline while
//   dragging, invalid targets are rejected.
// - Alt+Shift+←/→ raises or lowers the level, Alt+Shift+↑/↓ moves (everywhere except in
//   input fields).
// - Context menu (right click, Shift+F10, context menu key, button „⋯“): rename (F2), new
//   item before/after/below, new part before/after (Round 14, `level: 0`, only on parts and
//   chapters), raise/lower the level, Löschen… (Del, with confirm() and `confirm: true`). F2, Del and the context menu
//   key only work with the focus in the outline.
// - Renaming in place: double click on the selected entry or F2; Enter confirms
//   (`outline.rename`), Esc cancels. Creating with an input field for the title (`outline.add`).
//   An empty, multi-line or over 200 characters long title is rejected in advance with the message
//   of the server (Round 7); the field stays open.
// - Numbering in the head (`form[data-outline-numbering]`): `vorlage` → null, `keine` → "".
//
// Every writing action asks `leaveEditor()` first and reports `baruch:navigate` with
// `{url, asked: true}` after success (the workspace redraws). Without JavaScript the forms
// per entry (↑ ↓ ← →) and the pages /gliederung… to /gliederung/act do the same.
//
// In addition it asks `outline.check` before saving in the editor (`form[data-outline-save]`) and
// confirms splitting or merging by confirm(); then it sends the form with split/merge = 1.
// Before that it triggers `baruch:before-save` on the form (`detail.wait`: promises it waits for –
// e.g. the typeset view, which writes its text into the text field). The server stays authoritative
// (validity, levels); here things are only rejected in advance.
// Round 22: the triggering button (`submitter`, e.g. „Als neue Fassung speichern“ with `as_new=1`) goes along on
// submitting again (`requestSubmit(submitter)`) so that the workspace knows the mode.

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { editorLocked, leaveEditor, lockEditor, register } from "./widgets.js";

type Place = "before" | "after" | "into";

interface Answer {
  node: number;
  /// Fixed key of the item to show afterwards (Round 8); if it is missing, the position.
  key?: number | null;
}

interface HeadingCheck {
  title: string | null;
  level: number | null;
  split: string[];
  merge: boolean;
  valid: boolean;
  message: string | null;
}

/// Catalog keys of the level names (part, chapter, section, subsection).
const levelNames = [
  "widget.outline.level_part",
  "widget.outline.level_chapter",
  "widget.outline.level_section",
  "widget.outline.level_subsection",
];
/// Drop hint per place: „vor/hinter/unter „Titel“ · Ebene n (Name)“.
const judgeKeys: Record<Place, string> = {
  before: "widget.outline.judge_before",
  after: "widget.outline.judge_after",
  into: "widget.outline.judge_into",
};
/// Label of the title field of a new item: [part, item] per place.
const addKeys: Record<Place, [string, string]> = {
  before: ["widget.outline.add_part_before", "widget.outline.add_item_before"],
  after: ["widget.outline.add_part_after", "widget.outline.add_item_after"],
  into: ["widget.outline.add_part_into", "widget.outline.add_item_into"],
};

/// Title rules like `baruch_store::check_title` (Round 7; the server stays authoritative): message or `null`.
function titleProblem(title: string): string | null {
  const trimmed = title.trim();
  if (trimmed === "") return t("widget.outline.title_empty");
  if (/[\n\r\v\f\u0085\u2028\u2029]/.test(trimmed)) return t("widget.outline.title_newline");
  const count = [...trimmed].length;
  if (count > 200) return t("widget.outline.title_long", { max: 200, count });
  return null;
}

/// Focus the selected entry after the next mount (the keyboard stays in the outline).
let focusAfterRender = false;

function notify(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast";
  toast.setAttribute("role", "alert");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 5000);
}

/// Have it redrawn; the caller has already asked (unsaved text).
function navigate(url: string, asked: boolean): void {
  document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url, asked } }));
}

function nodeOf(item: HTMLElement): number {
  return Number(item.dataset.outlineNode);
}

/// Fixed key (`data-key`): the position can become outdated when another tab rearranges.
function keyOf(item: HTMLElement): number {
  return Number(item.dataset.key);
}

function levelOf(item: HTMLElement): number {
  return Number(item.dataset.level);
}

/// Outline change through RPC, then have it redrawn. Unsaved text in the editor would be lost
/// on redrawing: ask first, change nothing on „Abbrechen“.
async function act(work: number, method: string, params: Record<string, unknown>): Promise<boolean> {
  if (!leaveEditor()) return false;
  try {
    const result = await call<Answer>(method, { work, ...params });
    focusAfterRender = true;
    // Key form (Round 8); only without a key in the answer the position as a fallback.
    const place = typeof result.key === "number" ? `key=${result.key}` : `node=${result.node}`;
    navigate(`/?work=${work}&${place}`, true);
    return true;
  } catch (error) {
    notify(String((error as Error).message ?? error));
    return false;
  }
}

interface Verdict {
  ok: boolean;
  text: string;
}

/// Check the target like the server: the level stays (before/after only next to the same level, below only
/// under one level higher), not into the own subtree.
function judge(dragged: HTMLElement, target: HTMLElement, place: Place): Verdict {
  if (dragged === target || dragged.contains(target)) {
    return { ok: false, text: t("widget.outline.own_subtree") };
  }
  const level = levelOf(dragged);
  const targetLevel = levelOf(target);
  const keeps = place === "into" ? targetLevel + 1 === level : targetLevel === level;
  if (!keeps) return { ok: false, text: t("widget.outline.keeps_level") };
  const title = target.dataset.title ?? "";
  const name = levelNames[level];
  return { ok: true, text: t(judgeKeys[place], { title, level, name: name ? t(name) : "" }) };
}

const marks = ["drop-before", "drop-after", "drop-into", "drop-invalid"];

/// The entry that keys refer to: the one with the focus, otherwise the selected one.
function focusedItem(root: HTMLElement): HTMLElement | null {
  const active = document.activeElement as HTMLElement | null;
  const fromFocus = active && root.contains(active) ? active.closest<HTMLElement>("li[data-outline-node]") : null;
  return fromFocus ?? root.querySelector(".tree-entry.selected")?.closest<HTMLElement>("li[data-outline-node]") ?? null;
}

function entryOf(item: HTMLElement): HTMLAnchorElement | null {
  return item.querySelector<HTMLAnchorElement>(":scope > details > summary .tree-entry, :scope > .leaf .tree-entry");
}

function rowOf(item: HTMLElement): HTMLElement | null {
  return item.querySelector<HTMLElement>(":scope > details > summary .outline-row, :scope > .leaf .outline-row");
}

function typing(element: Element | null): boolean {
  const el = element as HTMLElement | null;
  return !!el && (el.isContentEditable || el.matches("textarea, input, select"));
}

/// Input field that does not pass clicks and keys on to summary, link or workspace.
function field(value: string, placeholder: string, className: string): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "text";
  input.value = value;
  input.placeholder = placeholder;
  input.className = className;
  input.setAttribute("aria-label", placeholder);
  for (const type of ["click", "mousedown", "dblclick"]) {
    input.addEventListener(type, (event) => {
      event.stopPropagation();
      if (type === "click") event.preventDefault();
    });
  }
  return input;
}

/// Renaming in place: Enter → outline.rename, Esc or leaving → cancel.
function startRename(root: HTMLElement, item: HTMLElement): void {
  const entry = entryOf(item);
  const row = rowOf(item);
  if (!entry || !row || row.querySelector(".outline-rename")) return;
  const title = item.dataset.title ?? "";
  const input = field(title, t("widget.outline.new_title"), "outline-rename");
  entry.hidden = true;
  row.draggable = false;
  entry.after(input);
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    input.remove();
    entry.hidden = false;
    row.draggable = true;
  };
  input.addEventListener("keydown", (event) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      finish();
      entry.focus();
    } else if (event.key === "Enter") {
      event.preventDefault();
      const next = input.value.trim();
      if (next === title) {
        finish();
        entry.focus();
        return;
      }
      const problem = titleProblem(input.value);
      if (problem) {
        // The field stays open; the message is the same as that of the server.
        notify(problem);
        return;
      }
      done = true;
      void act(Number(root.dataset.work), "outline.rename", { node: nodeOf(item), key: keyOf(item), title: next }).then(
        (ok) => {
          if (!ok) {
            done = false;
            finish();
          }
        },
      );
    }
  });
  input.addEventListener("blur", () => finish());
  input.focus();
  input.select();
}

/// New item with an input field for the title, before/after/below: Enter → outline.add. With
/// `part` a new part (`level: 0`, Round 14) before or after.
function startAdd(root: HTMLElement, item: HTMLElement, place: Place, part = false): void {
  root.querySelector(".outline-new")?.remove();
  const title = item.dataset.title ?? "";
  const holder = document.createElement("li");
  holder.className = "outline-new";
  const input = field("", t(addKeys[place][part ? 0 : 1], { title }), "");
  holder.append(input);
  if (place === "before") item.before(holder);
  else if (place === "after") item.after(holder);
  else {
    const list = item.querySelector<HTMLElement>(":scope > details > ul");
    if (list) list.append(holder);
    else item.after(holder);
  }
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    holder.remove();
  };
  input.addEventListener("keydown", (event) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      finish();
      entryOf(item)?.focus();
    } else if (event.key === "Enter") {
      event.preventDefault();
      const name = input.value.trim();
      const problem = titleProblem(input.value);
      if (problem) {
        notify(problem);
        return;
      }
      done = true;
      const params: Record<string, unknown> = { target: nodeOf(item), target_key: keyOf(item), place, title: name };
      if (part) params.level = 0;
      void act(Number(root.dataset.work), "outline.add", params).then((ok) => {
        if (!ok) {
          done = false;
          finish();
        }
      });
    }
  });
  input.addEventListener("blur", () => finish());
  input.focus();
}

/// Deleting with a confirmation; the server demands `confirm: true`.
function askDelete(root: HTMLElement, item: HTMLElement): void {
  if (item.dataset.children !== undefined) {
    notify(t("widget.outline.has_children"));
    return;
  }
  const title = item.dataset.title ?? "";
  if (!confirm(t("widget.outline.delete_question", { title }))) return;
  void act(Number(root.dataset.work), "outline.delete", { node: nodeOf(item), key: keyOf(item), confirm: true });
}

function setLevel(root: HTMLElement, item: HTMLElement, delta: number): void {
  const level = levelOf(item);
  if (delta < 0 && item.dataset.promote === undefined) return;
  if (delta > 0 && item.dataset.demote === undefined) return;
  void act(Number(root.dataset.work), "outline.set_level", { node: nodeOf(item), key: keyOf(item), level: level + delta });
}

// ---- Context menu ---------------------------------------------------------------------------
function openMenu(root: HTMLElement, item: HTMLElement, x: number | null, y: number | null): void {
  const menu = root.querySelector<HTMLElement>("[data-outline-menu]");
  if (!menu) return;
  const level = levelOf(item);
  const enable: Record<string, boolean> = {
    rename: true,
    "add-before": true,
    "add-after": true,
    "add-into": level < 3,
    "part-before": level <= 1,
    "part-after": level <= 1,
    promote: item.dataset.promote !== undefined,
    demote: item.dataset.demote !== undefined,
    delete: item.dataset.children === undefined,
  };
  menu.querySelectorAll<HTMLButtonElement>("button[data-action]").forEach((button) => {
    button.disabled = !enable[button.dataset.action ?? ""];
  });
  const box = root.getBoundingClientRect();
  const anchor = (rowOf(item) ?? item).getBoundingClientRect();
  const left = (x ?? anchor.left + 24) - box.left;
  const top = (y ?? anchor.bottom) - box.top;
  menu.style.left = `${Math.max(0, left)}px`;
  menu.style.top = `${Math.max(0, top)}px`;
  menu.dataset.for = String(keyOf(item));
  menu.hidden = false;
  menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
}

function closeMenu(root: HTMLElement, refocus: boolean): void {
  const menu = root.querySelector<HTMLElement>("[data-outline-menu]");
  if (!menu || menu.hidden) return;
  menu.hidden = true;
  if (refocus) {
    const item = root.querySelector<HTMLElement>(`li[data-key="${menu.dataset.for ?? ""}"]`);
    if (item) entryOf(item)?.focus();
  }
}

function runAction(root: HTMLElement, item: HTMLElement, action: string): void {
  switch (action) {
    case "rename":
      startRename(root, item);
      break;
    case "add-before":
      startAdd(root, item, "before");
      break;
    case "add-after":
      startAdd(root, item, "after");
      break;
    case "add-into":
      startAdd(root, item, "into");
      break;
    case "part-before":
      startAdd(root, item, "before", true);
      break;
    case "part-after":
      startAdd(root, item, "after", true);
      break;
    case "promote":
      setLevel(root, item, -1);
      break;
    case "demote":
      setLevel(root, item, 1);
      break;
    case "delete":
      askDelete(root, item);
      break;
  }
}

register("outline", (root) => {
  let dragged: HTMLElement | null = null;
  const hint = root.querySelector<HTMLElement>("[data-outline-hint]");
  const menu = root.querySelector<HTMLElement>("[data-outline-menu]");
  if (focusAfterRender) {
    focusAfterRender = false;
    root.querySelector<HTMLElement>(".tree-entry.selected")?.focus();
  }
  const clear = () => {
    root.querySelectorAll<HTMLElement>(".outline-row").forEach((row) => row.classList.remove(...marks));
  };
  const show = (text: string | null) => {
    if (!hint) return;
    hint.hidden = text === null;
    hint.textContent = text ?? "";
  };
  const spot = (event: DragEvent) => {
    const row = (event.target as Element | null)?.closest<HTMLElement>(".outline-row");
    const item = row?.closest<HTMLElement>("li[data-outline-node]");
    if (!row || !item || !dragged) return null;
    const box = row.getBoundingClientRect();
    const y = (event.clientY - box.top) / Math.max(1, box.height);
    const place: Place = y < 1 / 3 ? "before" : y > 2 / 3 ? "after" : "into";
    return { row, item, place };
  };

  root.addEventListener("dragstart", (event) => {
    const row = (event.target as Element | null)?.closest<HTMLElement>(".outline-row");
    const item = row?.closest<HTMLElement>("li[data-outline-node]");
    if (!row || !item) return;
    dragged = item;
    item.classList.add("dragging");
    if (event.dataTransfer) {
      event.dataTransfer.setData("text/plain", item.dataset.title ?? "");
      event.dataTransfer.effectAllowed = "move";
    }
  });
  root.addEventListener("dragend", () => {
    dragged?.classList.remove("dragging");
    dragged = null;
    clear();
    show(null);
  });
  root.addEventListener("dragover", (event) => {
    const found = spot(event);
    if (!found || !dragged) return;
    const verdict = judge(dragged, found.item, found.place);
    clear();
    found.row.classList.add(verdict.ok ? `drop-${found.place}` : "drop-invalid");
    show(verdict.text);
    if (verdict.ok) {
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    }
  });
  root.addEventListener("drop", (event) => {
    const found = spot(event);
    const source = dragged;
    if (!found || !source) return;
    event.preventDefault();
    clear();
    show(null);
    if (!judge(source, found.item, found.place).ok) return;
    void act(Number(root.dataset.work), "outline.move", {
      node: nodeOf(source),
      key: keyOf(source),
      target: nodeOf(found.item),
      target_key: keyOf(found.item),
      place: found.place,
    });
  });

  // Context menu: right click on an entry or the button „⋯“.
  root.addEventListener("contextmenu", (event) => {
    const item = (event.target as Element | null)?.closest<HTMLElement>("li[data-outline-node]");
    if (!item || typing(event.target as Element)) return;
    event.preventDefault();
    openMenu(root, item, event.clientX, event.clientY);
  });
  root.addEventListener("click", (event) => {
    const opener = (event.target as Element | null)?.closest<HTMLElement>("[data-outline-menu-open]");
    if (!opener) return;
    event.preventDefault();
    event.stopPropagation();
    const item = opener.closest<HTMLElement>("li[data-outline-node]");
    if (item) openMenu(root, item, null, null);
  });
  menu?.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>("button[data-action]");
    if (!button || button.disabled) return;
    event.stopPropagation();
    const item = root.querySelector<HTMLElement>(`li[data-key="${menu.dataset.for ?? ""}"]`);
    closeMenu(root, false);
    if (item) runAction(root, item, button.dataset.action ?? "");
  });
  menu?.addEventListener("keydown", (event) => {
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>("button[data-action]:not(:disabled)")];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeMenu(root, true);
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      const step = event.key === "ArrowDown" ? 1 : -1;
      buttons[(at + step + buttons.length) % buttons.length]?.focus();
    } else if (event.key === "Tab") {
      closeMenu(root, false);
    }
  });
  menu?.addEventListener("focusout", (event) => {
    const next = event.relatedTarget as Node | null;
    if (!next || !menu.contains(next)) closeMenu(root, false);
  });

  // A double click on the selected entry renames; a single click on it opens it (after a
  // short wait so that the double click is not swallowed by the redraw).
  let pending: number | undefined;
  root.addEventListener(
    "click",
    (event) => {
      const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a.tree-entry.selected");
      if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.detail > 1) return;
      window.clearTimeout(pending);
      pending = window.setTimeout(() => navigate(link.href, false), 300);
    },
    true,
  );
  root.addEventListener("dblclick", (event) => {
    const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a.tree-entry.selected");
    const item = link?.closest<HTMLElement>("li[data-outline-node]");
    if (!item) return;
    event.preventDefault();
    window.clearTimeout(pending);
    startRename(root, item);
  });

  // F2, Del, context menu key and Shift+F10 only with the focus in the outline.
  root.addEventListener("keydown", (event) => {
    if (typing(event.target as Element) || event.altKey || event.ctrlKey || event.metaKey) return;
    const menuKey = event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
    if (event.key !== "F2" && event.key !== "Delete" && !menuKey) return;
    if (menu && menu.contains(event.target as Node)) return;
    const item = focusedItem(root);
    if (!item) return;
    event.preventDefault();
    if (event.key === "F2") startRename(root, item);
    else if (event.key === "Delete") askDelete(root, item);
    else openMenu(root, item, null, null);
  });
});

// A click outside closes the context menu (once for the whole page; the tree is redrawn).
document.addEventListener("pointerdown", (event) => {
  const root = document.querySelector<HTMLElement>("[data-widget=outline]");
  const menu = root?.querySelector<HTMLElement>("[data-outline-menu]");
  if (root && menu && !menu.hidden && !menu.contains(event.target as Node)) closeMenu(root, false);
});

// ---- Keyboard: Alt+Shift+arrow (once for the whole page) -----------------------------------
document.addEventListener("keydown", (event) => {
  if (!event.altKey || !event.shiftKey || event.ctrlKey || event.metaKey) return;
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
  if (typing(document.activeElement)) return;
  const root = document.querySelector<HTMLElement>("[data-widget=outline]");
  const item = root?.querySelector(".tree-entry.selected")?.closest<HTMLElement>("li[data-outline-node]");
  if (!root || !item) return;
  event.preventDefault();
  const work = Number(root.dataset.work);
  const node = nodeOf(item);
  const key = keyOf(item);
  switch (event.key) {
    case "ArrowLeft":
      setLevel(root, item, -1);
      break;
    case "ArrowRight":
      setLevel(root, item, 1);
      break;
    case "ArrowUp":
      if (item.dataset.up !== undefined) {
        const target = { target: Number(item.dataset.up), target_key: Number(item.dataset.upKey) };
        void act(work, "outline.move", { node, key, ...target, place: "before" });
      }
      break;
    case "ArrowDown":
      if (item.dataset.down !== undefined) {
        const target = { target: Number(item.dataset.down), target_key: Number(item.dataset.downKey) };
        void act(work, "outline.move", { node, key, ...target, place: "after" });
      }
      break;
  }
});

// ---- Numbering in the head of the outline ---------------------------------------------------
document.addEventListener(
  "submit",
  (event) => {
    const form = (event.target as Element | null)?.closest<HTMLFormElement>("form[data-outline-numbering]");
    if (!form) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const data = new FormData(form);
    const raw = String(data.get("numbering") ?? "").trim();
    const numbering = raw === "" || raw === "vorlage" ? null : raw === "keine" ? "" : raw;
    void act(Number(data.get("work")), "outline.set_numbering", {
      node: Number(data.get("node")),
      key: Number(data.get("key")),
      numbering,
    });
  },
  true,
);

// ---- Editor: before saving check headings and confirm splitting/merging ----------------------
async function checkThenSubmit(form: HTMLFormElement, submitter: HTMLElement | null): Promise<void> {
  // Round 23a (code review web-1): one save at a time; until it is finished the editor stays locked.
  if (editorLocked()) return;
  const release = lockEditor();
  try {
    await checkedSubmit(form, submitter);
  } finally {
    release();
  }
}

async function checkedSubmit(form: HTMLFormElement, submitter: HTMLElement | null): Promise<void> {
  // Other widgets (typeset view) write their state into the text field first; the source of truth
  // on submitting stays the text field.
  const wait: Promise<unknown>[] = [];
  form.dispatchEvent(new CustomEvent("baruch:before-save", { detail: { wait } }));
  try {
    await Promise.all(wait);
  } catch (e) {
    notify(String((e as Error).message ?? e));
    return;
  }
  const data = new FormData(form);
  const params = {
    work: Number(data.get("work")),
    node: Number(data.get("node")),
    key: Number(data.get("key")),
    body: String(data.get("body") ?? ""),
  };
  let check: HeadingCheck;
  try {
    check = await call<HeadingCheck>("outline.check", params);
  } catch (error) {
    notify(String((error as Error).message ?? error));
    return;
  }
  if (!check.valid) {
    notify(check.message ?? t("widget.outline.invalid_headings"));
    return;
  }
  const extra: [string, string][] = [];
  if (check.split.length > 0) {
    if (!confirm(t("widget.outline.split_question", { headings: check.split.join(", ") }))) return;
    extra.push(["split", "1"]);
  }
  if (check.merge) {
    if (!confirm(`${check.message ?? t("widget.outline.no_heading")}\n${t("widget.outline.merge_question")}`)) return;
    extra.push(["merge", "1"]);
  }
  const added = extra.map(([name, value]) => {
    const input = document.createElement("input");
    input.type = "hidden";
    input.name = name;
    input.value = value;
    form.append(input);
    return input;
  });
  // The second submit goes through to the workspace (RPC outline.save).
  form.dataset.checked = "1";
  try {
    if (submitter instanceof HTMLButtonElement && submitter.form === form) form.requestSubmit(submitter);
    else form.requestSubmit();
  } finally {
    delete form.dataset.checked;
    added.forEach((input) => input.remove());
  }
}

document.addEventListener(
  "submit",
  (event) => {
    const form = (event.target as Element | null)?.closest<HTMLFormElement>("form[data-outline-save]");
    if (!form || form.dataset.checked === "1") return;
    event.preventDefault();
    event.stopImmediatePropagation();
    void checkThenSubmit(form, (event as SubmitEvent).submitter ?? null);
  },
  true,
);
