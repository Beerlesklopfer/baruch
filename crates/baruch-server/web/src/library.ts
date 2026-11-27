// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „library“: filters the title list of the literature while typing in the search field (only in the browser,
// without a server request). Submitting stays the ordinary GET form (?lq=…), which the server answers with
// the full-text search across work and library. A jump to #lit-<Schlüssel> unfolds
// the title.

import { register } from "./widgets.js";

/// Lower-case letters without diacritics so that „langmutig“ also finds „langmütig“.
function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

/// Target id from the anchor; an invalid % escape (`#lit-%E0`) counts as no target.
function targetId(): string | null {
  try {
    return decodeURIComponent(location.hash.slice(1));
  } catch {
    return null;
  }
}

/// Unfold the title for the current anchor. Looks up the literature root in the document anew every time
/// because `ui.render` replaces the left column and old roots are then no longer mounted.
function openTarget(): void {
  const id = targetId();
  if (!id || !id.startsWith("lit-")) return;
  const title = document.getElementById(id);
  if (!(title instanceof HTMLDetailsElement) || !title.closest('[data-widget="library"]')) return;
  title.hidden = false;
  title.open = true;
  title.scrollIntoView({ block: "nearest" });
}

// Exactly one listener at module level (the module is evaluated only once), not one per mount.
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
  // Do not filter on mounting: the list shows what the server supplied.
  openTarget();
});
