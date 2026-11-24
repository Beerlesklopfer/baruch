// SPDX-License-Identifier: AGPL-3.0-or-later
// Zeitwächter im Editorkopf (Runde 16b): Das Widget „timing“ (p.editor-timing, nur bei Predigt oder
// Zielzeit) zählt beim Tippen entprellt nach. Gezählt wird nur auf dem Server (RPC `timing.count`, dieselbe
// Regel wie beim Laden der Seite) – hier gibt es keine zweite Zählregel. Geschickt wird immer der Quelltext
// (Runde 17b, L6): Ist das Satzbild offen, liefert es ihn über das Ereignis `baruch:serialize-text` am
// Editor-Formular (`detail.wait`, Besitz wysiwyg.ts, ohne zu speichern); so zählen Typst-Inseln (Fußnoten,
// Arbeitsaufträge, Code) genau wie beim Laden der Seite. Antwortet niemand, gilt im offenen Satzbild dessen
// sichtbarer Text (innerText, Rückfall), sonst das Textfeld. Fehler bleiben leise: Die Anzeige behält den
// letzten Stand.

import { call } from "./rpc.js";
import { register } from "./widgets.js";

interface Counted {
  words: number;
  seconds: number;
  clock: string;
  words_per_minute: number;
}

const WAIT_MS = 700;

/// Uhr-Schreibweise wie `baruch_typst::timing::clock` (nur Anzeige der Summe, keine Zählung).
function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

/// Quelltext, der gerade im Editor steht: vom Satzbild über `baruch:serialize-text` (wenn offen); ohne Antwort
/// beim offenen Satzbild dessen sichtbarer Text, sonst das Textfeld.
async function editorText(editor: HTMLElement): Promise<string | null> {
  const form = editor.querySelector<HTMLFormElement>("form[data-widget=editor]");
  if (form) {
    const wait: Promise<string>[] = [];
    form.dispatchEvent(new CustomEvent("baruch:serialize-text", { detail: { wait } }));
    if (wait.length > 0) {
      try {
        const texts = await Promise.all(wait);
        const text = texts.find((t) => typeof t === "string");
        if (text !== undefined) return text;
      } catch {
        // Rückfall unten
      }
    }
  }
  // Niemand zuständig: offenes Satzbild → sein sichtbarer Text, sonst das Textfeld.
  const wrap = editor.querySelector<HTMLElement>("[data-ws-wrap]");
  const sheet = editor.querySelector<HTMLElement>("[data-ws-sheet]");
  if (wrap && sheet && !wrap.hidden) return sheet.innerText;
  const area = editor.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  return area ? area.value : null;
}

register("timing", (element) => {
  const editor = document.querySelector<HTMLElement>("[data-widget=wysiwyg]");
  if (!editor) return;
  const work = Number(element.dataset.work);
  const rest = Number(element.dataset.rest ?? "0");
  const target = element.dataset.target === undefined ? null : Number(element.dataset.target);
  const sectionOut = element.querySelector<HTMLElement>("[data-timing-section]");
  const totalOut = element.querySelector<HTMLElement>("[data-timing-total]");
  const overOut = element.querySelector<HTMLElement>("[data-timing-over]");
  let timer: number | undefined;
  let asked = 0;

  const update = async (): Promise<void> => {
    if (!element.isConnected) return;
    const mine = ++asked;
    const text = await editorText(editor);
    if (text === null || mine !== asked) return;
    try {
      const counted = await call<Counted>("timing.count", { work, text });
      if (mine !== asked) return;
      const total = rest + counted.seconds;
      if (sectionOut) sectionOut.textContent = counted.clock;
      if (totalOut) totalOut.textContent = clock(total);
      if (overOut && target !== null) {
        const over = total > target;
        overOut.hidden = !over;
        overOut.textContent = over ? `Zielzeit überschritten um ${clock(total - target)}` : "Zielzeit überschritten";
      }
    } catch {
      // leise: der letzte Stand bleibt stehen
    }
  };

  editor.addEventListener("input", () => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(() => void update(), WAIT_MS);
  });
});
