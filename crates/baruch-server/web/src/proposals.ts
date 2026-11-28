// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „proposals“: open proposals at their place in the reading flow (fragments/proposals.html,
// Round 8). Every piece of a proposal is a <details class="pv" data-pv="<key>-<rev>">; without
// JavaScript the browser folds them open and closed itself. The widget remembers collapsed proposals
// in the cookie `baruch_folded` ('.'-separated `<key>-<rev>`, at most 200, only [0-9.-]); the server
// thereby draws them collapsed right away (also after ui.render). A proposal with several pieces
// folds as a whole: whoever folds one piece open or closed folds all pieces of the same proposal along,
// also those that wysiwyg.ts has put into the typeset view (therefore the widget listens on the document).
// Plus the button „Alle einklappen/aufklappen“ ([data-pv-all], hidden without JavaScript); in the editor it stands
// in the bar before the reading flow (`[data-pv-bar=<key>]`, Round 18c) and thus stays visible with the typeset view
// open.

import { t } from "./i18n.js";
import { register, store } from "./widgets.js";

const MAX_FOLDED = 200;
const ENTRY = /^\d+-\d+$/;
const ITEM = "details.pv[data-pv]";

function folded(): string[] {
  const value = store.get("folded") ?? "";
  if (!/^[0-9.-]*$/.test(value)) return [];
  return value.split(".").filter((entry) => ENTRY.test(entry));
}

function save(entries: string[]): void {
  const unique = Array.from(new Set(entries.filter((entry) => ENTRY.test(entry))));
  // The oldest drop out when there are too many.
  const kept = unique.slice(Math.max(0, unique.length - MAX_FOLDED));
  store.set("folded", kept.length ? kept.join(".") : null);
}

function remember(id: string, open: boolean): void {
  const rest = folded().filter((entry) => entry !== id);
  save(open ? rest : [...rest, id]);
}

/// Labels of the "all" buttons, anew after every folding.
const labels = new Set<() => void>();

// `toggle` does not bubble; therefore in the capture phase on the document (also pieces in the typeset view).
document.addEventListener(
  "toggle",
  (event) => {
    const item = event.target;
    if (!(item instanceof HTMLDetailsElement) || !item.matches(ITEM)) return;
    const id = item.dataset.pv ?? "";
    remember(id, item.open);
    // All pieces of the same proposal fold along (the same value triggers no further toggle).
    document.querySelectorAll<HTMLDetailsElement>(ITEM).forEach((other) => {
      if (other !== item && other.dataset.pv === id && other.open !== item.open) other.open = item.open;
    });
    labels.forEach((label) => label());
  },
  true,
);

register("proposals", (root) => {
  // Pieces of this reading flow; in the editor also those that the typeset view currently shows.
  const items = () => {
    const own = Array.from(root.querySelectorAll<HTMLDetailsElement>(ITEM));
    const key = root.dataset.key;
    const placed = key
      ? Array.from(document.querySelectorAll<HTMLDetailsElement>(`[data-ws-sheet] > ${ITEM}`)).filter((item) =>
          (item.dataset.pv ?? "").startsWith(`${key}-`),
        )
      : [];
    return [...own, ...placed];
  };
  // In the editor the bar stands before the reading flow (div.pv-bar-editor[data-pv-bar=<key>], Round 18c) so that it
  // stays visible with the typeset view open; in the preview it stands in the section itself.
  const key = root.dataset.key ?? "";
  const bar = Array.from(root.parentElement?.children ?? []).find(
    (child): child is HTMLElement => child instanceof HTMLElement && child.dataset.pvBar === key,
  );
  const all =
    root.querySelector<HTMLButtonElement>("[data-pv-all]") ?? bar?.querySelector<HTMLButtonElement>("[data-pv-all]");
  if (!all) return;
  const label = () => {
    if (!all.isConnected) {
      labels.delete(label);
      return;
    }
    all.textContent = items().some((item) => item.open) ? t("widget.proposals.collapse_all") : t("widget.proposals.expand_all");
  };
  labels.add(label);
  all.hidden = false;
  label();
  all.addEventListener("click", () => {
    const open = !items().some((item) => item.open);
    const ids = items().map((item) => item.dataset.pv ?? "");
    const rest = folded().filter((entry) => !ids.includes(entry));
    save(open ? rest : [...rest, ...ids]);
    for (const item of items()) item.open = open;
    label();
  });
});
