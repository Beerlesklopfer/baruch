// SPDX-License-Identifier: AGPL-3.0-or-later
// Findings (Round 28, befund-uebersicht): widget „findings“ for the accordion „Befunde“ of the workspace page
// (fragments/findings_panel.html) and the page /werke/<id>/befunde (templates/befunde.html). Without JavaScript
// everything stays usable through links, GET filters and forms to POST /werke/befunde/act; added here:
// - Page: filters without reloading – submitting the filter, changing a checkbox or a choice, a click on a cell
//   of the matrix or on „Lob einblenden“ fetches the same address by fetch and swaps only the content
//   ([data-findings-main]); the address moves into the history (back works).
// - Keyboard in the matrix: arrow keys move between the cells (also empty ones), Home/End to the edge of the row,
//   Enter opens the cell (ordinary link).
// - Status forms ([data-finding-form]): through RPC finding.set_status or finding.decide; afterwards the
//   workspace page redraws its areas (event baruch:navigate) or the page reloads its content. Errors stand
//   as a notice at the form, the draft stays.
// - Round 22: the query of a persona „Befund n scheint erledigt?“ („Ja, erledigt“ / „Nein, noch offen“) consists of
//   the same forms (finding.decide with confirm); in the finding card of a chat post the box of the
//   query itself carries `data-widget="findings"` so that the decision works without reloading there as well.

import { call } from "./rpc.js";
import { mount, register } from "./widgets.js";

const NUMERIC = new Set(["work", "id", "entry", "rev", "target_key", "target_idea"]);

/// Parameters of a status form: empty fields drop out, numbers as numbers, `confirm` as a boolean;
/// `method` and `back` do not belong to the RPC.
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
  // finding.decide only needs `entry` (the id of the finding stands in the form only for the page without JavaScript).
  if (method === "finding.decide") delete params.id;
  return { method, params };
}

/// Notice at the form (role=alert); an earlier one is replaced.
function showError(form: HTMLFormElement, message: string): void {
  form.querySelector(".fd-error")?.remove();
  const note = document.createElement("p");
  note.className = "notice error fd-error small";
  note.setAttribute("role", "alert");
  note.textContent = message;
  form.append(note);
}

/// Fetch the content of the page anew and swap it; `push` puts the address into the history.
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
    // Without an answer as without JavaScript: load the page itself.
    window.location.assign(url);
  } finally {
    root.removeAttribute("aria-busy");
  }
}

/// Address of a GET form with its fields.
function formUrl(form: HTMLFormElement): string {
  const url = new URL(form.action, window.location.href);
  const query = new URLSearchParams();
  for (const [key, value] of new FormData(form)) {
    if (String(value).trim() !== "") query.append(key, String(value));
  }
  url.search = query.toString();
  return url.pathname + url.search;
}

/// Arrow keys in the matrix: the focus moves between the cells (links or cells with tabindex).
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

  // Status forms through RPC (accordion and page).
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
          // Workspace page: redraw the areas (badges, accordion, revision tree).
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

  // Changing the filter takes effect at once (checkbox, choice); text fields take effect on submitting.
  root.addEventListener("change", (event) => {
    const field = event.target as HTMLInputElement | HTMLSelectElement;
    const filter = field.closest<HTMLFormElement>("form[data-findings-filter]");
    if (!filter || (field instanceof HTMLInputElement && !["checkbox", "date"].includes(field.type))) return;
    if (field instanceof HTMLInputElement || field instanceof HTMLSelectElement) void reload(root, formUrl(filter), true);
  });

  // Cells, „Lob einblenden“, „Filter aufheben“ and „Zurück zur ganzen Matrix“ without reloading.
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
  // After swapping the content there is a new table: attach the keyboard there again.
  new MutationObserver(() => {
    const fresh = root.querySelector<HTMLTableElement>("table[data-findings-matrix]");
    if (fresh && !fresh.dataset.keys) {
      fresh.dataset.keys = "1";
      matrixKeys(fresh);
    }
  }).observe(root, { childList: true, subtree: true });
  if (table) table.dataset.keys = "1";
});
