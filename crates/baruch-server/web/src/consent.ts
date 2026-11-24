// SPDX-License-Identifier: AGPL-3.0-or-later
// Aktive Zustimmung zu Anträgen (Nutzerfrage 12, Entscheidung 2026-11-07: „es braucht eine aktive
// Zustimmung (Popup)“). Der Server (crate::consent) liefert auf jeder Seite, solange ein solcher Antrag
// offen ist, die Hinweisleiste mit Link zum Antrag (ohne JavaScript bedienbar) und den <dialog
// id="consent">.
//
// - Widget „consent“: öffnet den Dialog beim Laden modal (showModal). „Ansehen und entscheiden“ in der
//   Leiste ([data-consent-open]) öffnet ihn wieder, statt zur Gliederungsseite zu gehen.
// - „Zustimmen“/„Ablehnen“ sind gewöhnliche Formulare an /gliederung/act; das Widget schickt sie über
//   RPC outline.request_decide und lädt danach die Seite neu (bei ungespeicherter Änderung im Editor
//   erst nach Rückfrage). Ist der Antrag veraltet, steht der Grund im Dialog.
// - Nichts geschieht ohne Klick: kein Zeitablauf, und Schließen („Später entscheiden“, Escape) lässt den
//   Antrag offen.

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
          // Veraltet: nichts geändert; der Grund bleibt stehen, bis der Mensch den Dialog schließt.
          if (message) message.textContent = `Nicht ausgeführt (${request.decision_label}): ${request.message ?? ""}`;
          return;
        }
        form.closest(".consent-request")?.remove();
        if (!dialog.querySelector(".consent-request")) dialog.close();
      } catch (error) {
        buttons.forEach((button) => (button.disabled = false));
        const text = error instanceof RpcError ? error.message : String(error);
        if (message) message.textContent = `Fehler: ${text}`;
      }
    });
  });
  open();
});
