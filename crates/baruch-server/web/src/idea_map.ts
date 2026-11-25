// SPDX-License-Identifier: AGPL-3.0-or-later
// Stillgelegt (Runde 34, mindmap_widget): Das Widget der Gedankenkarte ist durch `mindmap.ts` ersetzt. `main.ts`
// bindet diese Datei nicht mehr ein; sie registriert kein Widget und ruft keine RPC.
//
// Die Datei bleibt nur als leere Hülle stehen, weil `tests/round18_menueband.rs` (Besitz Integrator) sie beim
// Übersetzen per `include_str!` einliest – ohne sie ließe sich die ganze Testdatei nicht übersetzen. Der Integrator
// löscht sie zusammen mit dem dortigen Test `the_map_side_panel_starts_below_the_toolbar` (Vertrag R34 §9).
export {};
