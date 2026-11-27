// SPDX-License-Identifier: AGPL-3.0-or-later
// Comparing variants side by side (Round 30b, ownership chat-loesungen): widget „variants“ of the page /varianten
// (templates/variants.html). Without JavaScript everything is there (columns, differences, forms to POST
// /loesungen/act); added here is only:
// - If one opens „Unterschied zu den übrigen Varianten“ in one column, the corresponding areas of the other
//   columns open or close along (same height, easier to compare).
// - „Übrige verwerfen“ asks once more before submitting (the checkbox stays the confirmation without JavaScript).

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
