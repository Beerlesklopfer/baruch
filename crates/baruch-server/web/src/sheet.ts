// SPDX-License-Identifier: AGPL-3.0-or-later
// Satzbild-Werkzeuge (Runde 18c). Ohne JavaScript geht alles über das Formular „Bild einfügen“ (Stelle aus einer
// Liste: am Ende, am Anfang, vor einem Block) und Einstellungen → Werk → Satzbild.
//
// - Widget „sheet-tools“ auf `details[data-image-insert]` (fragments/editor.html): Die Liste der Stellen bekommt
//   vorn „an der Cursorstelle“. Beim Absenden fragt das Widget bei offenem Satzbild (`div.wysiwyg[data-ws-open]`)
//   über `baruch:ws-caret` (ws_command.ts) nach Quelltext und Blockgrenze am Cursor, im Quelltext nimmt es
//   `selectionStart` des Textfelds. Es schickt `place=offset:<n>` und den ungespeicherten Text als `body` mit
//   (der Server fügt das Bild an der nächsten Zeilengrenze ≥ n als eigenen Block ein). Die Stelle wird schon beim
//   Drücken auf „Bild einfügen …“ bzw. den Knopf „Bild“ des Menübands (`[data-ribbon-image]`, seit Runde 33 angedockt
//   im Kopf – darum lauscht das Modul am Dokument) gemerkt, solange der
//   Cursor noch im Satzbild steht. Antwortet niemand auf `baruch:ws-caret`, geht der Text über
//   `baruch:serialize-text` mit und das Bild ans Ende.
// - Stil des Satzbilds: Nach `baruch:settings-changed` (Einstellungen gespeichert) holt das Modul `work.sheet` und
//   ersetzt den Inhalt von `<style id="ws-style" data-work="W">` (vom Server im Editor, sonst von wysiwyg.ts).
//
// wysiwyg.ts bleibt unberührt: Es spricht nur über die Ereignisse aus ws_command.ts und `baruch:serialize-text`.

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

/// Editor-Bereich (`div.wysiwyg`), Formular und Textfeld zum Widget.
function editorOf(details: HTMLElement): { root: HTMLElement; form: HTMLFormElement; area: HTMLTextAreaElement } | null {
  const root = details.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
  const form = root?.querySelector<HTMLFormElement>("form#editor-form, form[data-widget=editor]");
  const area = form?.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  return root && form && area ? { root, form, area } : null;
}

/// Quelltext und Stelle am Cursor: im Satzbild über `baruch:ws-caret` (am Editor-Formular, steigt auf), im
/// Quelltext aus dem Textfeld. `null`, wenn das offene Satzbild nicht antwortet.
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

/// Vollständiger Quelltext ohne Stelle (Rückfall): `baruch:serialize-text` wie in timing.ts/literature.ts.
async function sourceText(form: HTMLFormElement, area: HTMLTextAreaElement): Promise<string> {
  const wait: Promise<string>[] = [];
  form.dispatchEvent(new CustomEvent("baruch:serialize-text", { detail: { wait } }));
  if (wait.length > 0) {
    try {
      return await wait[0];
    } catch {
      // Rückfall unten
    }
  }
  return area.value;
}

/// Merker des gerade gezeigten Editors (nach jedem `ui.render` ein neues Widget) für den Knopf „Bild“ im Menüband.
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

  // Stelle merken, solange der Cursor noch im Satzbild bzw. Textfeld steht (vor dem Fokuswechsel).
  let remembered: Promise<Caret | null> | null = null;
  const remember = () => {
    remembered = caretOf(editor.root, editor.form, editor.area);
  };
  details.querySelector("summary")?.addEventListener("pointerdown", remember);
  // Der Knopf „Bild“ steht seit Runde 33 angedockt im Menüband (band.ts), also außerhalb von `div.wysiwyg`: Das Modul
  // lauscht einmal am Dokument (unten) und fragt das Widget des gerade gezeigten Editors.
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

/// Stil des Satzbilds nach geänderten Einstellungen neu holen (`work.sheet`) und in jedes `#ws-style` des Werks
/// schreiben (Server-Stil im Editor bzw. das Element, das wysiwyg.ts angelegt hat).
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
      // Ohne Antwort bleibt der bisherige Stil; beim nächsten Laden kommt er vom Server.
    }
  }
}

document.addEventListener("baruch:settings-changed", (event) => {
  const page = (event as CustomEvent<{ page?: string }>).detail?.page;
  if (page === undefined || page === "" || page === "werk") void refreshStyle();
});
