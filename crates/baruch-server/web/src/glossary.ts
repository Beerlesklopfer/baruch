// SPDX-License-Identifier: AGPL-3.0-or-later
// Glossar- und Registerseite (Runde 31c, Besitz kontextmenue): Komfort über den ohne JavaScript bedienbaren Seiten
// /werke/{id}/glossar und /werke/{id}/register.
//
// Widget „glossary“ auf `main.glossary-page` bzw. `main.index-page`: Das Suchfeld `[data-glossary-filter]` (ohne
// JavaScript verborgen) blendet beim Tippen alle Einträge `[data-filter-text]` aus, die den Suchtext nicht enthalten
// (Groß-/Kleinschreibung und Akzente zählen nicht, `matches`); Escape leert es. Alle Formulare bleiben normale
// Formulare (POST …/act, 303/422).

import { register } from "./widgets.js";

/// Vergleichsform: klein, ohne Akzente (NFD ohne Zeichen der Kategorie M), Leerraum zu einem Leerzeichen.
export function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/\s+/g, " ").trim();
}

/// Passt der Eintrag zum Suchtext? Leerer Suchtext passt immer; jedes Wort des Suchtexts muss vorkommen.
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
