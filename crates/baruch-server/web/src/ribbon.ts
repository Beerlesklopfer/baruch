// SPDX-License-Identifier: AGPL-3.0-or-later
// Menüband für das Bearbeiten (Runde 18a): Widget „ribbon“ auf `span.ribbon-formats` in
// templates/fragments/edit_toolbar.html. Die Knöpfe `data-ribbon-cmd` (Formate der Werkvorlage aus crate::ribbon)
// wirken je nach Ansicht:
// - Satzbild offen (`div.wysiwyg[data-ws-open]`): Befehl über `baruch:ws-command` (ws_command.ts) an das Satzbild.
//   Führt es ihn nicht aus (`handled` bleibt `false`), sagt ein Hinweis ehrlich, dass das Format im Satzbild noch
//   nicht geht und im Quelltext einzufügen ist – es wird nichts still verschluckt.
// - Quelltext: Das Widget fügt den Typst-Text `data-ribbon-snippet` selbst an der Cursorstelle ins Textfeld ein
//   (Blöcke an einem Zeilenanfang; eine Auswahl wandert in den Inhalt), Cursor in den Inhalt. Ein Zitat (Runde 23a)
//   steht mit genau einer Leerzeile davor und danach, darunter eine Zeile „– “ für die Stellenangabe. Das Einfügen löst
//   `input` aus wie Tippen, damit Zeitwächter und „ungespeichert“ es sehen.
// - „Bild“ (`data-ribbon-image`) öffnet nur `details[data-image-insert]` in der Statuszeile (Formular von
//   satzbild-werk) und setzt den Fokus hinein.
// Das Widget ändert wysiwyg.ts nicht und importiert es nicht (Vertrag R18 §4).
// Runde 23a: Pfeiltasten im Menüband (←/→ mit Umlauf, Pos1/Ende) nach dem ARIA-Muster für `role="toolbar"`.
// Runde 19b: „Bibelstelle“ (`scripture`) schickt im Satzbild den Funktionsnamen der Vorlage aus dem Snippet mit
// (`arg` = JSON `{fn}`), „Literaturverweis“ (`cite`) fügt im Quelltext `@` ein; das Satzbild fragt selbst nach Stelle
// bzw. Schlüssel (ws_command.ts).

import { register } from "./widgets.js";
import { scriptureFunction, sendWsCommand, type WsCommandName } from "./ws_command.js";

const COMMANDS: readonly WsCommandName[] = [
  "scene",
  "work",
  "scripture",
  "cite",
  "footnote",
  "quote",
  "list",
  "enum",
  "terms",
  "break",
  "link",
  "xref",
];
/// Befehle, deren Typst-Text ein eigener Block ist (beginnt an einem Zeilenanfang).
const BLOCKS: readonly WsCommandName[] = ["scene", "work", "quote", "list", "enum", "terms"];
/// Runde 23a (Befund des Nutzers): Befehle, deren Block genau eine Leerzeile davor und danach bekommt. Beim Zitat
/// klebte sonst der Block am Absatz davor und die Stellenangabe der nächsten Zeile am Block.
const SPACED: readonly WsCommandName[] = ["quote"];

function isCommand(value: string | undefined): value is WsCommandName {
  return value !== undefined && (COMMANDS as readonly string[]).includes(value);
}

/// Stelle des Cursors im eingefügten Text: in den Inhalt (`[⏎|⏎]`, `[|]`), hinter das Listenzeichen, sonst ans Ende.
export function caretIn(snippet: string, cmd: WsCommandName): number {
  if (cmd === "break") return snippet.length;
  const block = snippet.indexOf("[\n\n]");
  if (block >= 0) return block + 2;
  const inline = snippet.indexOf("[]");
  if (inline >= 0) return inline + 1;
  if (snippet.endsWith("\n")) return snippet.length - 1;
  return snippet.length;
}

/// Text zum Einfügen und Cursor darin: `before` = Text vor der Cursorstelle, `selected` = markierter Text, `after` =
/// Text hinter der Auswahl (Runde 23a; nur für die Leerzeile hinter einem Zitat). Ein Zitat bekommt genau eine
/// Leerzeile davor und danach: vorhandene Zeilenenden zählen mit (keine doppelte Leerzeile), am Anfang des Texts
/// steht nichts davor, am Ende nur das Zeilenende.
export function insertion(
  snippet: string,
  cmd: WsCommandName,
  before: string,
  selected: string,
  after = "",
): { text: string; caret: number } {
  if (SPACED.includes(cmd)) {
    const core = snippet.replace(/\n+$/, "");
    const at = caretIn(core, cmd);
    const prefix = before === "" || before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n";
    const suffix = after === "" ? "\n" : after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n";
    const text = prefix + core.slice(0, at) + selected + core.slice(at) + suffix;
    return { text, caret: prefix.length + at + selected.length };
  }
  const at = caretIn(snippet, cmd);
  const prefix = BLOCKS.includes(cmd) && before !== "" && !before.endsWith("\n") ? "\n" : "";
  const text = prefix + snippet.slice(0, at) + selected + snippet.slice(at);
  return { text, caret: prefix.length + at + selected.length };
}

/// Runde 23a (Gutachten R18): Ziel der Pfeiltasten in einer Werkzeugleiste (`role="toolbar"`, ARIA-Muster): ←/→
/// wandern mit Umlauf, Pos1/Ende springen an den Rand. `index` = Stelle des Elements mit Fokus unter `count`
/// bedienbaren Elementen; `null`, wenn die Taste nichts bewegt.
export function toolbarTarget(key: string, index: number, count: number): number | null {
  if (count <= 0 || index < 0) return null;
  switch (key) {
    case "ArrowRight":
      return (index + 1) % count;
    case "ArrowLeft":
      return (index - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

/// Pfeiltasten im Menüband: Fokus wandert zwischen den sichtbaren, nicht gesperrten Knöpfen und Links. In einer
/// Auswahlliste gehören die Pfeile der Liste; Tab bleibt wie bisher.
function toolbarKeys(bar: HTMLElement): void {
  if (bar.dataset.toolbarKeys === "1") return;
  bar.dataset.toolbarKeys = "1";
  bar.addEventListener("keydown", (event) => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const from = event.target instanceof HTMLElement ? event.target : null;
    if (!from || from instanceof HTMLSelectElement || from instanceof HTMLInputElement) return;
    const items = Array.from(bar.querySelectorAll<HTMLElement>("button, a[href], select")).filter(
      (item) => !(item as HTMLButtonElement).disabled && !item.closest("[hidden]"),
    );
    const to = toolbarTarget(event.key, items.indexOf(from), items.length);
    if (to === null) return;
    event.preventDefault();
    items[to]?.focus();
  });
}

function hint(message: string): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level: "info" } }));
}

/// Typst-Text ins Textfeld an der Cursorstelle (wie Tippen: Rückgängig bleibt möglich, `input` wird ausgelöst).
function insertIntoArea(area: HTMLTextAreaElement, snippet: string, cmd: WsCommandName): void {
  const start = area.selectionStart ?? area.value.length;
  const end = area.selectionEnd ?? start;
  const { text, caret } = insertion(
    snippet,
    cmd,
    area.value.slice(0, start),
    area.value.slice(start, end),
    area.value.slice(end),
  );
  area.focus();
  area.setSelectionRange(start, end);
  let done = false;
  try {
    done = document.execCommand("insertText", false, text);
  } catch {
    done = false;
  }
  if (!done) {
    area.setRangeText(text, start, end, "end");
    area.dispatchEvent(new Event("input", { bubbles: true }));
  }
  area.setSelectionRange(start + caret, start + caret);
}

register("ribbon", (group) => {
  const root = group.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
  if (!root) return;
  const bar = group.closest<HTMLElement>("[role=toolbar]");
  if (bar) toolbarKeys(bar);
  group.querySelectorAll<HTMLButtonElement>("[data-ribbon-cmd]").forEach((button) => {
    // Die Auswahl im Satzbild bzw. Textfeld bleibt beim Klick erhalten.
    button.addEventListener("mousedown", (event) => event.preventDefault());
    button.addEventListener("click", () => {
      const cmd = button.dataset.ribbonCmd;
      if (!isCommand(cmd)) return;
      const label = button.textContent?.trim() ?? cmd;
      // Runde 30a: „Verweis“ öffnet den Dialog (xref.ts), der im Satzbild bzw. Quelltext einfügt.
      if (cmd === "xref") {
        root.dispatchEvent(new CustomEvent("baruch:xref-open", { bubbles: true }));
        return;
      }
      if (root.hasAttribute("data-ws-open")) {
        const fn = cmd === "scripture" ? scriptureFunction(button.dataset.ribbonSnippet ?? "") : null;
        const arg = fn ? JSON.stringify({ fn }) : undefined;
        if (!sendWsCommand(root, cmd, arg)) {
          hint(`„${label}“ geht im Satzbild noch nicht – bitte im Quelltext einfügen (Knopf „Quelltext“).`);
        }
        return;
      }
      const area = root.querySelector<HTMLTextAreaElement>("textarea[name=body]");
      if (!area || area.hidden) {
        hint(`„${label}“: kein Textfeld zum Einfügen gefunden.`);
        return;
      }
      insertIntoArea(area, button.dataset.ribbonSnippet ?? "", cmd);
    });
  });
  group.querySelector<HTMLButtonElement>("[data-ribbon-image]")?.addEventListener("click", () => {
    const details = root.querySelector<HTMLDetailsElement>("details[data-image-insert]");
    if (!details) {
      hint("„Bild“: Das Einfügen von Bildern ist hier nicht verfügbar.");
      return;
    }
    details.open = true;
    details.scrollIntoView({ block: "nearest" });
    const target = details.querySelector<HTMLElement>("select, input, a, button") ?? details.querySelector("summary");
    target?.focus();
  });
});
