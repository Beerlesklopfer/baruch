// SPDX-License-Identifier: AGPL-3.0-or-later
// Place in the editor across saving (wish 2026-11-27: „Strg+S als normales Speichern“). Saving
// redraws the editor (`ui.render`); as in a word processor one should afterwards continue writing where one
// was: same cursor position or selection, same scroll position, focus in the text again – in the typeset view as in
// the source text. `capturePlace` reads the place when the editor is first locked (widgets.ts `lockEditor`), before
// anything is redrawn; `restorePlace` sets it after the redraw if the same section (key)
// stands in the editor again. The place counts characters of the visible text; if the text is shorter afterwards, it
// ends at the end.

export type Place = {
  key: string;
  sheet: boolean;
  start: number | null;
  end: number | null;
  scroll: number;
  areaScroll: number;
};

function editorForm(): HTMLFormElement | null {
  return document.getElementById("pane-editor")?.querySelector<HTMLFormElement>("form[data-widget=editor]") ?? null;
}

function openSheet(form: HTMLFormElement): HTMLElement | null {
  const root = form.closest<HTMLElement>("div.wysiwyg");
  const sheet = form.querySelector<HTMLElement>("[data-ws-sheet]");
  return root?.hasAttribute("data-ws-open") && sheet ? sheet : null;
}

/// Characters from the start of the sheet to the place (`node`, `offset`).
function offsetIn(sheet: HTMLElement, node: Node, offset: number): number {
  const range = document.createRange();
  range.setStart(sheet, 0);
  range.setEnd(node, offset);
  return range.toString().length;
}

/// Reverse of `offsetIn`: text node and offset for the character count (clamped at the end).
function pointAt(sheet: HTMLElement, chars: number): [Node, number] {
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT);
  let rest = chars;
  let last: Text | null = null;
  for (let node = walker.nextNode() as Text | null; node; node = walker.nextNode() as Text | null) {
    const length = node.data.length;
    if (rest <= length) return [node, rest];
    rest -= length;
    last = node;
  }
  return last ? [last, last.data.length] : [sheet, sheet.childNodes.length];
}

export function capturePlace(): Place | null {
  const form = editorForm();
  const key = form?.querySelector<HTMLInputElement>("input[name=key]")?.value;
  const area = form?.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  if (!form || !key || !area) return null;
  const body = form.closest<HTMLElement>(".editor-body");
  const sheet = openSheet(form);
  let start: number | null = null;
  let end: number | null = null;
  if (sheet) {
    const selection = document.getSelection();
    const range = selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
    if (range && sheet.contains(range.startContainer) && sheet.contains(range.endContainer)) {
      start = offsetIn(sheet, range.startContainer, range.startOffset);
      end = offsetIn(sheet, range.endContainer, range.endOffset);
    }
  } else {
    start = area.selectionStart;
    end = area.selectionEnd;
  }
  return { key, sheet: !!sheet, start, end, scroll: body?.scrollTop ?? 0, areaScroll: area.scrollTop };
}

/// `true` if the place was set (the same section stands in the editor again).
export function restorePlace(place: Place): boolean {
  const form = editorForm();
  const key = form?.querySelector<HTMLInputElement>("input[name=key]")?.value;
  const area = form?.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  if (!form || !area || key !== place.key) return false;
  const body = form.closest<HTMLElement>(".editor-body");
  const sheet = openSheet(form);
  if (sheet && place.sheet) {
    if (place.start !== null && place.end !== null) {
      sheet.focus({ preventScroll: true });
      const [startNode, startOffset] = pointAt(sheet, place.start);
      const [endNode, endOffset] = pointAt(sheet, place.end);
      const range = document.createRange();
      range.setStart(startNode, startOffset);
      range.setEnd(endNode, endOffset);
      const selection = document.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
  } else if (!sheet && !place.sheet) {
    area.focus({ preventScroll: true });
    if (place.start !== null && place.end !== null) {
      const length = area.value.length;
      area.setSelectionRange(Math.min(place.start, length), Math.min(place.end, length));
    }
    area.scrollTop = place.areaScroll;
  } else {
    // View changed (typeset view ↔ source text): only the scroll position no longer fits for sure; focus into the text.
    (sheet ?? area).focus({ preventScroll: true });
  }
  if (body) body.scrollTop = place.scroll;
  return true;
}
