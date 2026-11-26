// SPDX-License-Identifier: AGPL-3.0-or-later
// Runde 23a: Verhaltenstests der Widgets im Node-Nachbau (kein Browser). Dieser Text steht hinter dem DOM-Nachbau aus
// tests/round18_satzbild.rs (`DOM`) und ergänzt, was die hier geprüften Widgets zusätzlich brauchen: Formulardaten,
// Dialog, DOMParser, Verlauf, Auswahl im Textfeld. Die Szene wählt `scenario.scene`; beobachtet wird nur Verhalten
// (Aufrufe, Ereignisse, DOM), nie der Quelltext der Widgets. Ausgabe: genau ein JSON auf stdout.

// ---- Ergänzungen des Nachbaus -------------------------------------------------------------------------------------
Node_.TEXT_NODE = 3;
Node_.ELEMENT_NODE = 1;
document.activeElement = null;
HTMLElement.prototype.focus = function () { document.activeElement = this; };
HTMLElement.prototype.replaceChildren = function (...nodes) { for (const c of [...this.childNodes]) c.remove(); this.append(...nodes); };
HTMLElement.prototype.click = function () {
  this.dispatchEvent(Object.assign(new Event("click", { bubbles: true, cancelable: true }), { button: 0 }));
};
for (const [name, fallback] of [["name", ""], ["title", ""]]) {
  Object.defineProperty(HTMLElement.prototype, name, {
    get() { return this.getAttribute(name) ?? fallback; },
    set(v) { this.setAttribute(name, v); },
    configurable: true,
  });
}
Object.defineProperty(HTMLElement.prototype, "form", {
  get() {
    const id = this.getAttribute("form");
    return id ? document.getElementById(id) : this.closest("form");
  },
  configurable: true,
});
Object.defineProperty(HTMLButtonElement.prototype, "value", {
  get() { return this.getAttribute("value") ?? ""; },
  set(v) { this.setAttribute("value", v); },
  configurable: true,
});
Object.defineProperty(HTMLButtonElement.prototype, "type", {
  get() { return this.getAttribute("type") ?? "submit"; },
  set(v) { this.setAttribute("type", v); },
  configurable: true,
});
Object.defineProperty(HTMLInputElement.prototype, "type", {
  get() { return this.getAttribute("type") ?? "text"; },
  set(v) { this.setAttribute("type", v); },
  configurable: true,
});
Object.defineProperty(HTMLInputElement.prototype, "checked", {
  get() { return this._checked ?? this.hasAttribute("checked"); },
  set(v) { this._checked = !!v; },
  configurable: true,
});
// Felder eines Formulars: die darin und die mit form="<id>" außerhalb (der Knopf „Speichern“ der Statuszeile).
Object.defineProperty(HTMLFormElement.prototype, "elements", {
  get() {
    const inside = this.querySelectorAll("input, select, textarea, button");
    const id = this.getAttribute("id");
    const outside = id ? document.querySelectorAll("input, select, textarea, button").filter((e) => e.getAttribute("form") === id) : [];
    return [...inside, ...outside.filter((e) => !inside.includes(e))];
  },
  configurable: true,
});
HTMLFormElement.prototype.requestSubmit = function (submitter) {
  this.dispatchEvent(Object.assign(new Event("submit", { bubbles: true, cancelable: true }), { submitter: submitter ?? null }));
};
Object.defineProperty(HTMLFormElement.prototype, "action", {
  get() { return new URL(this.getAttribute("action") ?? "", window.location.href).href; },
  configurable: true,
});
HTMLTextAreaElement.prototype.setSelectionRange = function (start, end) { this.selectionStart = start; this.selectionEnd = end; };
HTMLTextAreaElement.prototype.setRangeText = function (text, start, end, mode) {
  const value = this.value;
  this.value = value.slice(0, start) + text + value.slice(end);
  if (mode === "end") { this.selectionStart = start + text.length; this.selectionEnd = start + text.length; }
};
class FormData {
  constructor(form, submitter) {
    this.pairs = [];
    for (const e of form ? form.elements : []) {
      const name = e.getAttribute("name");
      if (!name || e.disabled) continue;
      if (e instanceof HTMLButtonElement) continue;
      if (e instanceof HTMLInputElement && ["checkbox", "radio"].includes(e.type) && !e.checked) continue;
      if (e instanceof HTMLInputElement && ["submit", "button"].includes(e.type)) continue;
      const value = e instanceof HTMLInputElement && ["checkbox", "radio"].includes(e.type) ? (e.getAttribute("value") ?? "on") : e.value;
      this.pairs.push([name, String(value)]);
    }
    if (submitter && submitter.getAttribute("name")) this.pairs.push([submitter.getAttribute("name"), submitter.getAttribute("value") ?? ""]);
  }
  get(name) { const found = this.pairs.find(([k]) => k === name); return found ? found[1] : null; }
  append(name, value) { this.pairs.push([name, String(value)]); }
  forEach(fn) { for (const [k, v] of this.pairs) fn(v, k, this); }
  entries() { return this.pairs[Symbol.iterator](); }
  [Symbol.iterator]() { return this.pairs[Symbol.iterator](); }
}
class HTMLDialogElement extends HTMLElement {
  get open() { return this.hasAttribute("open"); }
  showModal() { this.setAttribute("open", ""); }
  close(value) {
    if (value !== undefined) this.returnValue = value;
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  }
}
KINDS.dialog = HTMLDialogElement;
class DOMParser {
  parseFromString(html) {
    const doc = make("parsed-document");
    parseInto(doc, html);
    doc.body = doc.querySelector("body") ?? doc;
    return doc;
  }
}
document.importNode = (node) => node;
const pushed = [];
const setLocation = (url) => {
  const next = new URL(url, window.location.href);
  Object.assign(window.location, { href: next.href, pathname: next.pathname, search: next.search, hash: next.hash });
};
window.location.origin = "http://127.0.0.1:8765";
const history = {
  pushState(_state, _title, url) { pushed.push({ how: "push", url }); setLocation(url); },
  replaceState(_state, _title, url) { pushed.push({ how: "replace", url }); setLocation(url); },
};
const stored = new Map();
const sessionStorage = { getItem: (k) => stored.get(k) ?? null, setItem: (k, v) => stored.set(k, String(v)), removeItem: (k) => stored.delete(k) };
window.setTimeout = (fn, ms) => setTimeout(fn, ms);
window.clearTimeout = (id) => clearTimeout(id);
window.getSelection = () => ({ toString: () => "", rangeCount: 0 });
Object.assign(globalThis, { FormData, HTMLDialogElement, DOMParser, history, sessionStorage, HTMLAnchorElement: HTMLElement,
  HTMLTableElement: HTMLElement, HTMLTableCellElement: HTMLElement, HTMLTableRowElement: HTMLElement });

// ---- Nachgebautes Netz: RPC (`scenario.rpc[method]` = Antwort oder Liste von Antworten) und Seiten (`scenario.pages`).
const rpcCalls = [];
const fetched = [];
globalThis.fetch = async (url, init) => {
  const href = String(url);
  if (href === "/rpc" || href.endsWith("/rpc")) {
    const request = JSON.parse(init.body);
    rpcCalls.push({
      method: request.method,
      params: request.params,
      editorInert: document.getElementById("pane-editor")?.inert === true,
    });
    const known = (scenario.rpc ?? {})[request.method];
    const answer = Array.isArray(known) ? (known.length > 1 ? known.shift() : known[0]) : known;
    await new Promise((r) => setTimeout(r, 0));
    if (answer && answer.error) return { ok: true, status: 200, json: async () => ({ jsonrpc: "2.0", id: request.id, error: answer.error }) };
    return { ok: true, status: 200, json: async () => ({ jsonrpc: "2.0", id: request.id, result: answer?.result ?? {} }) };
  }
  fetched.push({ url: href, method: init?.method ?? "GET", body: init?.body ?? null });
  const page = (scenario.pages ?? []).shift() ?? { status: 404, body: "" };
  await new Promise((r) => setTimeout(r, 0));
  return { ok: page.status >= 200 && page.status < 300, status: page.status, url: page.url ?? href, text: async () => page.body };
};

const dir = process.argv[3];
const settle = async (rounds = 30) => { for (let i = 0; i < rounds; i++) await new Promise((r) => setTimeout(r, 0)); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const key = (type, init) => Object.assign(new Event(type, { bubbles: true, cancelable: true }), init);
const out = {};

// ---- Szenen --------------------------------------------------------------------------------------------------------
switch (scenario.scene) {
  // Reine Funktionen eines Moduls: `scenario.module`, `scenario.calls` = [[Funktionsname, ...Argumente]].
  case "pure": {
    const module = await import(dir + "/" + scenario.module);
    out.values = scenario.calls.map(([name, ...args]) => module[name](...args));
    break;
  }

  // Einstellungen als Popup: Alt+, öffnet; `baruch:settings-changed` nur nach Erfolg.
  case "settings": {
    body.innerHTML =
      '<a href="/einstellungen/daten" data-settings-open data-settings-page="daten">Einstellungen</a>' +
      '<dialog id="settings" data-widget="settings"><div data-settings-content></div></dialog>';
    const widgets = await import(dir + "/widgets.js");
    await import(dir + "/settings.js");
    widgets.mount(body);
    const dialog = body.querySelector("dialog");
    const content = body.querySelector("[data-settings-content]");
    const events = [];
    document.addEventListener("baruch:settings-changed", (event) => events.push(event.detail));
    out.openBefore = dialog.open;
    const opened = key("keydown", { altKey: true, ctrlKey: false, metaKey: false, key: "," });
    document.dispatchEvent(opened);
    await settle();
    out.openAfterAltComma = dialog.open;
    out.altCommaPrevented = opened.defaultPrevented;
    out.firstFetch = fetched[0]?.url ?? null;
    out.framePage = content.querySelector("[data-settings-frame]")?.dataset.page ?? null;
    // Ohne Alt bleibt das Komma ein Komma.
    const plain = key("keydown", { altKey: false, key: "," });
    document.dispatchEvent(plain);
    out.plainCommaPrevented = plain.defaultPrevented;
    out.steps = [];
    for (let step = 0; step < scenario.submits; step++) {
      const form = content.querySelector("form");
      const before = events.length;
      form.dispatchEvent(Object.assign(new Event("submit", { bubbles: true, cancelable: true }), { submitter: form.querySelector("button") }));
      await settle();
      const last = fetched[fetched.length - 1];
      out.steps.push({
        posted: last ? { url: last.url, method: last.method, body: last.body } : null,
        events: events.slice(before),
        theme: document.documentElement.dataset.theme ?? null,
        alert: content.querySelector("[role=alert]")?.textContent ?? null,
        hasForm: !!content.querySelector("form"),
      });
    }
    break;
  }

  // Editor: Knopf-Weitergabe über requestSubmit(submitter), Sperre während des Speicherns, Hinweis danach.
  case "editor": {
    body.innerHTML =
      '<main class="workspace" data-widget="workspace"><nav id="pane-tree">' + scenario.tree + "</nav>" +
      '<section id="pane-editor">' + scenario.editor + "</section>" +
      '<aside id="pane-assistants">' + scenario.assistants + "</aside></main>";
    setLocation(scenario.url);
    const widgets = await import(dir + "/widgets.js");
    await import(dir + "/panes.js");
    await import(dir + "/outline.js");
    await import(dir + "/workspace.js");
    widgets.mount(body);
    const pane = document.getElementById("pane-editor");
    const form = pane.querySelector("form[data-widget=editor]");
    const area = form.querySelector("textarea");
    // Runde 33: Auch die Leiste (edit_toolbar.html, dockt ins Menüband an) trägt „Speichern“/„Als neue Fassung“ mit
    // form="editor-form"; geprüft werden wie bisher die maßgeblichen Knöpfe der Statuszeile.
    const buttons = form.elements.filter((e) => e instanceof HTMLButtonElement && e.type === "submit" && !e.closest("[data-edit-toolbar]"));
    const save = buttons.find((b) => !b.getAttribute("name"));
    const asNew = buttons.find((b) => b.getAttribute("name") === "as_new");
    out.buttonsFound = { save: !!save, asNew: !!asNew, asNewOutsideForm: !!asNew && !form.contains(asNew) };
    out.saveDisabledAtStart = save.disabled;
    area.value = area.value + "\nEin neuer Satz vom Test.\n";
    area.dispatchEvent(new Event("input", { bubbles: true }));
    out.dirtyAfterInput = { dirty: form.dataset.dirty, saveDisabled: save.disabled, label: save.textContent };
    // Der Browser löst beim Klick auf einen Absende-Knopf `submit` mit `submitter` aus.
    const press = (button) =>
      form.dispatchEvent(Object.assign(new Event("submit", { bubbles: true, cancelable: true }), { submitter: button }));
    press(scenario.button === "as_new" ? asNew : save);
    out.inertRightAfterPress = pane.inert === true;
    out.busyRightAfterPress = pane.getAttribute("aria-busy");
    // Ein zweiter Druck, während das erste Speichern läuft.
    press(scenario.button === "as_new" ? asNew : save);
    await settle(60);
    out.calls = rpcCalls.map((c) => ({ method: c.method, editorInert: c.editorInert, mode: c.params?.mode ?? null,
      asNew: c.params?.as_new ?? null, body: typeof c.params?.body === "string" ? c.params.body : null,
      basedOn: c.params?.based_on ?? null, basedOnAmend: c.params?.based_on_amend ?? null }));
    out.inertAfter = pane.inert === true;
    out.busyAfter = pane.getAttribute("aria-busy");
    out.notices = document.querySelectorAll(".toast-notice").map((t) => ({ text: t.textContent, role: t.getAttribute("role") }));
    out.alerts = document.querySelectorAll(".toast").filter((t) => !t.classList.contains("toast-notice")).map((t) => t.textContent);
    out.editorAfter = pane.innerHTML;
    out.areaValue = pane.querySelector("textarea[name=body]")?.value ?? null;
    out.pushed = pushed;
    break;
  }

  // Runde 33 (menueband): das Menüband auf der echten Arbeitsseite des Servers (`scenario.body` = Inhalt von <body>).
  // Beobachtet: Andocken der Leiste, Umschalter Satzbild/Quelltext nach dem Andocken, Reiter mit Tasten und Klick,
  // Band auf/zu mit Cookie, Gruppen als Popups nach `baruch:laptop`, Tastenkürzel.
  case "band": {
    body.innerHTML = scenario.body;
    setLocation(scenario.url);
    html.setAttribute("data-band", "open");
    const widgets = await import(dir + "/widgets.js");
    for (const name of ["band", "menu", "panes", "ribbon", "sheet", "wysiwyg", "workspace", "settings", "literature", "ideas"]) {
      await import(dir + "/" + name + ".js");
    }
    const head = body.querySelector("header.band");
    const pane = document.getElementById("pane-editor");
    out.before = {
      barInPane: !!pane.querySelector("[data-edit-toolbar]"),
      barHidden: pane.querySelector("[data-edit-toolbar]")?.hidden ?? null,
      dockChildren: head.querySelector("[data-band-dock]").childNodes.length,
    };
    widgets.mount(body);
    await settle();
    const dock = head.querySelector("[data-band-dock]");
    const pressed = (mode) => dock.querySelector("[data-ws-mode=" + mode + "]")?.getAttribute("aria-pressed") ?? null;
    const tabState = () => ({
      tab: head.getAttribute("data-tab"),
      selected: head.querySelectorAll("[data-band-tab]").filter((t) => t.getAttribute("aria-selected") === "true").map((t) => t.dataset.bandTab),
      shown: head.querySelectorAll("[data-band-panel]").filter((p) => !p.hidden).map((p) => p.dataset.bandPanel),
      focus: document.activeElement?.dataset?.bandTab ?? null,
      band: html.getAttribute("data-band"),
      peek: head.hasAttribute("data-band-peek"),
      expanded: head.querySelector("[data-band-toggle]").getAttribute("aria-expanded"),
      cookie: document.cookie,
    });
    out.docked = {
      barInPane: !!pane.querySelector("[data-edit-toolbar]"),
      barInDock: !!dock.querySelector("[data-edit-toolbar]"),
      barHidden: dock.querySelector("[data-edit-toolbar]")?.hidden ?? null,
      parts: dock.querySelectorAll("[data-band-part]").map((p) => p.dataset.bandPart),
      headDocked: head.hasAttribute("data-docked"),
      satz: pressed("satz"), quelltext: pressed("quelltext"),
      sheetOpen: pane.querySelector("div.wysiwyg").hasAttribute("data-ws-open"),
    };
    // Umschalter im Band: Quelltext, dann zurück ins Satzbild.
    dock.querySelector("[data-ws-mode=quelltext]").click();
    await settle();
    out.afterQuelltext = { satz: pressed("satz"), quelltext: pressed("quelltext"),
      areaHidden: pane.querySelector("textarea[name=body]").hidden,
      toolsHidden: dock.querySelector("[data-ws-tools]").hidden,
      sheetOpen: pane.querySelector("div.wysiwyg").hasAttribute("data-ws-open") };
    // Strg+S im Textfeld nach einer Änderung: gespeichert wird über das Formular des Editors.
    const area = pane.querySelector("textarea[name=body]");
    const statusSave = pane.querySelectorAll("button").find((b) => b.hasAttribute("data-saved"));
    out.saveDisabledClean = statusSave.disabled;
    // Unverändert: „Speichern“ im Band speichert nicht (der Knopf der Statuszeile ist gesperrt).
    const bandSave = dock.querySelector("[data-band-save]");
    const idle = Object.assign(new Event("click", { bubbles: true, cancelable: true }), { button: 0 });
    bandSave.dispatchEvent(idle);
    out.bandSaveIdlePrevented = idle.defaultPrevented;
    area.value = area.value + "\nEin Satz aus dem Verhaltenstest.\n";
    area.dispatchEvent(new Event("input", { bubbles: true }));
    out.saveLabelDirty = statusSave.textContent;
    out.bandSaveLabel = bandSave.textContent;
    // Wunsch 2026-11-27: Strg+S gilt überall auf der Seite (hier mit dem Fokus im Menüband, nicht im Textfeld), und
    // danach steht der Cursor wieder an derselben Stelle im neu gezeichneten Textfeld.
    area.setSelectionRange(4, 9);
    const ctrlS = key("keydown", { ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, key: "s" });
    dock.dispatchEvent(ctrlS);
    out.ctrlSPrevented = ctrlS.defaultPrevented;
    out.dockInertWhileSaving = dock.inert === true;
    await settle(60);
    out.saveCalls = rpcCalls.map((c) => c.method);
    out.dockInertAfter = dock.inert === true;
    {
      const fresh = pane.querySelector("textarea[name=body]");
      out.placeAfterSave = {
        newArea: fresh !== area,
        focused: document.activeElement === fresh,
        start: fresh.selectionStart, end: fresh.selectionEnd,
      };
    }
    // Nach dem Neuzeichnen (ui.render mit dem Editor von vorher): wieder angedockt, genau eine Leiste.
    out.redocked = {
      bars: document.querySelectorAll("[data-edit-toolbar]").length,
      barInDock: !!dock.querySelector("[data-edit-toolbar]"),
      barInPane: !!pane.querySelector("[data-edit-toolbar]"),
    };
    // Reiter mit der Tastatur (ARIA-Tabs).
    const tabOf = (id) => head.querySelector("[data-band-tab=" + id + "]");
    out.tabs = { start: tabState() };
    const press = (id, k) => { const e = key("keydown", { key: k, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false }); tabOf(id).dispatchEvent(e); return e.defaultPrevented; };
    out.tabs.rightPrevented = press("start", "ArrowRight");
    out.tabs.afterRight = tabState();
    out.tabs.partsAfterRight = head.getAttribute("data-tab");
    press("einfuegen", "End");
    out.tabs.afterEnd = tabState();
    press("ansicht", "ArrowRight");
    out.tabs.afterWrap = tabState();
    press("start", "ArrowLeft");
    out.tabs.afterLeft = tabState();
    press("ansicht", "Home");
    out.tabs.afterHome = tabState();
    out.tabs.tabIndex = head.querySelectorAll("[data-band-tab]").map((t) => t.tabIndex);
    // Klick: anderer Reiter wechselt, der aktive klappt das Band zu (Cookie), zugeklappt zeigt ein Reiter sein Feld vorübergehend.
    const click = (el) => { const e = Object.assign(new Event("click", { bubbles: true, cancelable: true }), { button: 0 }); el.dispatchEvent(e); return e.defaultPrevented; };
    out.clicks = { buchPrevented: click(tabOf("buch")) };
    out.clicks.afterBuch = tabState();
    click(tabOf("buch"));
    out.clicks.afterActive = tabState();
    click(tabOf("pruefen"));
    out.clicks.peek = tabState();
    document.dispatchEvent(key("keydown", { key: "Escape" }));
    out.clicks.afterEscape = tabState();
    click(tabOf("pruefen"));
    click(body.querySelector("main"));
    out.clicks.afterOutside = tabState();
    click(head.querySelector("[data-band-toggle]"));
    out.clicks.afterToggle = tabState();
    // Gruppen: kompakt als Popups (laptop.ts meldet den Wechsel), danach wieder voll.
    const groups = () => head.querySelectorAll("details[data-band-group]");
    const groupState = () => ({ count: groups().length, open: groups().filter((g) => g.open).length,
      named: groups().filter((g) => g.getAttribute("name") === "band-group").length,
      dockGroups: dock.querySelectorAll("details[data-band-group]").length });
    out.groups = { full: groupState() };
    html.setAttribute("data-compact", "");
    document.dispatchEvent(new CustomEvent("baruch:laptop", { detail: { compact: true, narrow: false } }));
    out.groups.compact = groupState();
    const one = groups()[0]; one.open = true;
    document.dispatchEvent(key("keydown", { key: "Escape" }));
    out.groups.afterEscape = groupState();
    html.removeAttribute("data-compact");
    document.dispatchEvent(new CustomEvent("baruch:laptop", { detail: { compact: false, narrow: false } }));
    out.groups.fullAgain = groupState();
    // Tastenkürzel: Alt+L (Literatur), Alt+E (Einfall), Alt+, (Einstellungen).
    out.keys = {};
    for (const [name, init, id] of [["altL", { key: "l" }, "literature"], ["altE", { key: "e", code: "KeyE" }, "idea-capture"], ["altComma", { key: "," }, "settings"]]) {
      const dialog = document.getElementById(id);
      const event = key("keydown", { altKey: true, ctrlKey: false, metaKey: false, shiftKey: false, ...init });
      document.dispatchEvent(event);
      await settle();
      out.keys[name] = { dialog: !!dialog, prevented: event.defaultPrevented, open: dialog?.open ?? null };
      if (dialog?.open) dialog.close();
      await settle();
    }
    out.openers = Object.fromEntries(["data-settings-open", "data-literature-open", "data-idea-open", "data-work-import-open", "data-view-toggle"]
      .map((hook) => [hook, head.querySelectorAll("[" + hook + "]").length]));
    break;
  }

  // Befunde im Akkordeon: Statusformular über RPC, danach Neuzeichnen; Fehler als Hinweis am Formular.
  case "findings": {
    body.innerHTML = '<aside id="pane-assistants">' + scenario.assistants + "</aside>";
    setLocation(scenario.url);
    const widgets = await import(dir + "/widgets.js");
    await import(dir + "/findings.js");
    widgets.mount(body);
    const navigations = [];
    document.addEventListener("baruch:navigate", (event) => navigations.push(event.detail));
    const forms = body.querySelectorAll("form[data-finding-form]");
    out.forms = forms.length;
    const form = forms.find((f) => f.querySelector("input[name=status]")?.value === scenario.status) ?? forms[0];
    out.formMethod = form.querySelector("input[name=method]")?.value ?? null;
    const reason = form.querySelector("[name=reason]");
    if (reason && scenario.reason !== undefined) reason.value = scenario.reason;
    const button = form.querySelector("button[type=submit]");
    const send = async () => {
      const event = new Event("submit", { bubbles: true, cancelable: true });
      form.dispatchEvent(event);
      const disabledDuring = button ? button.disabled : null;
      await settle();
      return { prevented: event.defaultPrevented, disabledDuring, disabledAfter: button ? button.disabled : null };
    };
    out.first = await send();
    out.errorAfterFirst = form.querySelector(".fd-error")?.textContent ?? null;
    out.navigationsAfterFirst = navigations.length;
    out.second = await send();
    out.errorAfterSecond = form.querySelector(".fd-error")?.textContent ?? null;
    out.navigations = navigations;
    out.calls = rpcCalls.map((c) => ({ method: c.method, params: c.params }));
    break;
  }

  // Registermarke im Satzbild: Steht dieselbe Marke schon direkt hinter der Stelle?
  case "marks": {
    const ws = await import(dir + "/wysiwyg.js");
    const cmd = await import(dir + "/ws_command.js");
    const mark = (m) => {
      const el = make("span");
      el.className = "ws-index-mark";
      el.dataset.raw = cmd.markSource(cmd.cleanMark(m));
      return el;
    };
    const block = make("div");
    const before = new Text_("Die Gnade");
    const empty = new Text_("");
    const after = new Text_(" trägt.");
    const first = mark(scenario.present[0]);
    const second = mark(scenario.present[1]);
    block.append(before, first, empty, second, after);
    const at = (node, offset) => ({ startContainer: node, startOffset: offset });
    out.values = scenario.asked.map(([where, m]) => {
      const range =
        where === "end-of-text" ? at(before, before.data.length)
        : where === "inside-text" ? at(before, 3)
        : where === "element-offset" ? at(block, 1)
        : where === "after-marks" ? at(after, after.data.length)
        : at(block, 4);
      return ws.sameMarkBehind(range, m);
    });
    break;
  }

  // Dialog „Verweis“ (Runde 30a) im Quelltext: Ziel suchen, wählen, einfügen.
  case "xref": {
    body.innerHTML =
      '<div class="wysiwyg" data-widget="wysiwyg" data-work="' + scenario.work + '"><textarea name="body">' +
      scenario.text + "</textarea></div>";
    await import(dir + "/xref.js");
    const root = body.querySelector("div.wysiwyg");
    const area = root.querySelector("textarea");
    area.selectionStart = scenario.caret;
    area.selectionEnd = scenario.caret;
    let inputs = 0;
    area.addEventListener("input", () => inputs++);
    root.dispatchEvent(new CustomEvent("baruch:xref-open", { bubbles: true }));
    await settle();
    const dialog = document.querySelector("dialog[data-xref-dialog]");
    out.dialogOpen = !!dialog && dialog.open;
    out.pageCheckedFromWork = dialog.querySelector("[data-xref-page]").checked;
    const q = dialog.querySelector("[data-xref-q]");
    q.value = scenario.query;
    q.dispatchEvent(new Event("input", { bubbles: true }));
    await sleep(220);
    await settle();
    const options = dialog.querySelectorAll("[data-xref-targets] button");
    out.options = options.map((b) => b.textContent);
    out.previewBefore = dialog.querySelector("[data-xref-preview]").textContent;
    if (!options[scenario.pick]) {
      errors.push("kein Ziel in der Liste: " + out.previewBefore);
      break;
    }
    options[scenario.pick].click();
    out.selected = options.map((b) => b.getAttribute("aria-selected"));
    out.textAfterPick = dialog.querySelector("[data-xref-text]").value;
    out.previewAfter = dialog.querySelector("[data-xref-preview]").textContent;
    dialog.close("ok");
    await settle();
    out.areaAfter = area.value;
    out.inputs = inputs;
    out.dialogGone = !document.querySelector("dialog[data-xref-dialog]");
    out.calls = rpcCalls.map((c) => ({ method: c.method, params: c.params }));
    break;
  }

  // EPUB-Vorschau: Auswahllisten schicken erst bei Bestätigung ab (Pfeiltasten laden nicht neu).
  case "epub-preview": {
    body.innerHTML =
      '<form class="epub-preview-form" data-widget="epub-preview" method="get" action="/werke/3/epub/vorschau">' +
      '<input type="checkbox" name="leser" value="kindle" checked><input type="checkbox" name="nacht" value="1">' +
      '<select name="datei"><option value="a" selected>A</option><option value="b">B</option></select>' +
      '<button type="submit">Anzeigen</button></form>';
    const widgets = await import(dir + "/widgets.js");
    await import(dir + "/epub_preview.js");
    widgets.mount(body);
    const form = body.querySelector("form");
    const select = form.querySelector("select");
    const night = form.querySelector("input[name=nacht]");
    let submits = 0;
    form.addEventListener("submit", (event) => { submits++; event.preventDefault(); });
    const change = (target) => target.dispatchEvent(new Event("change", { bubbles: true }));
    const down = (name) => select.dispatchEvent(key("keydown", { key: name }));
    out.steps = [];
    const note = (what) => out.steps.push([what, submits]);
    // Drei Pfeiltasten in der geschlossenen Liste: Firefox meldet jedes Mal `change`.
    for (let i = 0; i < 3; i++) { down("ArrowDown"); select.value = i % 2 ? "a" : "b"; change(select); }
    note("nach drei Pfeiltasten");
    down("Enter");
    note("nach Eingabetaste");
    // Wahl mit der Maus: sofort.
    select.dispatchEvent(Object.assign(new Event("pointerdown", { bubbles: true }), {}));
    select.value = "a"; change(select);
    note("nach Wahl mit der Maus");
    // Ein Kästchen wirkt sofort.
    night.checked = true; change(night);
    note("nach Kästchen");
    // Aufgeklappte Liste: Eingabetaste, dann erst `change`.
    down("ArrowDown"); down("Enter"); select.value = "b"; change(select);
    note("aufgeklappt: Pfeil, Eingabetaste, change");
    break;
  }

  default:
    errors.push("unbekannte Szene " + scenario.scene);
}
out.errors = errors;
process.stdout.write(JSON.stringify(out), () => process.exit(0));
