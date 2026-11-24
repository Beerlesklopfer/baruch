// SPDX-License-Identifier: AGPL-3.0-or-later
// Varianten nebeneinander vergleichen (Runde 30b, Besitz chat-loesungen): Widget „variants“ der Seite /varianten
// (templates/variants.html). Ohne JavaScript ist alles da (Spalten, Unterschiede, Formulare an POST /loesungen/act);
// hier kommt nur dazu:
// - Öffnet man in einer Spalte „Unterschied zu den übrigen Varianten“, öffnen bzw. schließen sich die entsprechenden
//   Bereiche der anderen Spalten mit (gleiche Höhe, leichter zu vergleichen).
// - „Übrige verwerfen“ fragt vor dem Absenden noch einmal nach (das Kästchen bleibt die Bestätigung ohne JavaScript).

import { register } from "./widgets.js";

register("variants", (root) => {
  const details = Array.from(root.querySelectorAll<HTMLDetailsElement>("details.variant-against"));
  for (const one of details) {
    one.addEventListener("toggle", () => {
      for (const other of details) {
        if (other !== one && other.open !== one.open) other.open = one.open;
      }
    });
  }
  root.querySelectorAll<HTMLFormElement>("form.variant-reject").forEach((form) => {
    form.addEventListener("submit", (event) => {
      const label = form.getAttribute("aria-label") ?? "Übrige Varianten verwerfen";
      if (!window.confirm(`${label}?`)) event.preventDefault();
    });
  });
});
