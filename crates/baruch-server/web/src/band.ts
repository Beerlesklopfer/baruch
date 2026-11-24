// SPDX-License-Identifier: AGPL-3.0-or-later
// Menüband (Runde 33): Widgets „band“ (<header class="band"> in templates/base.html) und „band-dock“ (Marke am Ende
// von fragments/editor.html). Ohne JavaScript ist das Band schon vollständig bedienbar: Die Reiter sind Links
// (?tab=…), die Gruppen <details>, den Zustand setzt der Server ein (src/band.rs). Hier kommt dazu:
// - Reiter nach dem ARIA-Muster für Tabs: Klick wechselt ohne Neuladen, ←/→ (mit Umlauf), Pos1/Ende wandern und
//   wählen, nur der gewählte Reiter steht in der Tab-Folge.
// - Band auf/zu: Klick auf den gewählten Reiter oder auf den Schalter rechts klappt das Band zu (Attribut `data-band`
//   an <html>, Cookie `baruch_band`). Im zugeklappten Band zeigt ein Reiter sein Feld vorübergehend über dem Inhalt
//   (`data-band-peek` am Kopf); Esc, ein Klick daneben oder das Auslösen eines Eintrags blendet es wieder aus.
// - Gruppen: Im vollen Band sind alle offen und ihre Beschriftung ist kein Schalter. Im kompakten Band
//   (`data-compact` an <html>, gesetzt vom Server bzw. von laptop.ts) sind sie Popups: nur eines offen, Esc und ein
//   Klick daneben schließen, ein ausgelöster Knopf oder Link schließt sein Popup. Das Modul misst nie Breiten; auf
//   einen Wechsel reagiert es über das Ereignis `baruch:laptop` (laptop.ts).
// - Andocken: Die Knöpfe des Editors (`[data-edit-toolbar]`, fragments/edit_toolbar.html) stehen im Fragment des
//   Editors und werden mit ihm neu gezeichnet (`ui.render`). „band-dock“ leert bei jedem Einhängen den Andockplatz
//   (`div[data-band-dock]`) und hängt die Leiste des gezeigten Editors hinein (Vorschau: nur leeren). Welcher Teil
//   sichtbar ist (`data-band-part`), regelt band.css über `header.band[data-tab]`. Die Widgets der Leiste (wysiwyg,
//   ribbon, context-menu) haben sich vorher eingehängt und behalten ihre Verweise auf den Editor.
// - „Prüfen“/„Vorschlag“ der Gruppe Assistenten (`[data-band-assist]`) lösen den gleichnamigen Knopf des Formulars
//   im Akkordeon „Assistenten“ aus (Persona, Hinweis, Bereich und Prüfgrundlage gelten von dort).

import { toolbarTarget } from "./ribbon.js";
import { register, store, submitButton } from "./widgets.js";

/// Cookie `baruch_band` (über `store`): `closed`, fehlt = offen.
const COOKIE = "band";

function compact(): boolean {
  return document.documentElement.hasAttribute("data-compact");
}

function hint(message: string): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level: "info" } }));
}

/// Gruppen unter `scope` nach `data-compact` an <html>: voll alle offen (Beschriftung ohne Fokus), kompakt als
/// Popups mit gemeinsamem Namen (der Browser hält dann nur eines offen); beim Wechsel nach kompakt zuklappen.
export function syncGroups(scope: ParentNode): void {
  const popups = compact();
  scope.querySelectorAll<HTMLDetailsElement>("details[data-band-group]").forEach((group) => {
    const label = group.querySelector<HTMLElement>("summary");
    if (popups) {
      if (!group.hasAttribute("name")) group.open = false;
      group.setAttribute("name", "band-group");
      if (label) label.tabIndex = 0;
    } else {
      group.removeAttribute("name");
      group.open = true;
      if (label) label.tabIndex = -1;
    }
  });
}

register("band", (head) => {
  const html = document.documentElement;
  const tabs = () => Array.from(head.querySelectorAll<HTMLAnchorElement>("[data-band-tab]"));
  const panels = () => Array.from(head.querySelectorAll<HTMLElement>("[data-band-panel]"));
  const groups = () => Array.from(head.querySelectorAll<HTMLDetailsElement>("details[data-band-group]"));
  const toggle = head.querySelector<HTMLElement>("[data-band-toggle]");
  const closed = () => html.getAttribute("data-band") === "closed";
  const peeking = () => head.hasAttribute("data-band-peek");
  const peek = (on: boolean) => {
    head.toggleAttribute("data-band-peek", on);
  };

  /// Offene Popups schließen (nur kompakt); `true`, wenn eines offen war.
  const closePopups = (focus = false): boolean => {
    if (!compact()) return false;
    let any = false;
    for (const group of groups()) {
      if (!group.open) continue;
      group.open = false;
      any = true;
      if (focus) group.querySelector<HTMLElement>("summary")?.focus();
    }
    return any;
  };

  const select = (id: string, focus = false) => {
    head.setAttribute("data-tab", id);
    for (const tab of tabs()) {
      const on = tab.dataset.bandTab === id;
      tab.setAttribute("aria-selected", String(on));
      tab.tabIndex = on ? 0 : -1;
      if (on && focus) tab.focus();
    }
    for (const panel of panels()) panel.hidden = panel.dataset.bandPanel !== id;
    closePopups();
  };

  const setClosed = (now: boolean) => {
    html.setAttribute("data-band", now ? "closed" : "open");
    store.set(COOKIE, now ? "closed" : null);
    toggle?.setAttribute("aria-expanded", String(!now));
    peek(false);
  };

  // Ausgangszustand vom Server (data-tab am Kopf); fehlt er, gilt die Vorgabe der Seite bzw. der erste Reiter.
  const known = tabs().map((tab) => tab.dataset.bandTab ?? "");
  const wanted = [head.getAttribute("data-tab"), head.querySelector("[data-band-default]")?.getAttribute("data-band-default")];
  const first = wanted.find((id) => id && known.includes(id)) ?? known[0];
  if (first) select(first);
  toggle?.setAttribute("aria-expanded", String(!closed()));
  syncGroups(head);

  head.addEventListener("click", (event) => {
    const e = event as MouseEvent;
    const target = event.target as Element | null;
    if (!target?.closest) return;
    const plain = e.button === 0 && !e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey;

    const tab = target.closest<HTMLElement>("[data-band-tab]");
    if (tab) {
      // Mit Strg/Umschalt bleibt es ein Link (neuer Reiter des Browsers).
      if (!plain) return;
      event.preventDefault();
      const id = tab.dataset.bandTab ?? "";
      const active = head.getAttribute("data-tab") === id;
      if (closed()) {
        // Zugeklappt: Feld vorübergehend zeigen; derselbe Reiter noch einmal blendet es wieder aus.
        const again = peeking() && active;
        select(id);
        peek(!again);
      } else if (active) {
        setClosed(true);
      } else {
        select(id);
      }
      return;
    }

    if (target.closest("[data-band-toggle]")) {
      if (!plain) return;
      event.preventDefault();
      setClosed(!closed());
      return;
    }

    const label = target.closest<HTMLElement>("summary");
    const labelled = label?.parentElement?.hasAttribute("data-band-group") ? label.parentElement : null;
    if (labelled) {
      if (!compact()) {
        // Volles Band: Die Beschriftung unter der Gruppe ist kein Schalter.
        event.preventDefault();
        return;
      }
      for (const other of groups()) if (other !== labelled && other.open) other.open = false;
      return;
    }

    const assist = target.closest<HTMLElement>("[data-band-assist]");
    if (assist) {
      const method = assist.dataset.bandAssist ?? "";
      const button = Array.from(document.querySelectorAll<HTMLButtonElement>("form.assist-form button")).find(
        (b) => b.getAttribute("name") === "method" && b.getAttribute("value") === method,
      );
      if (button) button.click();
      else hint("Keine Persona ist aktiv – einrichten unter Einstellungen → Assistenten.");
    }

    // „Speichern“ im Band gehört über form="editor-form" zum Formular; maßgeblich bleibt der Knopf der Statuszeile:
    // Ist er gesperrt (nichts geändert), speichert auch der Knopf im Band nicht.
    const save = target.closest<HTMLButtonElement>("[data-band-save]");
    if (save) {
      const form = document.getElementById("editor-form");
      const main = form instanceof HTMLFormElement ? submitButton(form) : null;
      if (main && main !== save && main.disabled) {
        event.preventDefault();
        hint("Nichts zu speichern – der Text ist unverändert.");
      }
    }

    // Ein Knopf oder Link wurde ausgelöst: sein Popup (kompakt) und ein vorübergehend gezeigtes Feld schließen.
    // Auswahllisten und Felder bleiben offen; „Register“ klappt seine Liste im Popup auf.
    const item = target.closest<HTMLElement>("a, button");
    if (!item || item.hasAttribute("data-index-toggle")) return;
    window.setTimeout(() => {
      closePopups();
      peek(false);
    }, 0);
  });

  // Die Beschriftungen der angedockten Gruppen nehmen dem Satzbild nicht die Auswahl (wie die Knöpfe der Leiste).
  head.addEventListener("mousedown", (event) => {
    const label = (event.target as Element | null)?.closest?.("summary");
    if (label?.closest("[data-band-dock]")) event.preventDefault();
  });

  head.addEventListener("keydown", (event) => {
    const e = event as KeyboardEvent;
    const target = event.target as Element | null;
    const tab = target?.closest?.<HTMLElement>("[data-band-tab]");
    if (tab) {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      if (e.key === " ") {
        e.preventDefault();
        tab.click();
        return;
      }
      const list = tabs();
      const to = toolbarTarget(e.key, list.indexOf(tab as HTMLAnchorElement), list.length);
      if (to === null) return;
      e.preventDefault();
      select(list[to].dataset.bandTab ?? "", true);
      if (closed()) peek(true);
      return;
    }
    // Volles Band: Enter/Leertaste auf der Beschriftung klappt die Gruppe nicht zu.
    const label = target?.closest?.("summary");
    if (label?.parentElement?.hasAttribute("data-band-group") && !compact() && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
    }
  });

  // Klick daneben: Popup und vorübergehend gezeigtes Feld schließen.
  document.addEventListener("click", (event) => {
    const target = event.target as Element | null;
    if (target?.closest?.("header.band")) return;
    closePopups();
    peek(false);
  });
  // Esc: erst ein offenes Popup (Fokus zurück auf seine Beschriftung), sonst das vorübergehend gezeigte Feld.
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (closePopups(true)) return;
    if (!peeking()) return;
    peek(false);
    tabs().find((tab) => tab.getAttribute("aria-selected") === "true")?.focus();
  });
  // laptop.ts meldet einen Wechsel der Breitenklasse bzw. des Modus: Gruppen neu als Popups oder offen.
  document.addEventListener("baruch:laptop", () => syncGroups(head));
});

register("band-dock", (mark) => {
  const dock = document.querySelector<HTMLElement>("[data-band-dock]");
  // Seite ohne Band (z. B. ein einzeln eingebettetes Fragment): Die Leiste bleibt am Blatt.
  if (!dock) return;
  const scope = mark.closest<HTMLElement>("#pane-editor") ?? mark.parentElement;
  const bar = scope?.querySelector<HTMLElement>("[data-edit-toolbar]") ?? null;
  dock.replaceChildren();
  if (bar) dock.append(bar);
  const head = dock.closest<HTMLElement>("header.band");
  head?.toggleAttribute("data-docked", bar !== null);
  syncGroups(dock);
});
