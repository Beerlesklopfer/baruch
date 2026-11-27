# Baustein Runde 37 – hilfe_text

## CHANGELOG

Für „Unveröffentlicht“, Abschnitt „Hilfe auf der Webseite (Runde 37)“:

- **Hilfe für Anwender:** Die Webseite hat unter `/hilfe/` 14 Seiten in Alltagssprache mit Schritten zum Nachmachen
  („So geht es“): Erste Schritte · Vorhandene Texte einlesen · Menüband und Laptop-Modus · Gliederung · Schreiben,
  Bilder und Fassungen · Assistenten, Chat und Befunde · Einfälle und Mindmap · Bibel, Literatur und Quellen ·
  Suchen, Verweise, Register und Glossar · Buchsatz · Austausch, Sicherung und Sicherheit · Einstellungen · Konten
  und Lektoren · Tasten und Maus. Jede Überschrift hat einen festen Anker, jeder Abschnitt ein Bild mit alt-Text
  (105 Aufrufe von `hilfebild`, alle 94 Namen der Bildliste mit Hilfeseite verwendet). Der Einstieg nennt den
  schnellsten Weg: Text hinein, Vorschläge annehmen, PDF heraus.
- **Tasten für Windows/Linux und Mac:** `tasten.md` sammelt alle Tastenkürzel und Mausgesten in Tabellen mit beiden
  Spalten; die Mac-Tasten sind aus dem Code belegt (`ctrlKey || metaKey`, `event.code`) und als „am Mac nicht
  ausprobiert“ gekennzeichnet.
- **`docs/Bedienung.md` ist nur noch Einstieg:** Verweis auf die Hilfe, die Tabelle „Wo steht was“ (alle 44 früheren
  `##`-Überschriften → Hilfeseite mit Anker), die Regel für Entwickler („Bedienung nachziehen heißt: die Hilfeseite
  im selben Commit ändern“) und „Technische Anmerkungen“ je Hilfeseite (Routen, Cookies, Parameter, Commits,
  RPC-/MCP-Namen, Grenzen, Annahmen). Von rund 4200 auf rund 1000 Zeilen.
- **Tests:** `round37_hilfe` (11 Tests: Seiten vorhanden und ausgeschrieben, Kopf nach Vertrag, Bildnamen und
  alt-Texte, jeder gelistete Name verwendet, Anker und Bild je Abschnitt, feste Anker, Verweise, verbotene Wörter
  und Routen, Tasten mit Mac-Entsprechung, `docs/Bedienung.md` mit lückenloser Tabelle). `round24_abnahme`,
  `round32_abnahme`, `round33_abnahme` und `round33_seiten` lesen die sichtbaren Texte in der Hilfe, das Technische
  weiter in `docs/Bedienung.md`; die Verbotslisten gelten für alle Dateien unter `site/content/hilfe/`.

## Backlog/Offen

- **Verdichtet, nicht wörtlich übertragen.** Die alte Datei hatte rund 360 KB; Hilfe und technische Anmerkungen
  zusammen sind kürzer. Ich habe jeden Abschnitt gelesen und jede Aussage entweder in die Hilfe oder in die
  technischen Anmerkungen geschrieben, aber nicht Satz für Satz gegengeprüft. Wer eine Einzelheit vermisst:
  `git show 00537dd:docs/Bedienung.md` (die Tabelle „Wo steht was“ nennt den neuen Ort).
- **Alt+L (Literatur) und Alt+, (Einstellungen) am Mac:** Beide werten nur `event.key` aus (`literature.ts`,
  `settings.ts`). Am Mac erzeugt ⌥L bzw. ⌥, ein anderes Zeichen, die Kürzel greifen dort vermutlich nicht. Die Hilfe
  sagt „am Mac ungeprüft“ und nennt den Weg über das Menü. Korrektur wäre klein (zusätzlich `event.code === "KeyL"`
  bzw. `"Comma"`, wie bei Alt+E) – nicht in dieser Aufgabe, weil außerhalb des Dateibesitzes.
- **Strg+H am Mac:** `search_keys.ts` nimmt auch ⌘H an; macOS blendet mit ⌘H aber in der Regel das Fenster aus. Die
  Hilfe nennt Ctrl+H als Ausweg. Nicht am Mac geprüft.
- **Bilder auf fremden Seiten verwendet** (kein eigenes Motiv in §6; der Integrator kann `seite` in der Bildliste
  ergänzen oder eigene Namen nachtragen):
  - `erste-schritte#starten` nimmt `installation-erster-start`;
  - `erste-schritte#adresse` nimmt `gliederung-leiste` – Wunsch: Arbeitsseite mit sichtbarer Adresszeile;
  - `erste-schritte#zeitangaben` nimmt `schreiben-zeitwaechter` – Wunsch: Fassungskopf mit Datum und Uhrzeit;
  - `austausch-sicherung#sicherheit` nimmt `einstellungen-mcp`;
  - `buchsatz#typst-export` nimmt `band-buch`.
- **Bilder zweimal auf derselben Seite** (Wunsch nach eigenem Motiv): `menueband#vergroessern` (`band-ansicht`;
  Wunsch: ein vergrößerter Bereich neben einem normalen), `menueband#hilfe` (`band-menue-baruch`),
  `buchsatz#pdf` (`satz-vorschau`), `austausch-sicherung#datenordner` (`einstellungen-daten`),
  `konten#passwort` (`konten-einstellungen`).
- **alt-Texte beschreiben das erwartete Motiv.** Sie sind vor den Fotos geschrieben. Nach `--fotos` jedes Bild
  gegen seinen alt-Text halten; auffällig sind: `schreiben-zeitwaechter` (der alt-Text nennt eine Zeile
  „Abschnitt ≈ … / Ziel …“ – das Beispielwerk braucht dafür eine Zielzeit oder ist eine Predigt),
  `assistenten-websuche` (Leerstand), `abgleich-seite` (nicht verbunden), `einstellungen-mcp` (ohne lesbares Token).
- **Bildliste:** `site/data/hilfebilder.toml` hat 94 Namen, der Vertrag (§6) spricht von 95.
- **Beispielwerk:** Die Hilfe nennt das mitgelieferte Werk nur „Beispielwerk“; die Bilder zeigen „Ein Jahr auf
  Mövenoog“. Das Programm sät weiterhin „Beispielwerk: Leise Abhängigkeit (Auszug)“ (`sample.rs`) – so steht es
  auch in den technischen Anmerkungen.
- **Kommandozeile:** Die Hilfe zeigt nur die häufigsten Aufrufe (`import`, `export --pdf/--epub/--docx/--baruch`,
  `passwort`); alle Schalter und Exit-Codes stehen in den technischen Anmerkungen.

## SKILLS

- **Bedienung = Hilfeseite.** Wer Oberfläche ändert, ändert im selben Commit die Seite unter
  `site/content/hilfe/` und – für Routen, Cookies, Parameter, Commits – die „Technischen Anmerkungen“ in
  `docs/Bedienung.md`. Regeln stehen dort unter „Regel für Entwickler“; `round37_hilfe` setzt sie durch.
- **Hilfeseite schreiben:** jede Überschrift mit `{#anker}`; jede `##` mit `{{< hilfebild … >}}` oder Eintrag in
  `ohne_bild`; kein rohes HTML und kein HTML-Kommentar (Hugo bricht mit `--panicOnWarning` ab); spitze Klammern nur
  in Code (`` `<name>` ``); Verweise nur ab `/` und nur auf vorhandene Anker; Tasten als „Strg+S (Mac: ⌘S)“ in
  derselben oder der folgenden Zeile. Verboten sind u. a. „Runde “, „Revision“, „Akkordeon“, „Cookie“, „Commit“,
  „RPC“, „RAG“, „docs/“, `` `/ `` (Code, der mit einem Schrägstrich beginnt) und die Routen der Oberfläche.
- **Tests, die Doku lesen:** sichtbare Texte der Oberfläche in der Hilfeseite suchen, Technisches in
  `docs/Bedienung.md`. Überschriften der Hilfe enden mit dem Anker – `"\n## Menüband {#menueband}\n"`, nicht
  `"\n## Menüband\n"`.
- **Mac-Tasten belegen:** `grep -rn 'metaKey\|ctrlKey\|altKey' crates/baruch-server/web/src`. `ctrlKey || metaKey`
  → ⌘; Alt-Kürzel gehen am Mac nur, wenn der Code `event.code` auswertet (⌥ erzeugt sonst ein anderes Zeichen).
- **Worktree-Schutz:** Befehle mit `git` in Pipes oder mit Hier-Dokument lehnt der Schutz ab („too complex“);
  längere Prüfungen als Skript ins Scratchpad legen und einzeln aufrufen.

## Nutzerfragen

1. **AGENTS.md** (Vertrag §1): Die Zeile „Bedienung (`docs/Bedienung.md`)“ sollte künftig lauten: „Bedienung
   (`site/content/hilfe/`, Einstieg `docs/Bedienung.md`), Betrieb (`site/content/betrieb.md`)“. Bis dahin gilt die
   Regel über den Verweis in `docs/Bedienung.md`. Soll die Zeile so geändert werden?
2. **Mac:** Niemand hat die Mac-Tasten ausprobiert. Kann jemand mit einem Mac die Tabelle in `/hilfe/tasten/`
   durchgehen – vor allem Alt+L, Alt+, und ⌘H?
3. **Alt+L und Alt+, am Mac reparieren?** (siehe „Offen“) – ein Zweizeiler in `literature.ts` und `settings.ts`.
4. **Kommandozeile in der Hilfe:** Sollen die Aufrufe von `baruch-server` (einlesen, ausgeben, Passwort) in der
   Hilfe für Anwender bleiben, oder gehören sie ganz nach `/betrieb/`?
5. **Lektoren und PDF:** Die Hilfe sagt, Lektoren können das Buch zurzeit nicht als PDF abrufen (alte Datei: „Offen
   ist, ob …“). Soll das so bleiben?
