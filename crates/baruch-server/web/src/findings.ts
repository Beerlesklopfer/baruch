// SPDX-License-Identifier: AGPL-3.0-or-later
// Befunde (Runde 28, befund-uebersicht): Widget „findings“ für das Akkordeon „Befunde“ der Arbeitsseite
// (fragments/findings_panel.html) und die Seite /werke/<id>/befunde (templates/befunde.html). Ohne JavaScript bleibt
// alles über Links, GET-Filter und Formulare an POST /werke/befunde/act bedienbar; hier kommt dazu:
// - Seite: Filter ohne Neuladen – Absenden des Filters, Ändern eines Kästchens bzw. einer Auswahl, Klick auf eine Zelle
//   der Matrix oder auf „Lob einblenden“ holt dieselbe Adresse per fetch und tauscht nur den Inhalt
//   ([data-findings-main]); die Adresse wandert in den Verlauf (Zurück funktioniert).
// - Tastatur in der Matrix: Pfeiltasten wandern zwischen den Zellen (auch leeren), Pos1/Ende an den Zeilenrand,
//   Enter öffnet die Zelle (gewöhnlicher Link).
// - Statusformulare ([data-finding-form]): über RPC finding.set_status bzw. finding.decide; danach zeichnet die
//   Arbeitsseite ihre Bereiche neu (Ereignis baruch:navigate) bzw. lädt die Seite ihren Inhalt neu. Fehler stehen
//   als Hinweis am Formular, der Entwurf bleibt stehen.
// - Runde 22: Die Rückfrage einer Persona „Befund n scheint erledigt?“ („Ja, erledigt“ / „Nein, noch offen“) besteht aus
//   denselben Formularen (finding.decide mit confirm); in der Befundkarte eines Chat-Beitrags trägt der Kasten der
//   Rückfrage selbst `data-widget="findings"`, damit auch dort ohne Neuladen entschieden wird.

import { call } from "./rpc.js";
import { mount, register } from "./widgets.js";

const NUMERIC = new Set(["work", "id", "entry", "rev", "target_key", "target_idea"]);

/// Parameter eines Statusformulars: leere Felder fallen weg, Zahlen als Zahlen, `confirm` als Wahrheitswert;
/// `method` und `back` gehören nicht zur RPC.
function formParams(form: HTMLFormElement): { method: string; params: Record<string, unknown> } {
  const data = new FormData(form);
  const method = String(data.get("method") ?? "");
  const params: Record<string, unknown> = {};
  for (const [key, raw] of data) {
    const value = String(raw);
    if (key === "method" || key === "back" || value.trim() === "") continue;
    if (key === "confirm") params.confirm = value === "1" || value === "true";
    else if (NUMERIC.has(key)) params[key] = Number(value);
    else params[key] = value;
  }
  // finding.decide braucht nur `entry` (die id des Befunds steht nur für die Seite ohne JavaScript im Formular).
  if (method === "finding.decide") delete params.id;
  return { method, params };
}

/// Hinweis am Formular (role=alert); ein früherer wird ersetzt.
function showError(form: HTMLFormElement, message: string): void {
  form.querySelector(".fd-error")?.remove();
  const note = document.createElement("p");
  note.className = "notice error fd-error small";
  note.setAttribute("role", "alert");
  note.textContent = message;
  form.append(note);
}

/// Inhalt der Seite neu holen und tauschen; `push` legt die Adresse in den Verlauf.
async function reload(root: HTMLElement, url: string, push: boolean): Promise<void> {
  const main = root.querySelector<HTMLElement>("[data-findings-main]");
  if (!main) {
    window.location.assign(url);
    return;
  }
  root.setAttribute("aria-busy", "true");
  try {
    const response = await fetch(url, { headers: { Accept: "text/html" } });
    if (!response.ok) throw new Error(`Server antwortet ${response.status}`);
    const doc = new DOMParser().parseFromString(await response.text(), "text/html");
    const fresh = doc.querySelector<HTMLElement>("[data-findings-main]");
    if (!fresh) throw new Error("Antwort ohne Befunde");
    main.innerHTML = fresh.innerHTML;
    mount(main);
    if (push) history.pushState({ findings: true }, "", url);
    const hash = new URL(url, window.location.href).hash;
    if (hash) main.querySelector<HTMLElement>(hash)?.scrollIntoView({ block: "start" });
  } catch {
    // Ohne Antwort wie ohne JavaScript: die Seite selbst laden.
    window.location.assign(url);
  } finally {
    root.removeAttribute("aria-busy");
  }
}

/// Adresse eines GET-Formulars mit seinen Feldern.
function formUrl(form: HTMLFormElement): string {
  const url = new URL(form.action, window.location.href);
  const query = new URLSearchParams();
  for (const [key, value] of new FormData(form)) {
    if (String(value).trim() !== "") query.append(key, String(value));
  }
  url.search = query.toString();
  return url.pathname + url.search;
}

/// Pfeiltasten in der Matrix: Fokus wandert zwischen den Zellen (Links bzw. Zellen mit tabindex).
function matrixKeys(table: HTMLTableElement): void {
  const rows = () => Array.from(table.querySelectorAll<HTMLTableRowElement>("tr.fd-row"));
  const target = (cell: HTMLTableCellElement): HTMLElement => {
    const link = cell.querySelector<HTMLElement>("a");
    if (link) return link;
    cell.tabIndex = -1;
    return cell;
  };
  table.addEventListener("keydown", (event) => {
    const cell = (event.target as Element).closest<HTMLTableCellElement>("td.fd-cell, th.fd-row-head");
    const row = cell?.closest<HTMLTableRowElement>("tr.fd-row");
    if (!cell || !row) return;
    const all = rows();
    const r = all.indexOf(row);
    const c = Array.from(row.cells).indexOf(cell);
    let next: HTMLTableCellElement | undefined;
    switch (event.key) {
      case "ArrowRight":
        next = row.cells[c + 1];
        break;
      case "ArrowLeft":
        next = row.cells[c - 1];
        break;
      case "ArrowDown":
        next = all[r + 1]?.cells[c];
        break;
      case "ArrowUp":
        next = all[r - 1]?.cells[c];
        break;
      case "Home":
        next = row.cells[0];
        break;
      case "End":
        next = row.cells[row.cells.length - 1];
        break;
      default:
        return;
    }
    if (!next) return;
    event.preventDefault();
    target(next).focus();
  });
}

register("findings", (root) => {
  const page = root.hasAttribute("data-findings-page");

  // Statusformulare über RPC (Akkordeon und Seite).
  root.addEventListener("submit", (event) => {
    const form = (event.target as Element).closest<HTMLFormElement>("form[data-finding-form]");
    if (form) {
      event.preventDefault();
      const { method, params } = formParams(form);
      const button = form.querySelector<HTMLButtonElement>("button[type=submit]");
      if (button) button.disabled = true;
      void call(method, params)
        .then(() => {
          if (page) return reload(root, window.location.pathname + window.location.search, false);
          // Arbeitsseite: Bereiche neu zeichnen (Abzeichen, Akkordeon, Fassungsbaum).
          document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url: window.location.href } }));
          return undefined;
        })
        .catch((error: unknown) => showError(form, String((error as Error).message ?? error)))
        .finally(() => {
          if (button) button.disabled = false;
        });
      return;
    }
    const filter = (event.target as Element).closest<HTMLFormElement>("form[data-findings-filter]");
    if (filter && page) {
      event.preventDefault();
      void reload(root, formUrl(filter), true);
    }
  });

  if (!page) return;

  // Filter ändern wirkt sofort (Kästchen, Auswahl); Textfelder wirken beim Absenden.
  root.addEventListener("change", (event) => {
    const field = event.target as HTMLInputElement | HTMLSelectElement;
    const filter = field.closest<HTMLFormElement>("form[data-findings-filter]");
    if (!filter || (field instanceof HTMLInputElement && !["checkbox", "date"].includes(field.type))) return;
    if (field instanceof HTMLInputElement || field instanceof HTMLSelectElement) void reload(root, formUrl(filter), true);
  });

  // Zellen, „Lob einblenden“, „Filter aufheben“ und „Zurück zur ganzen Matrix“ ohne Neuladen.
  root.addEventListener("click", (event) => {
    const mouse = event as MouseEvent;
    if (mouse.button !== 0 || mouse.ctrlKey || mouse.metaKey || mouse.shiftKey) return;
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[href]");
    if (!link) return;
    const url = new URL(link.href, window.location.href);
    if (url.origin !== window.location.origin || url.pathname !== window.location.pathname) return;
    event.preventDefault();
    void reload(root, url.pathname + url.search + url.hash, true);
  });

  window.addEventListener("popstate", () => void reload(root, window.location.pathname + window.location.search, false));

  const table = root.querySelector<HTMLTableElement>("table[data-findings-matrix]");
  if (table) matrixKeys(table);
  // Nach dem Tausch des Inhalts gibt es eine neue Tabelle: Tastatur dort wieder einhängen.
  new MutationObserver(() => {
    const fresh = root.querySelector<HTMLTableElement>("table[data-findings-matrix]");
    if (fresh && !fresh.dataset.keys) {
      fresh.dataset.keys = "1";
      matrixKeys(fresh);
    }
  }).observe(root, { childList: true, subtree: true });
  if (table) table.dataset.keys = "1";
});
