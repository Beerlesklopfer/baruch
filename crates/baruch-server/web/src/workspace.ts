// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „workspace“: Links und Formulare ohne Neuladen. Ein Link `data-nav` holt die drei
// Bereiche über `ui.render`; ein Formular `data-rpc` ruft seine Methode (Feld `method`) über RPC
// und zeichnet danach neu. Alt+←/→ blättert durch die Fassungen (nach Nummer, mit Lücken);
// Alt+Umschalt+Pfeil gehört der Gliederung (`outline.ts`). Runde 30a: Alt+← springt bei vorhandenem
// Sprungverlauf (jump.ts) zur Ausgangsstelle zurück; nach jedem Neuzeichnen kommt `baruch:rendered`. Andere Widgets lösen mit dem Ereignis
// `baruch:navigate` (`detail.url`) ein Neuzeichnen über `ui.render` aus. Vor allem, was neu
// zeichnet, fragt `leaveEditor()` bei ungespeichertem Text im Editor nach.
// Runde 22: Der auslösende Knopf eines Formulars geht mit (Name/Wert, z. B. `as_new=1` → `mode: "new"` bei
// `outline.save`); nach dem Speichern zeigt eine Meldung „fortgeschrieben (Stand k)“ bzw. „neue Fassung n (Grund)“.

import { jumpBack } from "./jump.js";
import { call, locationFrom, savedMessage, urlFor, type Location, type Rendered, type SaveAnswer } from "./rpc.js";
import { leaveEditor, lockEditor, mount, register } from "./widgets.js";

const panes = ["tree", "editor", "assistants"] as const;

function showError(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast";
  toast.setAttribute("role", "alert");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 5000);
}

/// Hinweis ohne Alarm (Runde 22: Rückmeldung nach dem Speichern).
function showNotice(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast toast-notice";
  toast.setAttribute("role", "status");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 4000);
}

/// Runde 23a (Codereview web-5): laufende Nummer des Neuzeichnens. Es gilt die zuletzt **ausgelöste** Anfrage, nicht
/// die zuletzt eintreffende Antwort; eine überholte Antwort wird verworfen (sonst zeigte ein schneller zweiter Klick
/// wieder den ersten Abschnitt, mit vertauschten Einträgen im Verlauf).
let renderSeq = 0;

async function render(at: Location, push: boolean): Promise<void> {
  const mine = ++renderSeq;
  const result = await call<Rendered>("ui.render", { ...at });
  if (mine !== renderSeq) return;
  for (const pane of panes) {
    const element = document.getElementById(`pane-${pane}`);
    if (!element) continue;
    element.innerHTML = result[pane];
    mount(element);
  }
  document.title = result.title;
  // Kopfzeile: Umschalter Editor/Vorschau zeigt auf die andere Ansicht (Werk und Suche bleiben).
  const toggle = document.querySelector<HTMLAnchorElement>("[data-view-toggle]");
  if (toggle) {
    const now = locationFrom(result.url);
    const preview = now.view === "preview";
    toggle.textContent = preview ? "Editor" : "Vorschau";
    toggle.href = urlFor({ ...now, view: preview ? undefined : "preview" });
  }
  if (push) history.pushState(null, "", result.url);
  else history.replaceState(null, "", result.url);
  // Runde 30a: jump.ts markiert danach eine ausstehende Fundstelle (`baruch:find`).
  document.dispatchEvent(new CustomEvent("baruch:rendered", { detail: { url: result.url } }));
}

async function navigate(url: string, push = true): Promise<void> {
  try {
    // Fehlen Werk oder Suche im Link, bleiben die der aktuellen Adresse (Navigation im Werk).
    const at = locationFrom(url);
    const here = locationFrom(window.location.href);
    await render({ ...at, work: at.work ?? here.work, lq: at.lq ?? here.lq }, push);
  } catch (error) {
    showError(String((error as Error).message ?? error));
  }
}

async function submit(form: HTMLFormElement, submitter: HTMLElement | null = null): Promise<void> {
  // Runde 23a (Codereview web-1): Das Editor-Formular sperrt den Editor, bis gespeichert und neu gezeichnet ist; ein
  // zweites Absenden während des Laufs entfällt.
  const editor = form.matches("[data-widget=editor]");
  if (editor && form.dataset.saving === "1") return;
  const release = editor ? lockEditor() : null;
  if (editor) form.dataset.saving = "1";
  try {
    await send(form, submitter);
  } finally {
    if (editor) delete form.dataset.saving;
    release?.();
  }
}

async function send(form: HTMLFormElement, submitter: HTMLElement | null): Promise<void> {
  const data = new FormData(form);
  // Der auslösende Knopf mit Name (z. B. „Als neue Fassung speichern“, `as_new=1`) zählt wie ein Feld.
  if (submitter instanceof HTMLButtonElement && submitter.name) data.append(submitter.name, submitter.value);
  const method = String(data.get("method") ?? "");
  const params: Record<string, unknown> = {};
  for (const [key, value] of data) {
    if (key === "method" || key === "view" || key === "lq") continue;
    // Pflicht-Kontrollkästchen vor dem Löschen: angehakt → `confirm: true` (der Server prüft es).
    if (key === "confirm") {
      params.confirm = true;
      continue;
    }
    const numeric =
      key === "work" || key === "node" || key === "key" || key === "rev" || key === "based_on" ||
      key === "based_on_amend" || key === "id" ||
      key === "target_key";
    params[key] = numeric ? Number(value) : String(value);
  }
  // Runde 22: „Als neue Fassung speichern“ geht vor dem versteckten `mode=auto` (wie `/gliederung/act`).
  if (params.as_new !== undefined) {
    if (params.as_new === "1") params.mode = "new";
    delete params.as_new;
  }
  const view = data.get("view") === "preview" ? "preview" : undefined;
  // Werk und Suche: aus dem Formular, sonst aus der aktuellen Adresse.
  const here = locationFrom(window.location.href);
  if (params.work === undefined && here.work !== undefined) params.work = here.work;
  const lq = data.get("lq") !== null ? String(data.get("lq")) : here.lq;
  try {
    const result = await call<SaveAnswer>(method, params);
    const rev = result.rev ?? (params.rev as number | undefined);
    const work = params.work as number | undefined;
    // Danach zu zeigen (Key-Form, Runde 8): Beiträge verwalten bleibt im gezeigten Abschnitt (`key` des
    // Formulars; beim Verschieben nennt die Antwort das Ziel); sonst der Schlüssel der Antwort, dann der
    // des Formulars, zuletzt die Stelle.
    const shown = params.key as number | undefined;
    const answered = typeof result.key === "number" ? result.key : undefined;
    const key = method.startsWith("post.") ? (shown ?? answered) : (answered ?? shown);
    const quiet = method.startsWith("post.");
    await render({ work, key, node: key === undefined ? result.node : undefined, rev, view, lq }, !quiet);
    const said = method === "outline.save" || method === "revision.save" ? savedMessage(result) : null;
    if (said) showNotice(said);
    if (method === "post.add") {
      document.querySelector<HTMLInputElement>(".chat-input input[name=body]")?.focus();
    }
  } catch (error) {
    showError(String((error as Error).message ?? error));
  }
}

/// Sprungverlauf nicht leer: (nach der Rückfrage bei ungespeichertem Text) zurückspringen; `true` = erledigt.
function leaveEditorIfJump(): boolean {
  try {
    const raw = sessionStorage.getItem("baruch_jumps");
    const back = raw ? (JSON.parse(raw) as { back?: unknown[] }).back : undefined;
    if (!Array.isArray(back) || back.length === 0) return false;
  } catch {
    return false;
  }
  if (!leaveEditor()) return true;
  return jumpBack();
}

register("workspace", (root) => {
  root.addEventListener("click", (event) => {
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[data-nav]");
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    if (leaveEditor()) void navigate(link.href);
  });
  // Auch der Umschalter in der Kopfzeile liegt außerhalb des Arbeitsbereichs.
  document.querySelector("[data-view-toggle]")?.addEventListener("click", (event) => {
    const link = event.currentTarget as HTMLAnchorElement;
    const e = event as MouseEvent;
    if (e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey) return;
    event.preventDefault();
    if (leaveEditor()) void navigate(link.href);
  });
  root.addEventListener("submit", (event) => {
    const form = (event.target as Element).closest<HTMLFormElement>("form[data-rpc]");
    if (!form) return;
    event.preventDefault();
    // Jedes Formular außer dem Editor selbst zeichnet danach neu und verwürfe ungespeicherten Text
    // (Gliederung ↑ ↓ ← →, Umschalten, Fassung ändern, Beitrag senden): erst fragen.
    if (!form.matches("[data-widget=editor]") && !leaveEditor()) return;
    void submit(form, (event as SubmitEvent).submitter ?? null);
  });
  document.addEventListener("keydown", (event) => {
    // Nur Alt ohne Umschalt; Alt+Umschalt+Pfeil stuft in der Gliederung um.
    if (!event.altKey || event.shiftKey || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    const key = event.key === "ArrowLeft" ? "prev" : "next";
    // Runde 30a: Alt+← kehrt bei vorhandenem Sprungverlauf zur Ausgangsstelle zurück (jump.ts).
    if (key === "prev" && leaveEditorIfJump()) {
      event.preventDefault();
      return;
    }
    const link = document.querySelector<HTMLAnchorElement>(`a[data-key=${key}]`);
    if (!link) return;
    event.preventDefault();
    if (leaveEditor()) void navigate(link.href);
  });
  window.addEventListener("popstate", () => void navigate(window.location.href, false));
  // Andere Widgets (z. B. die Gliederung nach dem Verschieben) zeichnen die Bereiche neu.
  // `detail.asked: true` = der Absender hat schon gefragt (outline.ts vor dem RPC-Aufruf).
  document.addEventListener("baruch:navigate", (event) => {
    const detail = (event as CustomEvent<{ url?: unknown; asked?: unknown }>).detail;
    const url = detail?.url;
    if (typeof url !== "string" || url === "") return;
    if (detail?.asked === true || leaveEditor()) void navigate(url);
  });
});
