// SPDX-License-Identifier: AGPL-3.0-or-later
// EPUB-Vorschau je Reader (Runde 20b, Besitz „reader“): Komfort über der ohne JavaScript bedienbaren Seite
// /werke/{id}/epub/vorschau (Formular-GET und iframes).
//
// Widget „epub-preview“ am Formular `form.epub-preview-form`: Eine Änderung an einem Kästchen (Reader an/aus,
// Nachtmodus) schickt das Formular sofort ab – ohne JavaScript tut das der Knopf „Anzeigen“. Die Seite selbst
// bleibt ein gewöhnliches GET; Adresse und Zurück-Knopf des Browsers stimmen darum immer.
//
// Runde 23a (Gutachten R20, WCAG 3.2.2): Auswahllisten (Datei, Fußnoten) schicken erst bei Bestätigung ab. Firefox
// meldet bei jeder Pfeiltaste in einer geschlossenen Liste `change`; das lüde die ganze Vorschau bei jedem
// Tastendruck neu und nähme der Liste den Fokus. Darum: Eine Wahl mit der Maus gilt als bestätigt und wird sofort
// abgeschickt; eine Wahl mit der Tastatur wartet auf die Eingabetaste in der Liste (oder auf „Anzeigen“).

import { register } from "./widgets.js";

/// Tasten, mit denen sich in einer Auswahlliste eine andere Zeile wählen lässt (auch Buchstaben zum Anspringen).
export function choosesWithKeyboard(key: string): boolean {
  return (
    ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"].includes(key) ||
    key.length === 1
  );
}

/// Hängt das Verhalten an das Formular der Vorschau.
export function mountPreviewForm(element: HTMLElement): void {
  if (!(element instanceof HTMLFormElement)) return;
  const form = element;
  // Ohne gewählten Reader zeigt der Server die Pflicht-Profile – beim Abwählen des letzten Häkchens wäre das
  // eine Überraschung; dann wartet das Widget auf „Anzeigen“.
  const ready = () => !!form.querySelector<HTMLInputElement>("input[name=leser]:checked");
  form.addEventListener("keydown", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLSelectElement)) return;
    if (event.key === "Enter") {
      // Bestätigung einer Wahl mit der Tastatur.
      if (target.dataset.pending === "1" && ready()) {
        event.preventDefault();
        delete target.dataset.pending;
        form.requestSubmit();
      } else if (target.dataset.keyboard === "1") {
        // Aufgeklappte Liste: Der Browser meldet `change` erst nach dieser Eingabetaste – sie ist die Bestätigung.
        target.dataset.confirmed = "1";
        setTimeout(() => delete target.dataset.confirmed, 300);
      }
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (choosesWithKeyboard(event.key)) target.dataset.keyboard = "1";
  });
  // Eine Wahl mit der Maus ist keine mit der Tastatur mehr.
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
