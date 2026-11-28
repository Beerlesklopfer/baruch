// SPDX-License-Identifier: AGPL-3.0-or-later
// Quick capture of an idea (contract Round 7, section 6e). Without JavaScript the
// button „Einfall“ of the header leads to the page /einfaelle with a form.
//
// - Widget „idea-capture“ (<dialog id="idea-capture">): a click on „Einfall“ ([data-idea-open]) and
//   Alt+E open the dialog. Selected text from the typeset view, the text field of the editor
//   (selectionStart/End) or the literature popup (#literature) becomes the field „Markiert“ (quote);
//   from the literature the source „Literatur“ is added (with the title, if in the DOM). `key` is the
//   fixed key from the editor form, `work` the work of the page.
// - Enter saves through RPC idea.create (Shift+Enter: new line in the thought), Escape closes;
//   afterwards focus and selection return to the old place.

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { register } from "./widgets.js";

interface Created {
  idea: { id: number };
  commit?: string;
  mirror?: string;
}

/// Where work was done before opening: element with focus and selection.
interface Spot {
  focus: HTMLElement | null;
  start?: number;
  end?: number;
  range?: Range;
}

function toast(message: string, ok: boolean): void {
  const element = document.createElement("div");
  element.className = "toast";
  element.setAttribute("role", ok ? "status" : "alert");
  if (ok) element.style.background = "var(--success)";
  element.textContent = message;
  document.body.append(element);
  setTimeout(() => element.remove(), ok ? 3000 : 6000);
}

function isTextField(element: Element | null): element is HTMLTextAreaElement | HTMLInputElement {
  return element instanceof HTMLTextAreaElement || (element instanceof HTMLInputElement && element.type === "text");
}

/// Selected text and source: text field of the editor, typeset view or literature popup.
function picked(spot: Spot): { quote: string; source: string } {
  const focus = spot.focus;
  if (isTextField(focus) && focus.closest("#pane-editor") && spot.start !== undefined && spot.end !== undefined) {
    if (spot.end > spot.start) return { quote: focus.value.slice(spot.start, spot.end), source: "" };
  }
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return { quote: "", source: "" };
  const text = selection.toString();
  if (text.trim() === "") return { quote: "", source: "" };
  const node = selection.getRangeAt(0).commonAncestorContainer;
  const element = node instanceof Element ? node : node.parentElement;
  if (!element) return { quote: "", source: "" };
  if (element.closest("#literature")) {
    const title = element.closest(".library-title")?.querySelector("cite")?.textContent?.trim();
    return { quote: text, source: title ? t("widget.ideas.source_titled", { title }) : t("widget.ideas.source") };
  }
  if (element.closest(".source-document")) {
    const title = document.querySelector(".source-head cite, .source-head h1")?.textContent?.trim();
    return { quote: text, source: title ? t("widget.ideas.source_titled", { title }) : t("widget.ideas.source") };
  }
  if (element.closest(".wysiwyg, #pane-editor")) return { quote: text, source: "" };
  return { quote: "", source: "" };
}

/// Work of the page: work selection of the header, otherwise the hidden field of the dialog.
function workOf(form: HTMLFormElement): number | null {
  const select = document.querySelector<HTMLSelectElement>("select[data-widget=work-select]");
  const value = select?.value ?? form.querySelector<HTMLInputElement>("input[name=work]")?.value ?? "";
  return value.trim() === "" || Number.isNaN(Number(value)) ? null : Number(value);
}

/// Fixed key of the open section from the editor form (current after ui.render).
function keyOf(): number | null {
  const field = document.querySelector<HTMLInputElement>("form.editor-form input[name=key]");
  const value = field?.value ?? "";
  return value.trim() === "" || Number.isNaN(Number(value)) ? null : Number(value);
}

register("idea-capture", (element) => {
  const dialog = element as HTMLDialogElement;
  const form = dialog.querySelector<HTMLFormElement>("form[data-idea-form]");
  const opener = document.querySelector<HTMLAnchorElement>("[data-idea-open]");
  if (!form || typeof dialog.showModal !== "function") return;
  const words = form.querySelector<HTMLInputElement>("[data-idea-words]");
  const quote = form.querySelector<HTMLTextAreaElement>("[data-idea-quote]");
  const source = form.querySelector<HTMLInputElement>("[data-idea-source]");
  const keyField = form.querySelector<HTMLInputElement>("[data-idea-key]");
  const nodeLabel = form.querySelector<HTMLElement>("[data-idea-node]");
  const status = form.querySelector<HTMLElement>("[data-idea-status]");
  let spot: Spot = { focus: null };
  let busy = false;

  const remember = (): Spot => {
    const focus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const remembered: Spot = { focus };
    if (isTextField(focus)) {
      remembered.start = focus.selectionStart ?? undefined;
      remembered.end = focus.selectionEnd ?? undefined;
    } else {
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) remembered.range = selection.getRangeAt(0).cloneRange();
    }
    return remembered;
  };

  const restore = (): void => {
    const { focus, start, end, range } = spot;
    if (focus && focus.isConnected) {
      focus.focus({ preventScroll: true });
      if (isTextField(focus) && start !== undefined && end !== undefined) focus.setSelectionRange(start, end);
    }
    if (range) {
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
    spot = { focus: null };
  };

  /// `fresh`: remember focus and selection now (keyboard); on a click they come from the mousedown.
  const open = (fresh: boolean): void => {
    if (dialog.open) return;
    if (fresh || spot.focus === null) spot = remember();
    const found = picked(spot);
    if (quote) quote.value = found.quote;
    if (source && found.source !== "") source.value = found.source;
    const key = keyOf();
    if (keyField) keyField.value = key === null ? "" : String(key);
    const heading = document.querySelector("#pane-editor h2.node-title");
    const number = heading?.querySelector(".num")?.textContent ?? "";
    const title = (heading?.textContent ?? "").slice(number.length).trim();
    if (nodeLabel && title !== "") nodeLabel.textContent = title;
    if (status) status.textContent = "";
    dialog.showModal();
    words?.focus();
  };

  const save = async (): Promise<void> => {
    if (busy) return;
    const data = new FormData(form);
    const text = (name: string) => String(data.get(name) ?? "");
    if (text("words").trim() === "") {
      words?.focus();
      if (status) status.textContent = t("widget.ideas.words_missing");
      return;
    }
    busy = true;
    try {
      const result = await call<Created>("idea.create", {
        work: workOf(form),
        key: keyOf(),
        words: text("words"),
        note: text("note"),
        quote: text("quote"),
        source: text("source"),
      });
      form.reset();
      dialog.close();
      toast(t("widget.ideas.created", { id: result.idea.id }), true);
      if (result.mirror) toast(t("widget.ideas.mirror", { commit: result.mirror }), false);
    } catch (error) {
      if (status) status.textContent = String((error as Error)?.message ?? error);
    } finally {
      busy = false;
    }
  };

  opener?.addEventListener("click", (event) => {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    open(false);
  });
  // Remember the selection before the click: a click on the link takes the focus away from the text field.
  opener?.addEventListener("mousedown", () => {
    spot = remember();
  });
  document.addEventListener("keydown", (event) => {
    // Plus `code`: on some keyboards Alt+E yields another character (e.g. „€“).
    if (!event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key.toLowerCase() !== "e" && event.code !== "KeyE") return;
    event.preventDefault();
    if (dialog.open) dialog.close();
    else open(true);
  });
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void save();
  });
  form.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
    if (!(event.target instanceof HTMLTextAreaElement)) return; // in the input field Enter submits anyway
    event.preventDefault();
    void save();
  });
  form.querySelector("[data-idea-close]")?.addEventListener("click", () => dialog.close());
  // Escape (cancel) and closing: focus and selection back.
  dialog.addEventListener("close", restore);
});
