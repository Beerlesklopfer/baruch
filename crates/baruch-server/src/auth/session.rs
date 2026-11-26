// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sitzungen (Runde 35, Vertrag §1.2): Token aus 32 Zufallsbytes im Cookie [`super::SESSION_COOKIE`], gespeichert nur
//! als SHA-256; gültig 30 Tage ohne Benutzung, höchstens 90 Tage.
//!
//! Schnittstelle R35-S1: nur der Platz des Moduls; den Inhalt legt „konten“ fest.
// STUB: Sitzung anlegen, prüfen, verlängern und löschen samt Cookie (Runde 35)
