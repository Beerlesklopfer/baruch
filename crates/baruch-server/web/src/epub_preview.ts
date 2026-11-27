// SPDX-License-Identifier: AGPL-3.0-or-later
// EPUB preview per reader (Round 20b, ownership „reader“): convenience on top of the page
// /werke/{id}/epub/vorschau, which is usable without JavaScript (form GET and iframes).
//
// Widget „epub-preview“ on the form `form.epub-preview-form`: a change to a checkbox (reader on/off,
// night mode) submits the form at once – without JavaScript the button „Anzeigen“ does that. The page itself
// stays an ordinary GET; address and back button of the browser are therefore always right.
//
// Round 23a (review R20, WCAG 3.2.2): choice lists (file, footnotes) submit only on confirmation. Firefox
// reports `change` on every arrow key in a closed list; that would reload the whole preview on every
// key press and take the focus away from the list. Therefore: a choice with the mouse counts as confirmed and is
// submitted at once; a choice with the keyboard waits for the Enter key in the list (or for „Anzeigen“).

import { register } from "./widgets.js";

/// Keys with which another row can be chosen in a choice list (also letters for jumping to an entry).
export function choosesWithKeyboard(key: string): boolean {
  return (
    ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"].includes(key) ||
    key.length === 1
  );
}

/// Attaches the behaviour to the form of the preview.
export function mountPreviewForm(element: HTMLElement): void {
  if (!(element instanceof HTMLFormElement)) return;
  const form = element;
  // Without a chosen reader the server shows the mandatory profiles – when unchecking the last checkbox that would be
  // a surprise; then the widget waits for „Anzeigen“.
  const ready = () => !!form.querySelector<HTMLInputElement>("input[name=leser]:checked");
  form.addEventListener("keydown", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLSelectElement)) return;
    if (event.key === "Enter") {
      // Confirmation of a choice with the keyboard.
      if (target.dataset.pending === "1" && ready()) {
        event.preventDefault();
        delete target.dataset.pending;
        form.requestSubmit();
      } else if (target.dataset.keyboard === "1") {
        // Expanded list: the browser reports `change` only after this Enter key – it is the confirmation.
        target.dataset.confirmed = "1";
        setTimeout(() => delete target.dataset.confirmed, 300);
      }
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (choosesWithKeyboard(event.key)) target.dataset.keyboard = "1";
  });
  // A choice with the mouse is no longer one with the keyboard.
  form.addEventListener("pointerdown", (event) => {
    const target = event.target;
    const select = target instanceof HTMLElement ? target.closest<HTMLSelectElement>("select") : null;
    if (select) delete select.dataset.keyboard;
  });
  form.addEventListener("change", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement || target instanceof HTMLSelectElement)) return;
    if (target instanceof HTMLSelectElement && target.dataset.keyboard === "1" && target.dataset.confirmed !== "1") {
      target.dataset.pending = "1";
      return;
    }
    if (!ready()) return;
    form.requestSubmit();
  });
}

register("epub-preview", mountPreviewForm);
