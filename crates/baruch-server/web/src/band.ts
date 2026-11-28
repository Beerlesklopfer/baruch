// SPDX-License-Identifier: AGPL-3.0-or-later
// Ribbon (Round 33): widgets „band“ (<header class="band"> in templates/base.html) and „band-dock“ (marker at the end
// of fragments/editor.html). Without JavaScript the ribbon is already fully usable: the tabs are links
// (?tab=…), the groups <details>, the server puts in the state (src/band.rs). Added here:
// - Tabs following the ARIA pattern for tabs: a click switches without reloading, ←/→ (wrapping), Home/End move and
//   select, only the selected tab is in the tab order.
// - Ribbon open/closed: a click on the selected tab or on the switch on the right collapses the ribbon (attribute
//   `data-band` on <html>, cookie `baruch_band`). In the collapsed ribbon a tab shows its panel temporarily above the
//   content (`data-band-peek` on the header); Esc, a click next to it or triggering an entry hides it again.
// - Groups: in the full ribbon all are open and their label is not a switch. In the compact ribbon
//   (`data-compact` on <html>, set by the server or by laptop.ts) they are popups: only one open, Esc and a
//   click next to it close, a triggered button or link closes its popup. The module never measures widths; it
//   reacts to a change through the event `baruch:laptop` (laptop.ts).
// - Docking: the buttons of the editor (`[data-edit-toolbar]`, fragments/edit_toolbar.html) stand in the fragment of
//   the editor and are redrawn with it (`ui.render`). „band-dock“ empties the docking place on every mount
//   (`div[data-band-dock]`) and hooks the bar of the shown editor into it (preview: only empty). Which part
//   is visible (`data-band-part`) is governed by band.css through `header.band[data-tab]`. The widgets of the bar
//   (wysiwyg, ribbon, context-menu) have mounted before and keep their references to the editor.
// - „Prüfen“/„Vorschlag“ of the group Assistenten (`[data-band-assist]`) trigger the button of the same name of the
//   form in the accordion „Assistenten“ (persona, note, scope and review basis apply from there).

import { t } from "./i18n.js";
import { toolbarTarget } from "./ribbon.js";
import { register, store, submitButton } from "./widgets.js";

/// Cookie `baruch_band` (through `store`): `closed`, missing = open.
const COOKIE = "band";

function compact(): boolean {
  return document.documentElement.hasAttribute("data-compact");
}

function hint(message: string): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level: "info" } }));
}

/// Groups below `scope` by `data-compact` on <html>: full all open (label without focus), compact as
/// popups with a shared name (the browser then keeps only one open); collapse when changing to compact.
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

  /// Close open popups (compact only); `true` if one was open.
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

  // Initial state from the server (data-tab on the header); if it is missing, the default of the page or the first tab
  // applies.
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
      // With Ctrl/Shift it stays a link (new tab of the browser).
      if (!plain) return;
      event.preventDefault();
      const id = tab.dataset.bandTab ?? "";
      const active = head.getAttribute("data-tab") === id;
      if (closed()) {
        // Collapsed: show the panel temporarily; the same tab once more hides it again.
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
        // Full ribbon: the label below the group is not a switch.
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
      else hint(t("widget.band.no_persona"));
    }

    // „Speichern“ in the ribbon belongs to the form through form="editor-form"; the button of the status line stays
    // authoritative: if it is disabled (nothing changed), the button in the ribbon does not save either.
    const save = target.closest<HTMLButtonElement>("[data-band-save]");
    if (save) {
      const form = document.getElementById("editor-form");
      const main = form instanceof HTMLFormElement ? submitButton(form) : null;
      if (main && main !== save && main.disabled) {
        event.preventDefault();
        hint(t("widget.band.nothing_to_save"));
      }
    }

    // A button or link was triggered: close its popup (compact) and a temporarily shown panel.
    // Choice lists and fields stay open; „Register“ unfolds its list in the popup.
    const item = target.closest<HTMLElement>("a, button");
    if (!item || item.hasAttribute("data-index-toggle")) return;
    window.setTimeout(() => {
      closePopups();
      peek(false);
    }, 0);
  });

  // The labels of the docked groups do not take the selection away from the typeset view (like the buttons of the bar).
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
    // Full ribbon: Enter/Space on the label does not collapse the group.
    const label = target?.closest?.("summary");
    if (label?.parentElement?.hasAttribute("data-band-group") && !compact() && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
    }
  });

  // Click next to it: close the popup and a temporarily shown panel.
  document.addEventListener("click", (event) => {
    const target = event.target as Element | null;
    if (target?.closest?.("header.band")) return;
    closePopups();
    peek(false);
  });
  // Esc: first an open popup (focus back to its label), otherwise the temporarily shown panel.
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (closePopups(true)) return;
    if (!peeking()) return;
    peek(false);
    tabs().find((tab) => tab.getAttribute("aria-selected") === "true")?.focus();
  });
  // laptop.ts reports a change of the width class or of the mode: groups anew as popups or open.
  document.addEventListener("baruch:laptop", () => syncGroups(head));
});

register("band-dock", (mark) => {
  const dock = document.querySelector<HTMLElement>("[data-band-dock]");
  // Page without a ribbon (e.g. a fragment embedded on its own): the bar stays at the sheet.
  if (!dock) return;
  const scope = mark.closest<HTMLElement>("#pane-editor") ?? mark.parentElement;
  const bar = scope?.querySelector<HTMLElement>("[data-edit-toolbar]") ?? null;
  dock.replaceChildren();
  if (bar) dock.append(bar);
  const head = dock.closest<HTMLElement>("header.band");
  head?.toggleAttribute("data-docked", bar !== null);
  syncGroups(dock);
});
