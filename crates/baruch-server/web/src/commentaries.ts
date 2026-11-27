// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „commentaries“ (Round 17c) for the commentary panel `<details class="commentaries">` from
// `bibles::commentary_panel_html` (Bible page /literatur/bibel). Without JavaScript the panel and, per module, a
// `<details class="commentary">` stay expandable and collapsible in the ordinary way. With JavaScript:
// - buttons „alle aufklappen“ / „alle zuklappen“ for the modules in the panel;
// - the state (panel open?, open per module?) stays remembered in localStorage (`baruch_commentaries`) – only in
//   try/catch, because localStorage can be missing or blocked; then simply the HTML of the server applies.
//   An explicit `open` from the server (link with kom=1) takes precedence.

import { register } from "./widgets.js";

const STORE = "baruch_commentaries";

interface Remembered {
  panel?: boolean;
  modules?: Record<string, boolean>;
}

function load(): Remembered {
  try {
    const raw = window.localStorage.getItem(STORE);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === "object" ? (parsed as Remembered) : {};
  } catch {
    return {};
  }
}

function save(state: Remembered): void {
  try {
    window.localStorage.setItem(STORE, JSON.stringify(state));
  } catch {
    // blocked or full: remember nothing
  }
}

register("commentaries", (root) => {
  if (!(root instanceof HTMLDetailsElement)) return;
  const modules = (): HTMLDetailsElement[] => [...root.querySelectorAll<HTMLDetailsElement>("details.commentary[data-commentary]")];
  const remembered = load();
  const serverOpened = root.open;
  if (!serverOpened && remembered.panel === true) root.open = true;
  for (const item of modules()) {
    const name = item.dataset.commentary ?? "";
    const known = remembered.modules?.[name];
    if (typeof known === "boolean") item.open = known;
  }

  const remember = (): void => {
    const state: Remembered = { panel: root.open, modules: { ...(load().modules ?? {}) } };
    for (const item of modules()) state.modules![item.dataset.commentary ?? ""] = item.open;
    save(state);
  };

  if (modules().length > 1) {
    const tools = document.createElement("div");
    tools.className = "commentaries-tools";
    const button = (label: string, open: boolean): HTMLButtonElement => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "btn small";
      b.textContent = label;
      b.addEventListener("click", () => {
        for (const item of modules()) item.open = open;
        remember();
      });
      return b;
    };
    tools.append(button("alle aufklappen", true), button("alle zuklappen", false));
    root.querySelector(":scope > summary")?.after(tools);
  }
  root.addEventListener("toggle", remember);
  for (const item of modules()) item.addEventListener("toggle", remember);
});
