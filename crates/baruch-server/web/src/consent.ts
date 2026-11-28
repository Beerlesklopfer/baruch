// SPDX-License-Identifier: AGPL-3.0-or-later
// Active consent to requests (user question 12, decision 2026-11-07: „es braucht eine aktive
// Zustimmung (Popup)“). The server (crate::consent) supplies on every page, as long as such a request
// is open, the notice bar with a link to the request (usable without JavaScript) and the <dialog
// id="consent">.
//
// - Widget „consent“: opens the dialog modally on loading (showModal). „Ansehen und entscheiden“ in the
//   bar ([data-consent-open]) opens it again instead of going to the outline page.
// - „Zustimmen“/„Ablehnen“ are ordinary forms to /gliederung/act; the widget sends them through
//   RPC outline.request_decide and reloads the page afterwards (with an unsaved change in the editor
//   only after a confirmation). If the request is outdated, the reason stands in the dialog.
// - Nothing happens without a click: no timeout, and closing („Später entscheiden“, Escape) leaves the
//   request open.

import { t } from "./i18n.js";
import { call, RpcError } from "./rpc.js";
import { leaveEditor, register } from "./widgets.js";

interface Decided {
  request: { id: number; decision: string; decision_label: string; message?: string | null };
}

function field(form: HTMLFormElement, name: string): string {
  const element = form.elements.namedItem(name);
  return element instanceof HTMLInputElement ? element.value : "";
}

function reload(): void {
  if (leaveEditor()) window.location.reload();
}

register("consent", (element) => {
  if (!(element instanceof HTMLDialogElement)) return;
  const dialog = element;
  const open = () => {
    if (!dialog.open) dialog.showModal();
  };
  document.querySelectorAll<HTMLElement>("[data-consent-open]").forEach((link) => {
    link.addEventListener("click", (event) => {
      event.preventDefault();
      open();
    });
  });
  let changed = false;
  dialog.addEventListener("close", () => {
    if (changed) reload();
  });
  dialog.querySelectorAll<HTMLFormElement>("form.consent-actions").forEach((form) => {
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const submitter = (event as SubmitEvent).submitter;
      const decision = submitter instanceof HTMLButtonElement ? submitter.value : "";
      if (decision !== "confirm" && decision !== "reject") return;
      const message = form.querySelector<HTMLElement>("[data-consent-message]");
      const buttons = Array.from(form.querySelectorAll<HTMLButtonElement>("button"));
      buttons.forEach((button) => (button.disabled = true));
      try {
        const result = await call<Decided>("outline.request_decide", {
          work: Number(field(form, "work")),
          id: Number(field(form, "id")),
          decision,
        });
        changed = true;
        const request = result.request;
        if (request.decision === "stale") {
          // Outdated: nothing changed; the reason stays until the human closes the dialog.
          if (message) message.textContent = t("widget.consent.not_done", { label: request.decision_label, message: request.message ?? "" });
          return;
        }
        form.closest(".consent-request")?.remove();
        if (!dialog.querySelector(".consent-request")) dialog.close();
      } catch (error) {
        buttons.forEach((button) => (button.disabled = false));
        const text = error instanceof RpcError ? error.message : String(error);
        if (message) message.textContent = t("widget.consent.error", { text });
      }
    });
  });
  open();
});
