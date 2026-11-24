// SPDX-License-Identifier: AGPL-3.0-or-later
// Widgets „band“ (<header class="band"> in templates/base.html) und „band-dock“ (Marke am Ende von
// fragments/editor.html), Runde 33, Schnittstelle R33-S1; Besitz: Aufgabe „menueband“. Reiter, Zuklappen,
// Popups der Gruppen und das Andocken der Editor-Knöpfe folgen mit der Aufgabe; band.ts misst nie Breiten
// (das macht laptop.ts und meldet es über das Ereignis „baruch:laptop“). Noch ohne Verhalten.

import { register } from "./widgets.js";

register("band", () => {});
register("band-dock", () => {});
