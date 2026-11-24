// SPDX-License-Identifier: AGPL-3.0-or-later
// Widgets der einzelnen Bereiche: Spaltengrenzen, Thema, Editor, Gliederung, Akkordeon (Chat: chat.ts).

import { convertLineAt, convertText } from "./markdown.js";
import { register, store, submitButton } from "./widgets.js";

// ---- splitter: Spaltenbreite ziehen (Maus) oder mit ←/→ verstellen (Tastatur) ------------
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
// Kleiner Umschalter in der Kopfzeile (base.html, Runde 9): Zeichen aus data-auto/-light/-dark, der
// Name des Themas steht im title. Die Einstellungen (Seite „Darstellung“) setzen das Cookie
// baruch_theme selbst und melden `baruch:settings-changed`; dann wird es hier neu gelesen.
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

// ---- editor: Speichern erst nach einer Änderung; Strg+S speichert ------------------------
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
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      if (!save.disabled) form.requestSubmit();
    }
    // Markdown-Überschrift (`# Titel`) wird beim Abschluss der Zeile mit Enter zur Typst-Überschrift;
    // danach fügt der Browser den Zeilenumbruch ein (Regel und Ausnahmen in markdown.ts).
    if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && !event.isComposing) {
      if (convertLineAt(area)) update();
    }
  });
  // Beim Verlassen des Textfelds alle Zeilen umwandeln (wie der Server beim Speichern).
  area.addEventListener("blur", () => {
    const converted = convertText(area.value);
    if (converted !== area.value) {
      area.value = converted;
      area.dispatchEvent(new Event("input", { bubbles: true }));
    }
  });
  update();
});

// ---- tree: zugeklappte Teile und Kapitel merken (der Server zeichnet sie zugeklappt) ------
register("tree", (tree) => {
  tree.querySelectorAll<HTMLDetailsElement>("details[data-node]").forEach((details) => {
    details.addEventListener("toggle", () => {
      const id = details.dataset.node ?? "";
      const closed = (store.get("closed") ?? "").split(".").filter((x) => x && x !== id);
      if (!details.open) closed.push(id);
      store.set("closed", closed.length ? closed.join(".") : null);
    });
  });
  // Klick auf den Eintrag wählt aus (workspace verhindert dabei das Klappen); der Pfeil klappt.
});

// ---- accordion: immer ein Abschnitt offen, Wahl merken -----------------------------------
// `<details name>` schließt die anderen selbst; ältere Browser bekommen es hier nachgebaut.
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
        // Nie alle zu: der zuletzt geschlossene bleibt offen.
        section.open = true;
      }
    });
  }
});

// ---- chat: siehe chat.ts (ans Ende rollen, neue Beiträge seit dem letzten Besuch) ----------
