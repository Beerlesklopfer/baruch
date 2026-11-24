// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „mention“ (Runde 11): Vervollständigung von „@…“ im Abschnitts-Chat.
//
// Der Server liefert am Eingabefeld `list="<id>"` ein `<datalist>` mit den aktiven Personas
// (`<option value="@handle" data-name data-role data-style>`); ohne JavaScript schlägt der Browser
// daraus vor. Mit JavaScript ersetzt eine eigene Liste das `<datalist>` (das Attribut `list` entfällt,
// damit nicht zwei Listen aufgehen):
// - Nach „@“ (am Anfang oder nach Leerraum) erscheint die Liste der aktiven Personas (Farbe, Name,
//   Rolle kurz); jedes weitere Zeichen filtert nach Kürzel oder Name, ohne Groß/Klein („@Na“ → @nathan).
// - Pfeil runter/hoch wählt, Tab oder Enter übernimmt „@handle “ (mit Leerzeichen), Esc schließt; ein
//   Klick übernimmt ebenso. Ohne offene Liste bleibt Enter das Absenden des Formulars.
// - ARIA: Eingabe `role=combobox`, `aria-autocomplete=list`, `aria-expanded`, `aria-controls`,
//   `aria-activedescendant`; Liste `role=listbox` mit `role=option`, `aria-selected`.
// Runde 30c (chat-bedienung): Die Vervollständigung zeigt beide Formen der Anrede. Nach „@“ steht je Persona der Hinweis
// „auch: „Name, …“ am Satzanfang“; am Satz- bzw. Beitragsanfang schlägt sie nach zwei Buchstaben die Anrede mit Namen vor
// („Le“ → „Lektor, “), die wie @handle ruft (Server: `assist::chat::addressed`). Die Anrede-Einträge des `<datalist>`
// (`data-address`, für den Browser ohne JavaScript) zählen hier nicht als Personas.

import { register } from "./widgets.js";

export interface Persona {
  handle: string;
  name: string;
  role: string;
  style: string;
}

/// Angefangene Erwähnung vor der Schreibmarke: Anfang (Stelle des „@“) und das Getippte danach; sonst
/// `null` (kein „@“ am Wortanfang, oder schon ein Leerzeichen dahinter).
export function mentionAt(text: string, caret: number): { start: number; query: string } | null {
  const before = text.slice(0, caret);
  const match = /(^|\s)@([^\s@]*)$/u.exec(before);
  if (!match) return null;
  const start = before.length - match[2].length - 1;
  return { start, query: match[2] };
}

/// Personas, deren Kürzel (ohne „@“) oder Name mit `query` beginnt, ohne Groß/Klein; leere Suche = alle.
export function filterPersonas(personas: Persona[], query: string): Persona[] {
  const q = query.toLocaleLowerCase("de");
  return personas.filter((p) => {
    const handle = p.handle.replace(/^@/, "").toLocaleLowerCase("de");
    return handle.startsWith(q) || p.name.toLocaleLowerCase("de").startsWith(q);
  });
}

/// Text nach dem Übernehmen: „@query“ ab `start` wird „@handle “; Ergebnis und neue Schreibmarke.
export function applyMention(text: string, start: number, caret: number, handle: string): { text: string; caret: number } {
  const inserted = `${handle.startsWith("@") ? handle : `@${handle}`} `;
  const rest = text.slice(caret).replace(/^\s+/u, "");
  return { text: text.slice(0, start) + inserted + rest, caret: start + inserted.length };
}

/// Angefangene Anrede vor der Schreibmarke (Runde 30c): ein Wort aus Buchstaben am Satz- bzw. Beitragsanfang (Anfang,
/// nach Zeilenumbruch oder nach „.“/„!“/„?“ mit Leerraum), mindestens zwei Buchstaben; sonst `null`.
export function addressAt(text: string, caret: number): { start: number; query: string } | null {
  const before = text.slice(0, caret);
  const match = /(^|\n|[.!?]\s+)\s*([\p{L}]{2,})$/u.exec(before);
  if (!match) return null;
  const start = before.length - match[2].length;
  return { start, query: match[2] };
}

/// Personas, deren Name (oder Kürzel ohne „@“) mit `query` beginnt, ohne Groß/Klein.
export function filterAddresses(personas: Persona[], query: string): Persona[] {
  const q = query.toLocaleLowerCase("de");
  if (q.length < 2) return [];
  return personas.filter((p) => {
    const handle = p.handle.replace(/^@/, "").toLocaleLowerCase("de");
    return p.name.toLocaleLowerCase("de").startsWith(q) || handle.startsWith(q);
  });
}

/// Text nach dem Übernehmen einer Anrede: das angefangene Wort ab `start` wird „Name, “.
export function applyAddress(text: string, start: number, caret: number, name: string): { text: string; caret: number } {
  const inserted = `${name}, `;
  const rest = text.slice(caret).replace(/^[\s,:]+/u, "");
  return { text: text.slice(0, start) + inserted + rest, caret: start + inserted.length };
}

/// Rolle kurz: erste Zeile, höchstens 60 Zeichen.
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
  // Runde 30c: „mention“ (nach „@“) oder „address“ (Anrede mit Namen am Satzanfang).
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
          // Anrede: „Lektor, …“ (ruft wie @lektor).
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
          event.preventDefault(); // Fokus bleibt im Feld
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
