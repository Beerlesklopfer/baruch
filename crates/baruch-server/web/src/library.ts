// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „library“: filtert die Titelliste der Literatur beim Tippen im Suchfeld (nur im Browser,
// ohne Serveranfrage). Abschicken bleibt das gewöhnliche GET-Formular (?lq=…), das der Server mit
// der Volltextsuche über Werk und Bibliothek beantwortet. Ein Sprung auf #lit-<Schlüssel> klappt
// den Titel auf.

import { register } from "./widgets.js";

/// Kleinbuchstaben ohne diakritische Zeichen, damit „langmutig“ auch „langmütig“ findet.
function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

/// Ziel-Id aus dem Anker; ein ungültiges %-Escape (`#lit-%E0`) gilt als kein Ziel.
function targetId(): string | null {
  try {
    return decodeURIComponent(location.hash.slice(1));
  } catch {
    return null;
  }
}

/// Titel zum aktuellen Anker aufklappen. Sucht die Literatur-Wurzel jedes Mal neu im Dokument,
/// weil `ui.render` die linke Spalte ersetzt und alte Wurzeln dann nicht mehr eingehängt sind.
function openTarget(): void {
  const id = targetId();
  if (!id || !id.startsWith("lit-")) return;
  const title = document.getElementById(id);
  if (!(title instanceof HTMLDetailsElement) || !title.closest('[data-widget="library"]')) return;
  title.hidden = false;
  title.open = true;
  title.scrollIntoView({ block: "nearest" });
}

// Genau ein Listener auf Modulebene (das Modul wird nur einmal ausgewertet), nicht je Einhängen.
window.addEventListener("hashchange", openTarget);

register("library", (root) => {
  const input = root.querySelector<HTMLInputElement>('input[name="lq"]');
  const titles = Array.from(root.querySelectorAll<HTMLDetailsElement>("details.library-title"));
  const none = root.querySelector<HTMLElement>(".library-none");
  const texts = titles.map((t) => fold(t.textContent ?? ""));

  const filter = (): void => {
    const words = fold(input?.value ?? "").split(/\s+/).filter((w) => w.length > 0);
    let shown = 0;
    titles.forEach((title, i) => {
      const match = words.every((w) => texts[i].includes(w));
      title.hidden = !match;
      if (match) shown += 1;
    });
    if (none) none.hidden = shown > 0 || titles.length === 0;
  };

  input?.addEventListener("input", filter);
  // Beim Einhängen nicht filtern: Die Liste zeigt, was der Server geliefert hat.
  openTarget();
});
