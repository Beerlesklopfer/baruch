// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „persona-tabs“ (Round 11, Einstellungen → Assistenten): one tab per persona. The server supplies
// all panels (`[role=tabpanel]`, those not chosen with `hidden`) and per tab a link
// `?persona=<handle ohne @>`; without JavaScript the link reloads the page with this tab.
//
// With JavaScript a click (or arrow left/right, Home, End on the tab) switches without reloading:
// `aria-selected`, `tabindex` (only the chosen tab is in the tab order) and `hidden` of the panels
// follow; on the page /einstellungen/assistenten the address follows by `history.replaceState`
// (`persona=` set, anchor `#persona-<handle>`). In the settings popup the address of the
// workspace page stays untouched. Ctrl/Shift/middle click open the link as usual.

import { register } from "./widgets.js";

/// New address of the page with `persona=<wert>` (the other pairs stay, `ok`/`rueckfrage` are dropped
/// so that an old notice does not appear again on the next load).
export function withPersona(href: string, persona: string): string {
  const url = new URL(href);
  url.searchParams.delete("ok");
  url.searchParams.delete("rueckfrage");
  url.searchParams.set("persona", persona);
  url.hash = `persona-${persona}`;
  return url.toString();
}

/// Index of the tab after a key: arrows move in a circle, Home/End jump; otherwise `null`.
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
        // The address cannot be changed (e.g. embedded): the panel is switched all the same.
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
