// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „proposals“: offene Vorschläge an ihrer Stelle im Lesefluss (fragments/proposals.html,
// Runde 8). Jedes Stück eines Vorschlags ist ein <details class="pv" data-pv="<key>-<rev>">; ohne
// JavaScript klappt der Browser sie selbst auf und zu. Das Widget merkt sich eingeklappte Vorschläge
// im Cookie `baruch_folded` ('.'-getrennte `<key>-<rev>`, höchstens 200, nur [0-9.-]); der Server
// zeichnet sie damit gleich eingeklappt (auch nach ui.render). Ein Vorschlag mit mehreren Stücken
// klappt als Ganzes: Wer ein Stück auf- oder zuklappt, klappt alle Stücke desselben Vorschlags mit,
// auch die, die wysiwyg.ts ins Satzbild gesetzt hat (darum hört das Widget am Dokument).
// Dazu die Schaltfläche „Alle einklappen/aufklappen“ ([data-pv-all], ohne JavaScript verborgen); im Editor steht
// sie in der Leiste vor dem Lesefluss (`[data-pv-bar=<key>]`, Runde 18c) und bleibt so bei offenem Satzbild zu sehen.

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
  // Die ältesten fallen weg, wenn es zu viele werden.
  const kept = unique.slice(Math.max(0, unique.length - MAX_FOLDED));
  store.set("folded", kept.length ? kept.join(".") : null);
}

function remember(id: string, open: boolean): void {
  const rest = folded().filter((entry) => entry !== id);
  save(open ? rest : [...rest, id]);
}

/// Beschriftungen der „Alle“-Knöpfe, neu nach jedem Klappen.
const labels = new Set<() => void>();

// `toggle` steigt nicht auf; darum in der Einfangphase am Dokument (auch Stücke im Satzbild).
document.addEventListener(
  "toggle",
  (event) => {
    const item = event.target;
    if (!(item instanceof HTMLDetailsElement) || !item.matches(ITEM)) return;
    const id = item.dataset.pv ?? "";
    remember(id, item.open);
    // Alle Stücke desselben Vorschlags klappen mit (gleicher Wert löst kein weiteres toggle aus).
    document.querySelectorAll<HTMLDetailsElement>(ITEM).forEach((other) => {
      if (other !== item && other.dataset.pv === id && other.open !== item.open) other.open = item.open;
    });
    labels.forEach((label) => label());
  },
  true,
);

register("proposals", (root) => {
  // Stücke dieses Leseflusses; im Editor auch die, die das Satzbild gerade zeigt.
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
  // Im Editor steht die Leiste vor dem Lesefluss (div.pv-bar-editor[data-pv-bar=<key>], Runde 18c), damit sie bei
  // offenem Satzbild sichtbar bleibt; in der Vorschau steht sie im Abschnitt selbst.
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
    all.textContent = items().some((item) => item.open) ? "Alle einklappen" : "Alle aufklappen";
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
