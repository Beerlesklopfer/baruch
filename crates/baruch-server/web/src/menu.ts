// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „menubar“ (<nav class="menubar"> in the ribbon, templates/base.html; Round 9, since Round 33 with the one
// menu „Baruch ▾“): the menu is <details>/<summary> with ordinary links and also unfolds without JavaScript. Added
// here:
// - Keyboard following the ARIA menu pattern: on the menu button ↓/Enter/Space opens and focuses the
//   first entry, ↑ the last; in the list ↑/↓ (wrapping), Home/End, Enter/Space activates, Esc closes
//   and focuses the menu button, Tab closes. (←/→ to the neighbouring menu only apply if there are several menus.)
// - A click next to it closes; activating an entry closes the menu.
// - aria-expanded on the menu button.
// - Badges in the whole ribbon (tab Start: open proposals, Prüfen: „N offen“ findings, Buch: requests for the
//   outline, plus the entries „Befunde“ and „Anträge zur Gliederung“): the server draws them on every page
//   (workspace page from the template, otherwise menu::fill_badges); they are fetched anew through menu.badges on
//   opening the menu, on loading outside the workspace page and after forms or navigation (the numbers can change
//   there).

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { register } from "./widgets.js";

interface Badges {
  work: number;
  outline_requests: number;
  open_proposals: number;
  /// Round 28: open findings in the work (counting rule §1.5); missing with older servers.
  open_findings?: number;
}

/// Work of the ribbon (`data-work` on the header `header.band`, base.html); without a work `null`.
function topbarWork(nav: HTMLElement): number | null {
  const value = nav.closest(".band")?.getAttribute("data-work") ?? "";
  return /^\d+$/.test(value) ? Number(value) : null;
}

function showBadges(nav: HTMLElement, badges: Badges): void {
  const root = nav.closest(".band") ?? nav;
  for (const name of ["outline_requests", "open_proposals", "open_findings"] as const) {
    const count = badges[name] ?? 0;
    root.querySelectorAll<HTMLElement>(`[data-badge="${name}"]`).forEach((badge) => {
      // On the tab „k offen“, in the entry only the number (as the server draws it).
      badge.textContent = name === "open_findings" && badge.closest(".band-tab") ? t("widget.menu.open_count", { count }) : String(count);
      badge.hidden = count === 0;
    });
    root.querySelectorAll<HTMLElement>(`[data-badge-item="${name}"]`).forEach((item) => {
      item.hidden = count === 0;
    });
  }
}

register("menubar", (nav) => {
  const menus = Array.from(nav.querySelectorAll<HTMLDetailsElement>("details.menu"));
  if (menus.length === 0) return;
  const summaryOf = (menu: HTMLDetailsElement) => menu.querySelector<HTMLElement>(":scope > summary");
  const itemsOf = (menu: HTMLDetailsElement) =>
    Array.from(menu.querySelectorAll<HTMLElement>(".menu-item")).filter(
      // Round 35: for Lektors konten.css hides the entries of the author; the arrow keys skip them.
      (item) => !item.hidden && !(document.body.dataset.rolle === "lektor" && item.classList.contains("nur-autor")),
    );

  // Entries are not in the tab order (Tab jumps from menu to menu), arrows lead into it.
  for (const menu of menus) {
    for (const item of menu.querySelectorAll<HTMLElement>(".menu-item")) item.tabIndex = -1;
    summaryOf(menu)?.setAttribute("aria-expanded", menu.open ? "true" : "false");
  }

  const close = (menu: HTMLDetailsElement, focus = false) => {
    if (menu.open) menu.open = false;
    if (focus) summaryOf(menu)?.focus();
  };
  const open = (menu: HTMLDetailsElement, which: "first" | "last" | "none") => {
    for (const other of menus) if (other !== menu) close(other);
    menu.open = true;
    const items = itemsOf(menu);
    const target = which === "first" ? items[0] : which === "last" ? items[items.length - 1] : undefined;
    target?.focus();
  };
  const neighbour = (menu: HTMLDetailsElement, step: number) =>
    menus[(menus.indexOf(menu) + step + menus.length) % menus.length];

  let work = topbarWork(nav);
  let pending = false;
  const refresh = async (): Promise<void> => {
    if (work === null || pending) return;
    pending = true;
    try {
      showBadges(nav, await call<Badges>("menu.badges", { work }));
    } catch {
      // Badges are only a hint; without an answer the previous state stays.
    } finally {
      pending = false;
    }
  };

  for (const menu of menus) {
    const summary = summaryOf(menu);
    menu.addEventListener("toggle", () => {
      summary?.setAttribute("aria-expanded", menu.open ? "true" : "false");
      if (!menu.open) return;
      for (const other of menus) if (other !== menu) close(other);
      void refresh();
    });
    summary?.addEventListener("keydown", (event) => {
      switch (event.key) {
        case "ArrowDown":
        case "Enter":
        case " ":
          event.preventDefault();
          open(menu, "first");
          break;
        case "ArrowUp":
          event.preventDefault();
          open(menu, "last");
          break;
        case "ArrowRight":
        case "ArrowLeft": {
          event.preventDefault();
          const next = neighbour(menu, event.key === "ArrowRight" ? 1 : -1);
          const wasOpen = menu.open;
          close(menu);
          if (wasOpen) open(next, "first");
          else summaryOf(next)?.focus();
          break;
        }
        case "Escape":
          if (menu.open) {
            event.preventDefault();
            close(menu, true);
          }
          break;
      }
    });
    menu.querySelector(".menu-list")?.addEventListener("keydown", (event) => {
      const e = event as KeyboardEvent;
      const items = itemsOf(menu);
      const at = items.indexOf(document.activeElement as HTMLElement);
      const focus = (index: number) => items[(index + items.length) % items.length]?.focus();
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          focus(at + 1);
          break;
        case "ArrowUp":
          e.preventDefault();
          focus(at < 0 ? items.length - 1 : at - 1);
          break;
        case "Home":
          e.preventDefault();
          focus(0);
          break;
        case "End":
          e.preventDefault();
          focus(items.length - 1);
          break;
        case "ArrowRight":
        case "ArrowLeft":
          e.preventDefault();
          close(menu);
          open(neighbour(menu, e.key === "ArrowRight" ? 1 : -1), "first");
          break;
        case "Escape":
          e.preventDefault();
          e.stopPropagation();
          close(menu, true);
          break;
        case "Tab":
          close(menu);
          break;
        case " ":
        case "Enter": {
          const item = items[at];
          if (!item) break;
          e.preventDefault();
          if (item.getAttribute("aria-disabled") === "true") break;
          item.click();
          break;
        }
      }
    });
    // An entry was activated (link, dialog, toggle): menu closed. Stubs stay open.
    menu.querySelector(".menu-list")?.addEventListener("click", (event) => {
      const item = (event.target as Element).closest<HTMLElement>(".menu-item");
      if (!item) return;
      if (item.getAttribute("aria-disabled") === "true") {
        event.preventDefault();
        return;
      }
      close(menu);
    });
  }

  // A click next to it closes.
  document.addEventListener("click", (event) => {
    const target = event.target as Node;
    for (const menu of menus) if (menu.open && !menu.contains(target)) close(menu);
  });
  // Focus leaves the menu bar (e.g. with Tab): close an open menu.
  nav.addEventListener("focusout", (event) => {
    const next = (event as FocusEvent).relatedTarget as Node | null;
    if (next && !nav.contains(next)) for (const menu of menus) close(menu);
  });

  // Badges: fetch at once outside the workspace page, otherwise after changes.
  if (!document.querySelector("[data-widget=workspace]")) void refresh();
  document.addEventListener("baruch:navigate", () => window.setTimeout(() => void refresh(), 300));
  document.addEventListener("submit", () => window.setTimeout(() => void refresh(), 800));
  window.addEventListener("popstate", () => {
    work = topbarWork(nav) ?? work;
    void refresh();
  });
});
