// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „persona-tabs“ (Runde 11, Einstellungen → Assistenten): je Persona ein Tab. Der Server liefert
// alle Tafeln (`[role=tabpanel]`, die nicht gewählten mit `hidden`) und je Tab einen Link
// `?persona=<handle ohne @>`; ohne JavaScript lädt der Link die Seite mit diesem Tab neu.
//
// Mit JavaScript schaltet ein Klick (bzw. Pfeil links/rechts, Pos1, Ende auf dem Tab) ohne Neuladen um:
// `aria-selected`, `tabindex` (nur der gewählte Tab ist in der Tab-Reihenfolge) und `hidden` der Tafeln
// folgen; auf der Seite /einstellungen/assistenten folgt die Adresse per `history.replaceState`
// (`persona=` gesetzt, Anker `#persona-<handle>`). Im Einstellungs-Popup bleibt die Adresse der
// Arbeitsseite unberührt. Strg-/Umschalt-/Mittelklick öffnen den Link wie gewohnt.

import { register } from "./widgets.js";

/// Neue Adresse der Seite mit `persona=<wert>` (übrige Paare bleiben, `ok`/`rueckfrage` entfallen,
/// damit ein alter Hinweis nicht beim nächsten Laden wieder erscheint).
export function withPersona(href: string, persona: string): string {
  const url = new URL(href);
  url.searchParams.delete("ok");
  url.searchParams.delete("rueckfrage");
  url.searchParams.set("persona", persona);
  url.hash = `persona-${persona}`;
  return url.toString();
}

/// Index des Tabs nach einer Taste: Pfeile wandern im Kreis, Pos1/Ende springen; sonst `null`.
export function nextIndex(key: string, at: number, count: number): number | null {
  if (count <= 0) return null;
  switch (key) {
    case "ArrowRight":
      return (at + 1) % count;
    case "ArrowLeft":
      return (at - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

register("persona-tabs", (root) => {
  const tabs = [...root.querySelectorAll<HTMLAnchorElement>("[role=tab]")];
  if (tabs.length === 0) return;
  const panelOf = (tab: HTMLElement): HTMLElement | null => {
    const id = tab.getAttribute("aria-controls");
    return id ? root.querySelector<HTMLElement>(`[role=tabpanel][id="${CSS.escape(id)}"]`) : null;
  };

  const select = (tab: HTMLAnchorElement, focus: boolean) => {
    for (const other of tabs) {
      const chosen = other === tab;
      other.setAttribute("aria-selected", chosen ? "true" : "false");
      other.tabIndex = chosen ? 0 : -1;
      const panel = panelOf(other);
      if (panel) panel.hidden = !chosen;
    }
    if (focus) tab.focus();
    const persona = tab.dataset.persona;
    if (persona && window.location.pathname === "/einstellungen/assistenten") {
      try {
        history.replaceState(history.state, "", withPersona(window.location.href, decodeURIComponent(persona)));
      } catch {
        // Adresse lässt sich nicht ändern (z. B. eingebettet): die Tafel ist trotzdem gewechselt.
      }
    }
  };

  root.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const tab = (event.target as Element | null)?.closest<HTMLAnchorElement>("[role=tab]");
    if (!tab || !tabs.includes(tab)) return;
    event.preventDefault();
    select(tab, false);
  });

  root.addEventListener("keydown", (event) => {
    const tab = (event.target as Element | null)?.closest<HTMLAnchorElement>("[role=tab]");
    if (!tab) return;
    const at = tabs.indexOf(tab);
    if (at < 0) return;
    const next = nextIndex(event.key, at, tabs.length);
    if (next === null) return;
    event.preventDefault();
    select(tabs[next], true);
  });
});
