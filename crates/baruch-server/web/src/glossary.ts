// SPDX-License-Identifier: AGPL-3.0-or-later
// Glossary and index page (Round 31c, ownership kontextmenue): convenience on top of the pages
// /werke/{id}/glossar and /werke/{id}/register, which are usable without JavaScript.
//
// Widget „glossary“ on `main.glossary-page` or `main.index-page`: the search field `[data-glossary-filter]` (hidden
// without JavaScript) hides, while typing, all entries `[data-filter-text]` that do not contain the search text
// (case and accents do not count, `matches`); Escape empties it. All forms stay normal
// forms (POST …/act, 303/422).

import { register } from "./widgets.js";

/// Comparison form: lower case, without accents (NFD without characters of category M), whitespace to one space.
export function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/\s+/g, " ").trim();
}

/// Does the entry match the search text? An empty search text always matches; every word of the search text must occur.
export function matches(query: string, text: string): boolean {
  const words = fold(query).split(" ").filter((w) => w !== "");
  const hay = fold(text);
  return words.every((w) => hay.includes(w));
}

if (typeof document !== "undefined") {
  register("glossary", (page) => {
    const input = page.querySelector<HTMLInputElement>("[data-glossary-filter]");
    if (!input) return;
    const wrap = input.closest<HTMLElement>("[hidden]");
    if (wrap) wrap.hidden = false;
    const count = page.querySelector<HTMLElement>("[data-glossary-count]");
    const apply = () => {
      let shown = 0;
      const rows = page.querySelectorAll<HTMLElement>("[data-filter-text]");
      rows.forEach((row) => {
        const ok = matches(input.value, row.dataset.filterText ?? "");
        row.hidden = !ok;
        if (ok) shown++;
      });
      if (count) count.textContent = input.value.trim() === "" ? "" : `${shown} von ${rows.length}`;
    };
    input.addEventListener("input", apply);
    input.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && input.value !== "") {
        event.preventDefault();
        input.value = "";
        apply();
      }
    });
  });
}
