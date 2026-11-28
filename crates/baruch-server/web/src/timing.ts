// SPDX-License-Identifier: AGPL-3.0-or-later
// Time guard in the editor head (Round 16b): the widget „timing“ (p.editor-timing, only for a sermon or a
// target time) recounts while typing, debounced. Counting happens only on the server (RPC `timing.count`, the same
// rule as on loading the page) – there is no second counting rule here. What is sent is always the source text
// (Round 17b, L6): if the typeset view is open, it supplies it through the event `baruch:serialize-text` on the
// editor form (`detail.wait`, ownership wysiwyg.ts, without saving); this way Typst islands (footnotes,
// work orders, code) count exactly as on loading the page. If nobody answers, in the open typeset view its
// visible text applies (innerText, fallback), otherwise the text field. Errors stay quiet: the display keeps the
// last state.

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { register } from "./widgets.js";

interface Counted {
  words: number;
  seconds: number;
  clock: string;
  words_per_minute: number;
}

const WAIT_MS = 700;

/// Clock notation like `baruch_typst::timing::clock` (only display of the sum, no counting).
function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

/// Source text that currently stands in the editor: from the typeset view through `baruch:serialize-text` (if
/// open); without an answer, with the typeset view open, its visible text, otherwise the text field.
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
        // fallback below
      }
    }
  }
  // Nobody responsible: open typeset view → its visible text, otherwise the text field.
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
        overOut.textContent = over ? t("widget.timing.over_by", { time: clock(total - target) }) : t("widget.timing.over");
      }
    } catch {
      // quiet: the last state stays
    }
  };

  editor.addEventListener("input", () => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(() => void update(), WAIT_MS);
  });
});
