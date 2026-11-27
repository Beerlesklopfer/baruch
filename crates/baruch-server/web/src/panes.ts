// SPDX-License-Identifier: AGPL-3.0-or-later
// Widgets of the individual areas: column borders, theme, editor, outline, accordion (chat: chat.ts).

import { convertLineAt, convertText } from "./markdown.js";
import { editorLocked, register, store, submitButton } from "./widgets.js";

// ---- splitter: drag the column width (mouse) or adjust it with ←/→ (keyboard) ------------
register("splitter", (bar) => {
  const side = bar.dataset.side === "right" ? "right" : "left";
  const variable = `--${side}`;
  const root = document.documentElement;
  const grid = bar.parentElement as HTMLElement;

  const set = (percent: number) => {
    const value = `${Math.min(45, Math.max(12, percent)).toFixed(1)}%`;
    root.style.setProperty(variable, value);
    store.set(side, value);
  };
  const current = () => {
    const pane = side === "left" ? grid.firstElementChild : grid.lastElementChild;
    return ((pane as HTMLElement).getBoundingClientRect().width / grid.clientWidth) * 100;
  };

  bar.addEventListener("pointerdown", (start) => {
    start.preventDefault();
    bar.setPointerCapture(start.pointerId);
    bar.classList.add("dragging");
    const box = grid.getBoundingClientRect();
    const move = (event: PointerEvent) => {
      const x = side === "left" ? event.clientX - box.left : box.right - event.clientX;
      set((x / box.width) * 100);
    };
    const stop = () => {
      bar.classList.remove("dragging");
      bar.removeEventListener("pointermove", move);
      bar.removeEventListener("pointerup", stop);
    };
    bar.addEventListener("pointermove", move);
    bar.addEventListener("pointerup", stop);
  });
  bar.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const wider = (event.key === "ArrowRight") === (side === "left");
    set(current() + (wider ? 2 : -2));
  });
});

// ---- theme: System → Hell → Dunkel ------------------------------------------------------
// Small toggle in the header (base.html, Round 9): signs from data-auto/-light/-dark, the
// name of the theme stands in the title. The settings (page „Darstellung“) set the cookie
// baruch_theme themselves and report `baruch:settings-changed`; then it is read anew here.
register("theme", (button) => {
  const modes = ["auto", "light", "dark"] as const;
  type Mode = (typeof modes)[number];
  const names: Record<Mode, string> = { auto: "System", light: "Hell", dark: "Dunkel" };
  const show = (mode: Mode) => {
    button.dataset.mode = mode;
    button.textContent = button.dataset[mode] ?? mode;
    button.title = `Thema: ${names[mode]}`;
    if (mode === "auto") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = mode;
  };
  const saved = (): Mode => {
    const value = store.get("theme") as Mode | null;
    return value && modes.includes(value) ? value : "auto";
  };
  show(saved());
  button.addEventListener("click", () => {
    const next = modes[(modes.indexOf(button.dataset.mode as Mode) + 1) % modes.length];
    store.set("theme", next === "auto" ? null : next);
    show(next);
  });
  document.addEventListener("baruch:settings-changed", () => show(saved()));
});

// ---- editor: saving only after a change; Ctrl+S saves ------------------------------------
// Wish 2026-11-27 („Strg+S als normales Speichern“): Ctrl+S (⌘+S) applies everywhere on the page with the editor,
// not only in the text field – also with the focus in the outline, in the ribbon or in the chat; the dialog "save
// page" of the browser never appears there. Without a change or while a save is running nothing happens. Afterwards
// the place in the text stays (place.ts through lockEditor). One listener on the document for all editors (also the
// typeset view).
document.addEventListener("keydown", (event) => {
  if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.key.toLowerCase() !== "s") return;
  if (event.defaultPrevented) return;
  const form = document.querySelector<HTMLFormElement>("form[data-widget=editor]");
  if (!form) return;
  event.preventDefault();
  const save = submitButton(form);
  if (save && !save.disabled && !editorLocked()) form.requestSubmit();
});

register("editor", (element) => {
  const form = element as HTMLFormElement;
  const area = form.querySelector("textarea");
  const save = submitButton(form);
  if (!area || !save) return;
  const original = area.value;
  const update = () => {
    const dirty = area.value !== original;
    form.dataset.dirty = dirty ? "1" : "";
    save.disabled = !dirty;
    save.textContent = (dirty ? save.dataset.dirty : save.dataset.saved) ?? save.textContent;
  };
  area.addEventListener("input", update);
  area.addEventListener("keydown", (event) => {
    // A Markdown heading (`# Titel`) becomes a Typst heading when the line is completed with Enter;
    // afterwards the browser inserts the line break (rule and exceptions in markdown.ts).
    if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && !event.isComposing) {
      if (convertLineAt(area)) update();
    }
  });
  // On leaving the text field convert all lines (like the server on saving).
  area.addEventListener("blur", () => {
    const converted = convertText(area.value);
    if (converted !== area.value) {
      area.value = converted;
      area.dispatchEvent(new Event("input", { bubbles: true }));
    }
  });
  update();
});

// ---- tree: remember collapsed parts and chapters (the server draws them collapsed) --------
register("tree", (tree) => {
  tree.querySelectorAll<HTMLDetailsElement>("details[data-node]").forEach((details) => {
    details.addEventListener("toggle", () => {
      const id = details.dataset.node ?? "";
      const closed = (store.get("closed") ?? "").split(".").filter((x) => x && x !== id);
      if (!details.open) closed.push(id);
      store.set("closed", closed.length ? closed.join(".") : null);
    });
  });
  // A click on the entry selects (workspace prevents the folding then); the arrow folds.
});

// ---- accordion: always one part open, remember the choice --------------------------------
// `<details name>` closes the others itself; older browsers get it rebuilt here.
register("accordion", (accordion) => {
  const sections = [...accordion.querySelectorAll<HTMLDetailsElement>(":scope > details[data-section]")];
  for (const section of sections) {
    section.addEventListener("toggle", () => {
      if (section.open) {
        sections.forEach((other) => { if (other !== section) other.open = false; });
        store.set("accordion", section.dataset.section ?? null);
        const chat = section.querySelector<HTMLElement>(".chat");
        if (chat) chat.scrollTop = chat.scrollHeight;
      } else if (!sections.some((s) => s.open)) {
        // Never all closed: the one closed last stays open.
        section.open = true;
      }
    });
  }
});

// ---- chat: see chat.ts (scroll to the end, new posts since the last visit) -----------------
