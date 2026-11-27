// SPDX-License-Identifier: AGPL-3.0-or-later
// Ribbon for editing (Round 18a): widget „ribbon“ on `div.ribbon-formats` in
// templates/fragments/edit_toolbar.html. The buttons `data-ribbon-cmd` (formats of the work template from
// crate::ribbon) act depending on the view:
// - Typeset view open (`div.wysiwyg[data-ws-open]`): command through `baruch:ws-command` (ws_command.ts) to the
//   typeset view. If it does not carry it out (`handled` stays `false`), a notice says honestly that the format does
//   not work in the typeset view yet and is to be inserted in the source text – nothing is swallowed silently.
// - Source text: the widget inserts the Typst text `data-ribbon-snippet` itself at the cursor position into the
//   text field (blocks at a line start; a selection moves into the content), cursor into the content. A quotation
//   (Round 23a) stands with exactly one empty line before and after, below it a line „– “ for the reference. The
//   insertion triggers `input` like typing so that the time guard and "unsaved" see it.
// - „Bild“ (`data-ribbon-image`) only opens `details[data-image-insert]` in the status line (form of
//   satzbild-werk) and puts the focus into it.
// The widget does not change wysiwyg.ts and does not import it (contract R18 §4).
// Round 33: the bar has two parts (`data-band-part`) and docks into the ribbon with JavaScript (band.ts); the
// widget sits on `div.ribbon-formats` in the part „einfuegen“ and remembers its editor when mounting (before
// docking) – afterwards the bar no longer lies in `div.wysiwyg`.
// Round 23a: arrow keys in the ribbon (←/→ wrapping, Home/End) following the ARIA pattern for `role="toolbar"`.
// Round 19b: „Bibelstelle“ (`scripture`) sends along, in the typeset view, the function name of the template from the
// snippet (`arg` = JSON `{fn}`), „Literaturverweis“ (`cite`) inserts `@` in the source text; the typeset view asks
// for passage or key itself (ws_command.ts).

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
/// Commands whose Typst text is a block of its own (begins at a line start).
const BLOCKS: readonly WsCommandName[] = ["scene", "work", "quote", "list", "enum", "terms"];
/// Round 23a (finding of the user): commands whose block gets exactly one empty line before and after. With the
/// quotation the block otherwise stuck to the paragraph before it and the reference of the next line to the block.
const SPACED: readonly WsCommandName[] = ["quote"];

function isCommand(value: string | undefined): value is WsCommandName {
  return value !== undefined && (COMMANDS as readonly string[]).includes(value);
}

/// Position of the cursor in the inserted text: into the content (`[⏎|⏎]`, `[|]`), behind the list sign, otherwise
/// to the end.
export function caretIn(snippet: string, cmd: WsCommandName): number {
  if (cmd === "break") return snippet.length;
  const block = snippet.indexOf("[\n\n]");
  if (block >= 0) return block + 2;
  const inline = snippet.indexOf("[]");
  if (inline >= 0) return inline + 1;
  if (snippet.endsWith("\n")) return snippet.length - 1;
  return snippet.length;
}

/// Text to insert and cursor in it: `before` = text before the cursor position, `selected` = selected text, `after`
/// = text behind the selection (Round 23a; only for the empty line behind a quotation). A quotation gets exactly one
/// empty line before and after: existing line ends count (no double empty line), at the start of the text
/// nothing stands before it, at the end only the line end.
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

/// Round 23a (review R18): target of the arrow keys in a toolbar (`role="toolbar"`, ARIA pattern): ←/→
/// move with wrapping, Home/End jump to the edge. `index` = position of the element with focus among `count`
/// operable elements; `null` if the key moves nothing.
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

/// Round 33: the bar stands docked in the ribbon (band.ts). Reachable there is only the part of the selected tab
/// (`data-band-part` against `header.band[data-tab]`; band.css hides the other), in the compact ribbon only the
/// label of a collapsed group, in the full ribbon never the label (it is not a switch there).
function reachable(item: HTMLElement): boolean {
  const head = item.closest("header.band");
  const part = item.closest("[data-band-part]");
  if (head && part && part.getAttribute("data-band-part") !== head.getAttribute("data-tab")) return false;
  const group = item.closest("details");
  if (!group || !group.hasAttribute("data-band-group")) return true;
  const popup = group.hasAttribute("name");
  if (item.localName === "summary") return popup;
  return !popup || group.hasAttribute("open");
}

/// Arrow keys in the ribbon: the focus moves between the visible, not disabled buttons and links. In a
/// choice list the arrows belong to the list; Tab stays as before.
function toolbarKeys(bar: HTMLElement): void {
  if (bar.dataset.toolbarKeys === "1") return;
  bar.dataset.toolbarKeys = "1";
  bar.addEventListener("keydown", (event) => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const from = event.target instanceof HTMLElement ? event.target : null;
    if (!from || from instanceof HTMLSelectElement || from instanceof HTMLInputElement) return;
    const items = Array.from(bar.querySelectorAll<HTMLElement>("button, a[href], select, summary")).filter(
      (item) => !(item as HTMLButtonElement).disabled && !item.closest("[hidden]") && reachable(item),
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

/// Typst text into the text field at the cursor position (like typing: undo stays possible, `input` is triggered).
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
    // The selection in the typeset view or text field is kept on the click.
    button.addEventListener("mousedown", (event) => event.preventDefault());
    button.addEventListener("click", () => {
      const cmd = button.dataset.ribbonCmd;
      if (!isCommand(cmd)) return;
      const label = button.textContent?.trim() ?? cmd;
      // Round 30a: „Verweis“ opens the dialog (xref.ts), which inserts in the typeset view or source text.
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
