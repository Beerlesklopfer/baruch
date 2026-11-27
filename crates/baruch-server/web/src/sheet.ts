// SPDX-License-Identifier: AGPL-3.0-or-later
// Typeset-view tools (Round 18c). Without JavaScript everything goes through the form „Bild einfügen“ (place from a
// list: at the end, at the beginning, before a block) and Einstellungen → Werk → Satzbild.
//
// - Widget „sheet-tools“ on `details[data-image-insert]` (fragments/editor.html): the list of places gets
//   „an der Cursorstelle“ at the front. On submitting, with the typeset view open (`div.wysiwyg[data-ws-open]`), the
//   widget asks through `baruch:ws-caret` (ws_command.ts) for source text and block boundary at the cursor, in the
//   source text it takes `selectionStart` of the text field. It sends `place=offset:<n>` and the unsaved text along
//   as `body` (the server inserts the image at the next line boundary ≥ n as a block of its own). The place is
//   already remembered when pressing „Bild einfügen …“ or the button „Bild“ of the ribbon (`[data-ribbon-image]`,
//   docked in the header since Round 33 – therefore the module listens on the document), as long as the
//   cursor is still in the typeset view. If nobody answers `baruch:ws-caret`, the text goes along through
//   `baruch:serialize-text` and the image to the end.
// - Style of the typeset view: after `baruch:settings-changed` (settings saved) the module fetches `work.sheet` and
//   replaces the content of `<style id="ws-style" data-work="W">` (from the server in the editor, otherwise from
//   wysiwyg.ts).
//
// wysiwyg.ts stays untouched: it speaks only through the events from ws_command.ts and `baruch:serialize-text`.

import { call } from "./rpc.js";
import { register } from "./widgets.js";
import { WS_CARET, type WsCaretDetail } from "./ws_command.js";

const CURSOR = "cursor";

interface Caret {
  body: string;
  offset: number;
}

interface SheetAnswer {
  css: string;
}

/// Editor area (`div.wysiwyg`), form and text field for the widget.
function editorOf(details: HTMLElement): { root: HTMLElement; form: HTMLFormElement; area: HTMLTextAreaElement } | null {
  const root = details.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
  const form = root?.querySelector<HTMLFormElement>("form#editor-form, form[data-widget=editor]");
  const area = form?.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  return root && form && area ? { root, form, area } : null;
}

/// Source text and place at the cursor: in the typeset view through `baruch:ws-caret` (on the editor form, bubbles),
/// in the source text from the text field. `null` if the open typeset view does not answer.
async function caretOf(root: HTMLElement, form: HTMLFormElement, area: HTMLTextAreaElement): Promise<Caret | null> {
  if (!root.hasAttribute("data-ws-open")) return { body: area.value, offset: area.selectionStart ?? area.value.length };
  const detail: WsCaretDetail = { wait: [] };
  form.dispatchEvent(new CustomEvent<WsCaretDetail>(WS_CARET, { detail, bubbles: true }));
  if (detail.wait.length === 0) return null;
  try {
    const answer = await detail.wait[0];
    return typeof answer?.body === "string" && Number.isFinite(answer.offset) ? answer : null;
  } catch {
    return null;
  }
}

/// Complete source text without a place (fallback): `baruch:serialize-text` as in timing.ts/literature.ts.
async function sourceText(form: HTMLFormElement, area: HTMLTextAreaElement): Promise<string> {
  const wait: Promise<string>[] = [];
  form.dispatchEvent(new CustomEvent("baruch:serialize-text", { detail: { wait } }));
  if (wait.length > 0) {
    try {
      return await wait[0];
    } catch {
      // fallback below
    }
  }
  return area.value;
}

/// Memo of the editor currently shown (a new widget after every `ui.render`) for the button „Bild“ in the ribbon.
let rememberCurrent: { details: HTMLElement; remember: () => void } | null = null;

document.addEventListener(
  "pointerdown",
  (event) => {
    if (!rememberCurrent?.details.isConnected) return;
    if ((event.target as Element | null)?.closest?.("[data-ribbon-image]")) rememberCurrent.remember();
  },
  true,
);

register("sheet-tools", (details) => {
  const editor = editorOf(details);
  const form = details.querySelector<HTMLFormElement>("form.image-insert-form");
  const place = form?.querySelector<HTMLSelectElement>("select[data-image-place]");
  const body = form?.querySelector<HTMLInputElement>("input[data-image-body]");
  if (!editor || !form || !place || !body) return;
  const cursor = document.createElement("option");
  cursor.value = CURSOR;
  cursor.textContent = "an der Cursorstelle";
  place.prepend(cursor);
  place.value = CURSOR;
  const note = form.querySelector<HTMLElement>("[data-image-note]");
  if (note) note.textContent = "Als eigener Block an der gewählten Stelle; ungespeicherte Änderungen gehen mit.";

  // Remember the place as long as the cursor is still in the typeset view or text field (before the focus change).
  let remembered: Promise<Caret | null> | null = null;
  const remember = () => {
    remembered = caretOf(editor.root, editor.form, editor.area);
  };
  details.querySelector("summary")?.addEventListener("pointerdown", remember);
  // The button „Bild“ has stood docked in the ribbon since Round 33 (band.ts), i.e. outside `div.wysiwyg`: the module
  // listens once on the document (below) and asks the widget of the editor currently shown.
  rememberCurrent = { details, remember };

  let sending = false;
  form.addEventListener("submit", (event) => {
    if (sending || place.value !== CURSOR) return;
    event.preventDefault();
    void (async () => {
      const caret = (await remembered) ?? (await caretOf(editor.root, editor.form, editor.area));
      if (caret) {
        const option = document.createElement("option");
        option.value = `offset:${Math.max(0, Math.floor(caret.offset))}`;
        option.textContent = "an der Cursorstelle";
        place.append(option);
        place.value = option.value;
        body.value = caret.body;
      } else {
        place.value = "end";
        body.value = await sourceText(editor.form, editor.area);
      }
      body.disabled = false;
      sending = true;
      form.submit();
    })();
  });
});

/// Fetch the style of the typeset view anew after changed settings (`work.sheet`) and write it into every
/// `#ws-style` of the work (server style in the editor or the element that wysiwyg.ts created).
async function refreshStyle(): Promise<void> {
  const styles = Array.from(document.querySelectorAll<HTMLStyleElement>("style#ws-style[data-work]"));
  const works = new Set(styles.map((style) => style.dataset.work ?? "").filter((work) => /^\d+$/.test(work)));
  for (const work of works) {
    try {
      const answer = await call<SheetAnswer>("work.sheet", { work: Number(work) });
      for (const style of styles) {
        if (style.dataset.work === work) style.textContent = answer.css;
      }
    } catch {
      // Without an answer the previous style stays; on the next load it comes from the server.
    }
  }
}

document.addEventListener("baruch:settings-changed", (event) => {
  const page = (event as CustomEvent<{ page?: string }>).detail?.page;
  if (page === undefined || page === "" || page === "werk") void refreshStyle();
});
