// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „menubar“ (<nav class="menubar"> in der Kopfzeile, templates/base.html, Runde 9): die Menüs
// Werk ▾, Ansicht ▾ und Werkzeuge ▾ sind <details>/<summary> mit gewöhnlichen Links und klappen auch
// ohne JavaScript auf. Hier kommt dazu:
// - Tastatur nach dem ARIA-Menü-Muster: auf dem Menüknopf ↓/Enter/Leertaste öffnet und fokussiert den
//   ersten Eintrag, ↑ den letzten, ←/→ wechseln zum Nachbarmenü; in der Liste ↑/↓ (umlaufend),
//   Pos1/Ende, Enter/Leertaste aktiviert, Esc schließt und fokussiert den Menüknopf, Tab schließt.
// - Nur ein Menü offen; Klick daneben schließt; Aktivieren eines Eintrags schließt das Menü.
// - aria-expanded am Menüknopf.
// - Abzeichen (offene Anträge zur Gliederung, offene Vorschläge, seit Runde 28 offene Befunde): außerhalb der Arbeitsseite füllt
//   menu.badges sie nach, auf der Arbeitsseite zeichnet sie der Server; neu geholt wird beim Öffnen
//   eines Menüs und nach Formularen bzw. Navigation (die Zahlen können sich dort ändern).

import { call } from "./rpc.js";
import { register } from "./widgets.js";

interface Badges {
  work: number;
  outline_requests: number;
  open_proposals: number;
  /// Runde 28: offene Befunde im Werk (Zählregel §1.5); fehlt bei älteren Servern.
  open_findings?: number;
}

/// Werk der Kopfzeile aus dem Baruch-Link (`/?work=W`); ohne Werk `null`.
function topbarWork(nav: HTMLElement): number | null {
  const brand = nav.closest(".topbar")?.querySelector<HTMLAnchorElement>("a.brand");
  if (!brand) return null;
  const value = new URL(brand.href, window.location.href).searchParams.get("work");
  return value && /^\d+$/.test(value) ? Number(value) : null;
}

function showBadges(nav: HTMLElement, badges: Badges): void {
  const root = nav.closest(".topbar") ?? nav;
  for (const name of ["outline_requests", "open_proposals", "open_findings"] as const) {
    const count = badges[name] ?? 0;
    root.querySelectorAll<HTMLElement>(`[data-badge="${name}"]`).forEach((badge) => {
      // Am Menüknopf „k offen“, im Eintrag nur die Zahl (wie der Server zeichnet).
      badge.textContent = name === "open_findings" && badge.closest("summary") ? `${count} offen` : String(count);
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
    Array.from(menu.querySelectorAll<HTMLElement>(".menu-item")).filter((item) => !item.hidden);

  // Einträge sind nicht in der Tab-Folge (Tab springt von Menü zu Menü), Pfeile führen hinein.
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
      // Abzeichen sind nur ein Hinweis; ohne Antwort bleibt der bisherige Stand.
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
    // Ein Eintrag wurde aktiviert (Link, Dialog, Umschalter): Menü zu. Stubs bleiben offen.
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

  // Klick daneben schließt.
  document.addEventListener("click", (event) => {
    const target = event.target as Node;
    for (const menu of menus) if (menu.open && !menu.contains(target)) close(menu);
  });
  // Fokus verlässt die Menüleiste (z. B. mit Tab): offenes Menü schließen.
  nav.addEventListener("focusout", (event) => {
    const next = (event as FocusEvent).relatedTarget as Node | null;
    if (next && !nav.contains(next)) for (const menu of menus) close(menu);
  });

  // Abzeichen: außerhalb der Arbeitsseite sofort holen, sonst nach Änderungen.
  if (!document.querySelector("[data-widget=workspace]")) void refresh();
  document.addEventListener("baruch:navigate", () => window.setTimeout(() => void refresh(), 300));
  document.addEventListener("submit", () => window.setTimeout(() => void refresh(), 800));
  window.addEventListener("popstate", () => {
    work = topbarWork(nav) ?? work;
    void refresh();
  });
});
