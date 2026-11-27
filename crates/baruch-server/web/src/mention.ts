// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „mention“ (Round 11): completion of „@…“ in the section chat.
//
// The server supplies at the input field `list="<id>"` a `<datalist>` with the active personas
// (`<option value="@handle" data-name data-role data-style>`); without JavaScript the browser suggests
// from it. With JavaScript a list of its own replaces the `<datalist>` (the attribute `list` is dropped
// so that not two lists open):
// - After „@“ (at the beginning or after whitespace) the list of the active personas appears (colour, name,
//   role in short); every further character filters by handle or name, ignoring case („@Na“ → @nathan).
// - Arrow down/up chooses, Tab or Enter accepts „@handle “ (with a space), Esc closes; a
//   click accepts likewise. Without an open list Enter stays the submitting of the form.
// - ARIA: input `role=combobox`, `aria-autocomplete=list`, `aria-expanded`, `aria-controls`,
//   `aria-activedescendant`; list `role=listbox` with `role=option`, `aria-selected`.
// Round 30c (chat-bedienung): the completion shows both forms of address. After „@“ each persona has the hint
// „auch: „Name, …“ am Satzanfang“; at the start of a sentence or post it suggests, after two letters, the address by
// name („Le“ → „Lektor, “), which calls like @handle (server: `assist::chat::addressed`). The address entries of the
// `<datalist>` (`data-address`, for the browser without JavaScript) do not count as personas here.

import { register } from "./widgets.js";

export interface Persona {
  handle: string;
  name: string;
  role: string;
  style: string;
}

/// Mention begun before the caret: start (position of the „@“) and what was typed after it; otherwise
/// `null` (no „@“ at the start of a word, or already a space behind it).
export function mentionAt(text: string, caret: number): { start: number; query: string } | null {
  const before = text.slice(0, caret);
  const match = /(^|\s)@([^\s@]*)$/u.exec(before);
  if (!match) return null;
  const start = before.length - match[2].length - 1;
  return { start, query: match[2] };
}

/// Personas whose handle (without „@“) or name starts with `query`, ignoring case; empty search = all.
export function filterPersonas(personas: Persona[], query: string): Persona[] {
  const q = query.toLocaleLowerCase("de");
  return personas.filter((p) => {
    const handle = p.handle.replace(/^@/, "").toLocaleLowerCase("de");
    return handle.startsWith(q) || p.name.toLocaleLowerCase("de").startsWith(q);
  });
}

/// Text after accepting: „@query“ from `start` becomes „@handle “; result and new caret.
export function applyMention(text: string, start: number, caret: number, handle: string): { text: string; caret: number } {
  const inserted = `${handle.startsWith("@") ? handle : `@${handle}`} `;
  const rest = text.slice(caret).replace(/^\s+/u, "");
  return { text: text.slice(0, start) + inserted + rest, caret: start + inserted.length };
}

/// Address begun before the caret (Round 30c): a word of letters at the start of a sentence or post (beginning,
/// after a line break or after „.“/„!“/„?“ with whitespace), at least two letters; otherwise `null`.
export function addressAt(text: string, caret: number): { start: number; query: string } | null {
  const before = text.slice(0, caret);
  const match = /(^|\n|[.!?]\s+)\s*([\p{L}]{2,})$/u.exec(before);
  if (!match) return null;
  const start = before.length - match[2].length;
  return { start, query: match[2] };
}

/// Personas whose name (or handle without „@“) starts with `query`, ignoring case.
export function filterAddresses(personas: Persona[], query: string): Persona[] {
  const q = query.toLocaleLowerCase("de");
  if (q.length < 2) return [];
  return personas.filter((p) => {
    const handle = p.handle.replace(/^@/, "").toLocaleLowerCase("de");
    return p.name.toLocaleLowerCase("de").startsWith(q) || handle.startsWith(q);
  });
}

/// Text after accepting an address: the word begun from `start` becomes „Name, “.
export function applyAddress(text: string, start: number, caret: number, name: string): { text: string; caret: number } {
  const inserted = `${name}, `;
  const rest = text.slice(caret).replace(/^[\s,:]+/u, "");
  return { text: text.slice(0, start) + inserted + rest, caret: start + inserted.length };
}

/// Role in short: first line, at most 60 characters.
function shortRole(role: string): string {
  const line = role.split("\n")[0].trim();
  return line.length > 60 ? `${line.slice(0, 59)}…` : line;
}

let counter = 0;

register("mention", (element) => {
  if (!(element instanceof HTMLInputElement)) return;
  const input = element;
  const listId = input.getAttribute("list");
  const source = listId ? document.getElementById(listId) : null;
  if (!(source instanceof HTMLDataListElement)) return;
  const personas: Persona[] = [...source.options].filter((o) => !o.dataset.address).map((o) => ({
    handle: o.value,
    name: o.dataset.name ?? o.value,
    role: o.dataset.role ?? "",
    style: o.dataset.style ?? "",
  }));
  if (personas.length === 0) return;
  input.removeAttribute("list");

  counter += 1;
  const boxId = `mention-list-${counter}`;
  const box = document.createElement("ul");
  box.id = boxId;
  box.className = "mention-list";
  box.setAttribute("role", "listbox");
  box.setAttribute("aria-label", "Personas");
  box.hidden = true;
  input.insertAdjacentElement("afterend", box);
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.setAttribute("aria-controls", boxId);
  input.parentElement?.classList.add("mention-host");

  let shown: Persona[] = [];
  let active = 0;
  let at: { start: number; query: string } | null = null;
  // Round 30c: „mention“ (after „@“) or „address“ (address by name at the start of a sentence).
  let form: "mention" | "address" = "mention";

  const close = () => {
    box.hidden = true;
    shown = [];
    at = null;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  };

  const paint = () => {
    box.replaceChildren(
      ...shown.map((p, i) => {
        const li = document.createElement("li");
        li.id = `${boxId}-${i}`;
        li.className = "mention-option pc";
        li.setAttribute("role", "option");
        li.setAttribute("aria-selected", i === active ? "true" : "false");
        if (p.style) li.setAttribute("style", p.style);
        const swatch = document.createElement("span");
        swatch.className = "pc-swatch";
        swatch.setAttribute("aria-hidden", "true");
        const name = document.createElement("strong");
        name.textContent = p.name;
        const handle = document.createElement("span");
        handle.className = "muted small";
        handle.textContent = ` ${p.handle}`;
        const role = document.createElement("span");
        role.className = "muted tiny mention-role";
        role.textContent = shortRole(p.role);
        if (form === "address") {
          // Address: „Lektor, …“ (calls like @lektor).
          name.textContent = `${p.name}, …`;
          handle.textContent = ` wie ${p.handle}`;
          li.append(swatch, name, handle, role);
        } else {
          const also = document.createElement("span");
          also.className = "muted tiny mention-also";
          also.textContent = p.handle === "@alle" ? "" : ` auch: „${p.name}, …“ am Satzanfang`;
          li.append(swatch, name, handle, also, role);
        }
        li.addEventListener("mousedown", (event) => {
          event.preventDefault(); // focus stays in the field
          accept(i);
        });
        return li;
      }),
    );
    input.setAttribute("aria-activedescendant", `${boxId}-${active}`);
  };

  const update = () => {
    const caret = input.selectionStart ?? input.value.length;
    at = mentionAt(input.value, caret);
    form = "mention";
    shown = at ? filterPersonas(personas, at.query) : [];
    if (!at) {
      at = addressAt(input.value, caret);
      form = "address";
      shown = at ? filterAddresses(personas.filter((p) => p.handle !== "@alle"), at.query) : [];
    }
    if (!at || shown.length === 0) {
      close();
      return;
    }
    active = Math.min(active, shown.length - 1);
    box.hidden = false;
    input.setAttribute("aria-expanded", "true");
    paint();
  };

  const accept = (index: number) => {
    const chosen = shown[index];
    if (!at || !chosen) return;
    const caret = input.selectionStart ?? input.value.length;
    const next =
      form === "address"
        ? applyAddress(input.value, at.start, caret, chosen.name)
        : applyMention(input.value, at.start, caret, chosen.handle);
    input.value = next.text;
    input.setSelectionRange(next.caret, next.caret);
    close();
    input.dispatchEvent(new Event("input", { bubbles: true }));
  };

  input.addEventListener("input", () => {
    active = 0;
    update();
  });
  input.addEventListener("click", update);
  input.addEventListener("blur", close);
  input.addEventListener("keydown", (event) => {
    if (box.hidden) {
      if (event.key === "ArrowDown" && mentionAt(input.value, input.selectionStart ?? 0)) {
        event.preventDefault();
        update();
      }
      return;
    }
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        active = (active + 1) % shown.length;
        paint();
        break;
      case "ArrowUp":
        event.preventDefault();
        active = (active - 1 + shown.length) % shown.length;
        paint();
        break;
      case "Enter":
      case "Tab":
        event.preventDefault();
        accept(active);
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        close();
        break;
      default:
        break;
    }
  });
});
