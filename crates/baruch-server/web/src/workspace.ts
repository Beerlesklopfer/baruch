// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „workspace“: links and forms without reloading. A link `data-nav` fetches the three
// areas through `ui.render`; a form `data-rpc` calls its method (field `method`) through RPC
// and redraws afterwards. Alt+←/→ pages through the revisions (by number, with gaps);
// Alt+Shift+arrow belongs to the outline (`outline.ts`). Round 30a: with an existing jump history (jump.ts)
// Alt+← jumps back to the place of origin; after every redraw `baruch:rendered` follows. Other widgets trigger a
// redraw through `ui.render` with the event `baruch:navigate` (`detail.url`). Before everything that
// redraws, `leaveEditor()` asks if there is unsaved text in the editor.
// Round 22: the triggering button of a form goes along (name/value, e.g. `as_new=1` → `mode: "new"` for
// `outline.save`); after saving a message shows „fortgeschrieben (Stand k)“ or „neue Fassung n (Grund)“.

import { t } from "./i18n.js";
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

/// Notice without alarm (Round 22: feedback after saving).
function showNotice(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast toast-notice";
  toast.setAttribute("role", "status");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 4000);
}

/// Round 23a (code review web-5): running number of the redraw. The request **triggered** last applies, not
/// the answer arriving last; an overtaken answer is discarded (otherwise a fast second click would show
/// the first section again, with swapped entries in the history).
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
  // Header: the toggle Editor/Vorschau points to the other view (work and search stay).
  const toggle = document.querySelector<HTMLAnchorElement>("[data-view-toggle]");
  if (toggle) {
    const now = locationFrom(result.url);
    const preview = now.view === "preview";
    toggle.textContent = preview ? t("widget.workspace.editor") : t("widget.workspace.preview");
    toggle.href = urlFor({ ...now, view: preview ? undefined : "preview" });
  }
  if (push) history.pushState(null, "", result.url);
  else history.replaceState(null, "", result.url);
  // Round 30a: jump.ts afterwards marks a pending place found (`baruch:find`).
  document.dispatchEvent(new CustomEvent("baruch:rendered", { detail: { url: result.url } }));
}

async function navigate(url: string, push = true): Promise<void> {
  try {
    // If work or search are missing in the link, those of the current address stay (navigation in the work).
    const at = locationFrom(url);
    const here = locationFrom(window.location.href);
    await render({ ...at, work: at.work ?? here.work, lq: at.lq ?? here.lq }, push);
  } catch (error) {
    showError(String((error as Error).message ?? error));
  }
}

async function submit(form: HTMLFormElement, submitter: HTMLElement | null = null): Promise<void> {
  // Round 23a (code review web-1): the editor form locks the editor until saving and redrawing are done; a
  // second submit during the run is dropped.
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
  // The triggering button with a name (e.g. „Als neue Fassung speichern“, `as_new=1`) counts like a field.
  if (submitter instanceof HTMLButtonElement && submitter.name) data.append(submitter.name, submitter.value);
  const method = String(data.get("method") ?? "");
  const params: Record<string, unknown> = {};
  for (const [key, value] of data) {
    if (key === "method" || key === "view" || key === "lq") continue;
    // Mandatory checkbox before deleting: checked → `confirm: true` (the server checks it).
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
  // Round 22: „Als neue Fassung speichern“ takes precedence over the hidden `mode=auto` (like `/gliederung/act`).
  if (params.as_new !== undefined) {
    if (params.as_new === "1") params.mode = "new";
    delete params.as_new;
  }
  const view = data.get("view") === "preview" ? "preview" : undefined;
  // Work and search: from the form, otherwise from the current address.
  const here = locationFrom(window.location.href);
  if (params.work === undefined && here.work !== undefined) params.work = here.work;
  const lq = data.get("lq") !== null ? String(data.get("lq")) : here.lq;
  try {
    const result = await call<SaveAnswer>(method, params);
    const rev = result.rev ?? (params.rev as number | undefined);
    const work = params.work as number | undefined;
    // To show afterwards (key form, Round 8): managing posts stays in the shown section (`key` of the
    // form; when moving, the answer names the target); otherwise the key of the answer, then that of
    // the form, last the position.
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

/// Jump history not empty: jump back (after the confirmation for unsaved text); `true` = done.
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
  // Round 33: on the document instead of on the workspace – the toggle Editor/Vorschau and the ‹ › links of the
  // docked bar also stand in the ribbon, outside <main>.
  document.addEventListener("click", (event) => {
    const link = (event.target as Element | null)?.closest?.<HTMLAnchorElement>("a[data-nav]");
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    if (leaveEditor()) void navigate(link.href);
  });
  root.addEventListener("submit", (event) => {
    const form = (event.target as Element).closest<HTMLFormElement>("form[data-rpc]");
    if (!form) return;
    event.preventDefault();
    // Every form except the editor itself redraws afterwards and would discard unsaved text
    // (outline ↑ ↓ ← →, switching, changing a revision, sending a post): ask first.
    if (!form.matches("[data-widget=editor]") && !leaveEditor()) return;
    void submit(form, (event as SubmitEvent).submitter ?? null);
  });
  document.addEventListener("keydown", (event) => {
    // Only Alt without Shift; Alt+Shift+arrow changes the level in the outline.
    if (!event.altKey || event.shiftKey || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    const key = event.key === "ArrowLeft" ? "prev" : "next";
    // Round 30a: with an existing jump history Alt+← returns to the place of origin (jump.ts).
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
  // Other widgets (e.g. the outline after moving) redraw the areas.
  // `detail.asked: true` = the sender has already asked (outline.ts before the RPC call).
  document.addEventListener("baruch:navigate", (event) => {
    const detail = (event as CustomEvent<{ url?: unknown; asked?: unknown }>).detail;
    const url = detail?.url;
    if (typeof url !== "string" || url === "") return;
    if (detail?.asked === true || leaveEditor()) void navigate(url);
  });
});
