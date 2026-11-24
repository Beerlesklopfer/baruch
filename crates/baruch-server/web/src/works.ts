// SPDX-License-Identifier: AGPL-3.0-or-later
// Werke mit JavaScript. Ohne JavaScript geht alles über das GET-Formular der Werkwahl (Knopf
// „Öffnen“ in <noscript>), die Seiten /werke und /werke/neu und die Formulare an POST /werke/import
// bzw. /werke/act.
//
// - Widget „work-select“ (Auswahl in der Kopfzeile): Ein Wechsel öffnet das gewählte Werk sofort
//   (`location.assign`) und merkt es im Cookie `baruch_work`; der Server setzt den Cookie ohnehin
//   bei jeder Antwort der Seite `/`.
// - Widget „work-import“ (<dialog> in der Kopfzeile): Der Link „Importieren…“ (/werke/neu) öffnet
//   den Dialog mit denselben Formularen.
// - Widget „work-upload“ (Upload-Formular): belegt das Titelfeld aus `#set document(title: …)` (Markdown: `title:` im
//   YAML-Kopf, Runde 21b; ZIP: nichts) der
//   gewählten Datei vor. Der Server bleibt maßgeblich; ein leeres Feld heißt „Titel aus der Datei“.

import { leaveEditor, register, store } from "./widgets.js";

/// Titel aus dem ersten `#set document(… title: "…" | […] …)` außerhalb von Kommentaren; sonst `null`.
/// Grob wie `baruch_typst::parse` (Raw-Blöcke werden nicht beachtet); nur ein Vorschlag.
export function documentTitle(source: string): string | null {
  const code = source
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/(^|[^:])\/\/[^\n]*/g, "$1");
  const call = /#set\s+document\s*\(/.exec(code);
  if (!call) return null;
  const rest = code.slice(call.index + call[0].length);
  const quoted = /^[^)]*?\btitle\s*:\s*"((?:[^"\\]|\\.)*)"/.exec(rest);
  if (quoted) return quoted[1].replace(/\\(.)/g, "$1").trim() || null;
  const content = /^[^)]*?\btitle\s*:\s*\[([^\]]*)\]/.exec(rest);
  if (content) return content[1].trim() || null;
  return null;
}

/// Titel aus dem YAML-Kopf einer Markdown-Datei (`title: …` zwischen zwei `---`-Zeilen ganz am Anfang); sonst
/// `null`. Nur ein Vorschlag für das Titelfeld. Runde 23a: im Gleichlauf mit `markdown_import::convert`
/// (`front_matter`, belegt durch einen Test, der beide auf dieselben Dateien ansetzt): Zeilen, die mit Leerraum oder
/// `-` beginnen, zählen nicht; der Schlüssel darf Leerraum vor dem Doppelpunkt haben; **ein** Paar Anführungszeichen
/// außen fällt weg (doppelte, sonst einfache); ein leerer Wert zählt nicht; stehen mehrere `title:` im Kopf, gilt der
/// letzte mit Wert.
export function markdownTitle(source: string): string | null {
  const head = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/.exec(source);
  if (!head) return null;
  let title: string | null = null;
  for (const line of head[1].split(/\r?\n/)) {
    if (/^[ \t-]/.test(line)) continue;
    const colon = line.indexOf(":");
    if (colon < 0 || line.slice(0, colon).trim() !== "title") continue;
    let value = line.slice(colon + 1).trim();
    if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) value = value.slice(1, -1);
    else if (value.length >= 2 && value.startsWith("'") && value.endsWith("'")) value = value.slice(1, -1);
    value = value.trim();
    if (value !== "") title = value;
  }
  return title;
}

register("work-select", (element) => {
  if (!(element instanceof HTMLSelectElement)) return;
  const select = element;
  let shown = select.value;
  select.addEventListener("change", () => {
    const id = select.value;
    if (!/^\d+$/.test(id) || id === shown) return;
    if (!leaveEditor()) {
      select.value = shown;
      return;
    }
    shown = id;
    store.set("work", id);
    window.location.assign(`/?work=${id}`);
  });
});

register("work-import", (element) => {
  if (!(element instanceof HTMLDialogElement)) return;
  const dialog = element;
  document.querySelectorAll<HTMLAnchorElement>("a[data-work-import-open]").forEach((link) => {
    link.addEventListener("click", (event) => {
      if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      dialog.showModal();
      dialog.querySelector<HTMLInputElement>("input[type=file]")?.focus();
    });
  });
});

register("work-upload", (form) => {
  const file = form.querySelector<HTMLInputElement>("input[type=file][name=file]");
  const title = form.querySelector<HTMLInputElement>("input[name=title][data-from-file]");
  if (!file || !title) return;
  // Selbst eingetippte Titel bleiben; nur ein leeres oder von hier vorbelegtes Feld wird gefüllt.
  title.addEventListener("input", () => {
    delete title.dataset.filled;
  });
  file.addEventListener("change", async () => {
    const chosen = file.files?.[0];
    if (!chosen || (title.value !== "" && title.dataset.filled !== "1")) return;
    // Ein ZIP liest der Browser nicht aus: der Server nimmt den Titel aus der Haupt-Datei bzw. dem Archivnamen.
    if (/\.zip$/i.test(chosen.name)) return;
    try {
      const head = await chosen.slice(0, 256 * 1024).text();
      const found = /\.(md|markdown)$/i.test(chosen.name) ? markdownTitle(head) : documentTitle(head);
      title.value = found ?? "";
      title.dataset.filled = "1";
      title.placeholder = found === null ? "aus dem Dateinamen" : "";
    } catch {
      // Lesen im Browser gescheitert: Der Server nimmt den Titel aus der Datei.
    }
  });
});
