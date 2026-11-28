<!-- translation-of: docs/Bedienung.md source-commit: c2fb812 -->

# Baruch bedienen {#using-baruch}

> Dies ist die deutsche Übersetzung von `docs/Bedienung.md` (englisches Original, maßgeblich), Stand: Commit `c2fb812`.

**Die Bedienung ist in der Hilfe auf der Website beschrieben:** <https://baruch.bernau.family/hilfe/> – im Repository die
Seiten unter [`site/content/de/hilfe/`](../../site/content/de/hilfe/); die englische Übersetzung (Runde 39b) steht unter
<https://baruch.bernau.family/en/hilfe/> mit den Seiten unter [`site/content/en/hilfe/`](../../site/content/en/hilfe/)
(gleiche Dateinamen). Sie ist die einzige Beschreibung für Nutzer, in Alltagssprache, Schritt für Schritt und mit Bildern. Der
Menüeintrag „Hilfe“ der Oberfläche öffnet die Hilfe in der Sprache der Oberfläche (`about::help_url`). Für Betreiber gilt dasselbe für
[`site/content/de/betrieb.md`](../../site/content/de/betrieb.md) (Einstiegspunkt: [`docs/Betrieb.md`](Betrieb.md)).

Diese Datei enthält jetzt nur noch:

1. die Tabelle **„Was steht wo“** – jede frühere Überschrift dieser Datei mit ihrem neuen Ort,
2. die **Regel für Entwickler**,
3. **technische Anmerkungen** je Hilfeseite: was kein Nutzer braucht (Adressen und Routen, Cookie-Namen,
   Parameter, Commits im Git-Spiegel, RPC- und MCP-Namen, Grenzen der Prüfung, Annahmen über Ungeprüftes).

Bedienschritte stehen hier nicht mehr – nie in beiden Dateien.

## Deutsche Übersetzung der Entwicklerdokumentation {#german-translation-of-the-developer-docs}

Seit Runde 39d gibt es die neun Entwicklerdokumente (diese Datei eingeschlossen) auch auf Deutsch unter `docs/de/`, veröffentlicht
unter /docs/ der deutschen Website. Die englische Datei unter `docs/` bleibt die maßgebliche Quelle: Wer sie ändert, aktualisiert die
Übersetzung im selben Commit, oder `scripts/check-doc-lag.py` meldet den Rückstand (informativ).

## Regel für Entwickler {#rule-for-developers}

„Die Bedienungsdoku aktualisieren“ (AGENTS.md: Doku wächst mit dem Code) heißt seit Runde 37: **die zuständige Hilfeseite unter
`site/content/de/hilfe/` im selben Commit ändern.** Seit Runde 39b ändert sich im selben Commit auch die englische Seite gleichen Namens unter
`site/content/en/hilfe/` (siehe „Adding a text in Round 39b“ unten). Technische Dinge
derselben Änderung gehören in die „Technischen Anmerkungen“ unten. Für die Hilfeseiten gilt:

- Alltagssprache: „Fassung“ statt „Revision“, „Assistent“ (Rolle) statt „Persona“; Fachbegriffe (MCP, Token, SWORD, Typst,
  Reverse-Proxy …) beim ersten Auftreten in einem halben Satz erklären. Keine Rundennummern, keine Dateinamen des
  Quelltexts, keine Routen (nur Adressen, die man wirklich eintippt).
- Tasten immer für beide Welten („Strg+S (Mac: ⌘S)“), die Mac-Taste am Code belegt; `tasten.md` sammelt alle.
- Jede Überschrift trägt einen festen Anker `{#…}`; jede `##`-Überschrift hat ein Bild
  (`{{< hilfebild name="…" alt="…" titel="…" >}}`, Namen nur aus `site/data/hilfebilder.toml`) oder steht im Kopf
  der Seite unter `ohne_bild`. Kein rohes HTML, auch keine HTML-Kommentare (der Bau bricht ab).
- Links nur als gewöhnliche Links ab `/`; auf `/installation/`, `/mcp/`, `/betrieb/` nur auf die Seite oder
  ihre festen Anker. Kein `doku`-Shortcode und kein Link zu GitHub in der Hilfe.
- Beispiele frei erfunden; das Beispielwerk der Hilfe heißt „Ein Jahr auf Mövenoog“ von Mara Tiedemann.
- Prüfen: `hugo --minify --panicOnWarning -s site` und `cargo test -p baruch-server --test server round37_hilfe`.

Kommentare im Rust-Code, die noch `docs/Bedienung.md „…“` nennen, meinen den Abschnitt aus der Tabelle unten.

### Einen Text lokalisieren (Runde 39a) {#localising-a-text-round-39a}

Die Oberfläche spricht Englisch (Basiskatalog) und Deutsch; die Werksprache ist eine andere Einstellung (Buch-
Ausgaben folgen dem Werk, nicht der Oberfläche). So kommt ein sichtbarer Text in die Kataloge:

1. Den Schlüssel in die Bereichsdatei deiner Aufgabe **beider** Kataloge eintragen: `crates/baruch-server/locales/en/<stem>.ftl` (natürliches
   Englisch) und `locales/de/<stem>.ftl` (der bisherige deutsche Wortlaut, Byte für Byte), an derselben Stelle. Schlüssel
   sind stabile Namen, kein Text: `<stem>.<part>.<name>` in Kleinbuchstaben (`outline.add.button = Add item`), das erste
   Segment ist der Dateiname ohne Endung. Gemeinsame Wörter (`common.save`, `common.cancel`, …) kommen aus `common.ftl`, die nur
   der Besitzer der Oberfläche ändert; ein Wort aus einem anderen Bereich wird unter dem eigenen Stamm doppelt geführt.
2. Aufrufen: in einer Vorlage `{{ t("outline.add.button") }}` (Argumente als Schlüsselwortargumente,
   `{{ t("outline.count", count=n) }}`; Zahlen bleiben Zahlen für Plurale; nie `|safe`, ein Schlüssel, der auf `_html` endet, darf
   `<code>`, `<strong>`, `<em>`, `<kbd>`, `<br>` enthalten), in Rust `i18n::tc("…", &[("name", value.into())])` (oder
   `i18n::t(lang, …)` bei bekannter Sprache), in einem Widget `t("widget.<module>.<name>")` aus `./i18n.js`. Daten
   über `ui_minutes`/`ui_day` (Filter in Vorlagen, `localtime::ui_minutes`/`ui_day` in Rust,
   `fmtMinutes`/`fmtDay` in Widgets), Zahlen mit Dezimalstellen über `ui_number`/`i18n::number`/`fmtNumber`. Der Schlüssel
   steht an der Aufrufstelle immer als vollständiges Zeichenkettenliteral.
3. Die Zeilen deiner Dateien aus `scripts/i18n-literal-allow.txt` entfernen (deutsche Literale, die der Finder noch in
   Vorlagen und Widgets sieht).
4. Die Prüfungen ausführen: `cargo test -p baruch-server --test server round39a_i18n` (gleiche Schlüssel und Platzhalter in en und de,
   kein unbekannter und kein verwaister Schlüssel, Markup nur in `_html`-Schlüsseln, der Literalfinder gegen die Erlaubnisliste) und der
   volle Lauf. Bestehende Tests bleiben deutsch: Der Prüfstand (`common::router`) sendet `Accept-Language: de`, wenn eine
   Anfrage keine Sprache nennt; `common::with_lang` fragt eine andere an.
5. In Runde 39a gefundene Fallstricke: Eine Schleifenvariable einer Vorlage namens `t` verdeckt die Funktion `t()` innerhalb der Schleife (umbenennen);
   der Text eines einfachen Schlüssels wird automatisch maskiert, daher erscheinen `/`, `"` und `'` als `&#x2f;`, `&quot;`, `&#x27;` im rohen
   HTML (Zusicherungen auf rohes HTML verwenden die maskierte Form); eine außerhalb einer Anfrage gerenderte Vorlage (Unit-Tests, direkte
   Aufrufe) ist englisch – in `i18n::sync_scope(Lang::De, …)`/`i18n::scope` einhüllen, wenn die Zusicherung Deutsch liest; der
   Widget-Katalog im Seitenkopf trägt jeden `widget.*`-Schlüssel und -Text, daher entfernt eine Prüfung auf rohe Schlüssel oder fehlende Wörter
   zuerst `<script id="baruch-i18n">`. `tests/round39a_pages.rs` geht jede GET-Seite des Routers in en und
   de durch (rohe Schlüssel, deutsche Buchstaben auf englischen Seiten außerhalb ihrer Liste bekannter Ausnahmen).

### Einen Text in Runde 39b ergänzen: Oberflächenschlüssel, Buchschlüssel, englische Hilfeseite {#adding-a-text-in-round-39b-interface-key-book-key-english-help-page}

Drei Arten von Text, drei Orte. Zu welcher ein Text gehört, hängt davon ab, wo er gelesen wird, nicht davon, wer ihn schreibt.

**Ein Oberflächenschlüssel** (Menüs, Seiten, Meldungen; folgt der Oberflächensprache `i18n::Lang`):

1. Die Bereichsdatei nach dem ersten Segment des Schlüssels wählen (`settings.typeset.field.font` liegt in `settings.ftl`) und
   die Zeile in `crates/baruch-server/locales/en/<area>.ftl` (Englisch, die Basis) und
   `crates/baruch-server/locales/de/<area>.ftl` (deutsche Übersetzung; ein bestehender deutscher Wortlaut bleibt Byte für
   Byte) an derselben Stelle ergänzen. Ein neuer Bereich ist ein neues Dateipaar (Beispiel: `readers.ftl`, Runde 39b).
2. Mit einem vollständigen Zeichenkettenliteral aufrufen (`t("…")`, `i18n::tc("…")`, `t("widget.…")`); ein zur Laufzeit gewählter Schlüssel
   (ein Enum einer anderen Crate) wird durch ein erschöpfendes `match` abgebildet, das jeden Schlüssel als Literal nennt, damit die Prüfung
   auf Verwaiste von `round39a_i18n` ihn weiter sieht (Muster: `numbering::scheme_keys`, `typeset::ui_labels`,
   `cover_layout::face_key`, `findings::status_label`, `epub_preview::profile_keys`).
3. Bezeichnungen anderer Crates (`baruch-typst`, `baruch-store`, `baruch-epub`) bleiben in ihrer Crate deutsch (der Spiegel und
   ältere Tests zitieren sie); der Server übersetzt sie nach Kennung. Ein Unit-Test hält fest, dass der Wert im deutschen Katalog
   der Bezeichnung der Crate gleicht, damit deutsche Seiten ihren Wortlaut behalten.
4. Prüfungen: `round39a_i18n` (Schlüssel, Platzhalter, Literalfinder), `round39b_literals` (Vollständigkeit der Kataloge je Bereich,
   kein Deutsch im englischen Katalog außer seinen aufgelisteten Ausnahmen, jeder Eintrag von `scripts/i18n-literal-allow.txt`
   unter einem `# reason:`-Kommentar), `round39a_pages` (jede GET-Seite in en und de).

**Ein Buchschlüssel** (Wörter, die in einem Buch landen oder als Tatsache von einem Modell zurückgelesen werden; folgt der Werksprache
`baruch_core::WorkLanguage`, nie der Oberflächensprache): `book.<area>.<name> = …` in
`crates/baruch-core/locales/en/book.ftl` und `crates/baruch-core/locales/de/book.ftl` ergänzen (eine Zeile je Eintrag, reiner
Text, Platzhalter `{ $name }`), `baruch_core::book_text::book_t(work_language, "book.…", &[…])` aufrufen und an der Aufrufstelle für
das Ausgabeformat maskieren. `crates/baruch-server/locales` bekommt nie eine `book.ftl`. Beispiel in der
Oberfläche: Das Einstellungsformular schlägt `book_t(work language, "book.draft.watermark")` als Wasserzeichen vor („DRAFT“ für
ein englisches Werk, „ENTWURF“ für ein deutsches).

**Eine englische Hilfeseite:** Die englische Seite hat genau den Dateinamen der deutschen Seite
(`site/content/de/hilfe/x.md` → `site/content/en/hilfe/x.md`), dieselben `{#anchor}`-Ids, dasselbe `weight` und dieselben
Bildaufrufe (`name`/`datei` unverändert, englische `alt` und `titel`); Links gehen auf `/en/…/`. Oberflächenwörter in
der englischen Hilfe sind die Werte aus `crates/baruch-server/locales/en/*.ftl`. Prüfungen: `hugo --minify --panicOnWarning
-s site` und `cargo test -p baruch-server --test server round39b_hilfe_en`.

## Was steht wo {#where-is-what}

Jede `##`-Überschrift der früheren Fassung dieser Datei (Stand `00537dd`, `git show 00537dd:docs/Bedienung.md`):

| Frühere Überschrift | Hilfeseite und Anker |
|---|---|
| „Starten“ | [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#starten`; Datenordner: `/betrieb/#datenordner` |
| „Typst- oder Markdown-Datei importieren“ | [`einlesen.md`](../../site/content/de/hilfe/einlesen.md) `#typst-markdown`, `#markdown`, `#teile`, `#export-import`, `#kommandozeile` |
| „PDF oder EPUB importieren (Runde 21a)“ | [`einlesen.md`](../../site/content/de/hilfe/einlesen.md) `#pdf-epub` |
| „Werke“ | [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#werke`, `#neues-werk`, `#predigt`, `#werk-bearbeiten`, `#loeschen-archiv` |
| „Bildschirm“ | [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#bildschirm` |
| „Menüband“ | [`menueband.md`](../../site/content/de/hilfe/menueband.md) `#menueband`, `#menue-baruch`, `#reiter`, `#band-einklappen`, `#kompaktes-band`, `#laptop-modus`, `#ohne-javascript`; Knöpfe des Editors: [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#einfuegen`; „Über Baruch“: [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#ueber` |
| „Zeitangaben“ | [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#zeitangaben` |
| „Adresse“ | [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#adresse` |
| „Gliederung“ | [`gliederung.md`](../../site/content/de/hilfe/gliederung.md) `#gliederung`, `#ueberschriften`, `#teile`, `#ohne-nummer`, `#seite`, `#antraege`, `#zustimmung-teil`, `#schema`, `#muster` |
| „Zeitwächter“ | [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#zeitwaechter` |
| „Satzbild“ | [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#satzbild`, `#ansichten`, `#einfuegen`, `#bild-einfuegen` |
| „Bilder“ | [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#bilder`, `#bild-einfuegen` |
| „Revisionen“ | [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#fassungen`, `#ausblenden` |
| „Vorschläge im Lesefluss“ | [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#vorschlaege` |
| „Abschnitts-Chat“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#chat` |
| „Assistenten fragen“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#pruefen`, `#websuche` |
| „Befunde (Runde 28)“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#befunde` |
| „Chat bedienen und Aufträge (Runde 28, chat-auftrag)“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#chat`, `#faeden` |
| „Suchen und Ersetzen (Runde 30a)“ | [`suchen.md`](../../site/content/de/hilfe/suchen.md) `#suchen`, `#ersetzen` |
| „Querverweise (Runde 30a)“ | [`suchen.md`](../../site/content/de/hilfe/suchen.md) `#verweise` |
| „Lösungsvorschläge und Varianten (Runde 30b)“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#pruefen`, `#loesungen`, `#rueckfrage` |
| „Chat: Wiederholen, Löschen, Anrede, Live, gegliederte Beiträge, Sprungmarken (Runde 30c)“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#chat` |
| „Bibelstellen“ | [`bibel-quellen.md`](../../site/content/de/hilfe/bibel-quellen.md) `#zitate-pruefen`, `#nachschlagen` |
| „Satz (Typst-Vorschau)“ | [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#satz`, `#satz-einstellungen` |
| „Buch als PDF, Umschlag und Titelei“ | [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#pdf`, `#umschlag`, `#epub` |
| „Stichwortzettel“ | [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#stichwortzettel` |
| „Literatur“ | [`bibel-quellen.md`](../../site/content/de/hilfe/bibel-quellen.md) `#literatur`, `#bedeutungssuche`, `#quellen`, `#nachschlagen` |
| „Einfälle“ | [`einfaelle.md`](../../site/content/de/hilfe/einfaelle.md) `#festhalten`, `#liste`, `#einfall` |
| „Mindmap“ | [`einfaelle.md`](../../site/content/de/hilfe/einfaelle.md) `#mindmap`, `#wolke`, `#mindmap-tasten`, `#mindmap-gliederung`, `#mindmap-austausch` |
| „Fäden“ | [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#faeden` |
| „Abgleich mit MultiGPT“ | [`bibel-quellen.md`](../../site/content/de/hilfe/bibel-quellen.md) `#abgleich` |
| „Vergrößern mit Strg+Mausrad“ | [`menueband.md`](../../site/content/de/hilfe/menueband.md) `#vergroessern` |
| „Einstellungen“ | [`einstellungen.md`](../../site/content/de/hilfe/einstellungen.md) `#oeffnen`, `#darstellung`, `#werk`, `#assistenten`, `#verbindungen`, `#quellen`, `#mcp-zugang`, `#daten`; Satz: [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#satz-einstellungen`; Bibeln (SWORD), Kommentare und Lexika: [`bibel-quellen.md`](../../site/content/de/hilfe/bibel-quellen.md) `#bibeln-waehlen` |
| „Sicherheit“ | [`austausch-sicherung.md`](../../site/content/de/hilfe/austausch-sicherung.md) `#sicherheit`; Betrieb hinter einem Proxy: `/betrieb/#reverse-proxy` |
| „Konten und Rollen (Runde 35)“ | [`konten.md`](../../site/content/de/hilfe/konten.md) `#einrichten`, `#anmelden`, `#lektoren`, `#lektor-rechte`, `#passwort` |
| „Austausch zwischen Instanzen“ | [`austausch-sicherung.md`](../../site/content/de/hilfe/austausch-sicherung.md) `#ausgeben`, `#einlesen`, `#instanz` |
| „Umschlag-Editor (Runde 27)“ | [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#umschlag` |
| „Runde 19 – RAG-Pflege, Reste der Stellen“ | [`bibel-quellen.md`](../../site/content/de/hilfe/bibel-quellen.md) `#bedeutungssuche`, `#zitate-pruefen`, `#nachschlagen`; [`einstellungen.md`](../../site/content/de/hilfe/einstellungen.md) `#verbindungen` |
| „EPUB (Runde 20)“ | [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#epub` |
| „Runde 31 – Register und Glossar“ | [`suchen.md`](../../site/content/de/hilfe/suchen.md) `#glossar`, `#register` |
| „Runde 22 – Revisionen und Spiegel“ | [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#fassungen`, `#verlauf`; Rückfrage: [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#rueckfrage`; Archiv: [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#loeschen-archiv`; Entwurf: [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#pdf`; Austausch: [`austausch-sicherung.md`](../../site/content/de/hilfe/austausch-sicherung.md) `#einlesen` |
| „Runde 23 – Härtung“ | nach Inhalt verteilt: [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#einfuegen`, `#fassungen`; [`gliederung.md`](../../site/content/de/hilfe/gliederung.md) `#antraege`; [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#chat`, `#befunde`, `#websuche`; [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#umschlag`, `#epub`; [`einstellungen.md`](../../site/content/de/hilfe/einstellungen.md) `#verbindungen`, `#mcp-zugang`; [`austausch-sicherung.md`](../../site/content/de/hilfe/austausch-sicherung.md) `#sicherung`, `#datenordner` |
| „Runde 32 – Word und Beenden“ | [`einlesen.md`](../../site/content/de/hilfe/einlesen.md) `#word`; [`buchsatz.md`](../../site/content/de/hilfe/buchsatz.md) `#word-export`; [`erste-schritte.md`](../../site/content/de/hilfe/erste-schritte.md) `#beenden` |
| „Runde 36 – Reste“ | nach Inhalt verteilt: [`konten.md`](../../site/content/de/hilfe/konten.md) `#lektor-rechte`, `#passwort`, `#anmelden`; [`einfaelle.md`](../../site/content/de/hilfe/einfaelle.md) `#mindmap-austausch`; [`schreiben.md`](../../site/content/de/hilfe/schreiben.md) `#ausblenden`, `#einfuegen`; [`assistenten.md`](../../site/content/de/hilfe/assistenten.md) `#rueckfrage`, `#pruefen`; [`menueband.md`](../../site/content/de/hilfe/menueband.md) `#laptop-modus`; [`austausch-sicherung.md`](../../site/content/de/hilfe/austausch-sicherung.md) `#sicherung`, `#datenordner`; [`gliederung.md`](../../site/content/de/hilfe/gliederung.md) `#gliederung` |

Alle Tastenkürzel und Mausgesten sind zusätzlich in
[`tasten.md`](../../site/content/de/hilfe/tasten.md) gesammelt; der Einstiegspunkt ist [`_index.md`](../../site/content/de/hilfe/_index.md).

## Technische Anmerkungen {#technical-notes}

Je Hilfeseite, was dort bewusst fehlt. Schnittstellen im Einzelnen: `docs/RPC.md`, `docs/MCP.md`; Aufbau:
`docs/Architektur.md`; Betrieb: `docs/Betrieb.md`; Verteilung und Starter: `docs/Verteilung.md`.

### „Erste Schritte“ (`erste-schritte.md`) {#erste-schritte-erste-schrittemd}

- **Aufruf:**

  ```
  baruch-server [serve] [--listen 127.0.0.1:8765] [--data ORDNER] [--secrets keyring|memory] [--allow-host NAME]…
  baruch-server --version
  baruch-server import DATEI.typ [--title "TITEL"] [--data ORDNER]
  baruch-server export ID|SLUG [--data ORDNER]
  ```

  Der Befehl (`serve`, `import`, `export`) steht vor den Optionen. `--version` gibt `baruch-server <Version>` aus.
  `--listen` auch als `BARUCH_LISTEN` (Standard `127.0.0.1:8765`, derselbe Port wie beim Benutzerdienst aus
  `packaging/systemd/`), `--data` auch als `BARUCH_DATA`. `--allow-host NAME` (wiederholbar, auch
  `BARUCH_ALLOWED_HOSTS`, kommagetrennt) erlaubt zusätzlich einen Rechnernamen für den Betrieb hinter einem Reverse-Proxy
  (`docs/Betrieb.md`).
- **Datenordner:** `ORDNER/baruch.sqlite`, Git-Spiegel jedes Werks in `ORDNER/works/<slug>/`. Standard ohne `--data`:
  Linux `~/.local/share/baruch` (oder `$XDG_DATA_HOME/baruch`), macOS `~/Library/Application Support/baruch`,
  Windows `C:\Users\<name>\AppData\Local\baruch` – bewusst **Local**, nicht Roaming: Auf Servern gespeicherte Profile einer
  Domäne würden sonst die SQLite-Datenbank (einschließlich WAL) und den Git-Spiegel bei jeder An- und Abmeldung hin- und herkopieren.
  Legt Baruch den Datenordner selbst an, bekommt er unter Linux und macOS die Rechte 0700; einen bestehenden Ordner
  ändert Baruch nicht (wer `--data` außerhalb des eigenen Home-Verzeichnisses setzt, prüft die Rechte selbst – Datenbank und
  Spiegel enthalten das ganze Manuskript).
- **Geheimnisse** der Verbindungen (Schlüssel, Token, Passwort) liegen nur beim
  Standard-Datenordner im Schlüsselbund des Systems (Dienst `baruch`). Mit jedem anderen `--data` bleiben sie im Speicher des Prozesses und
  sind am Ende weg – so kann ein Wegwerf- oder Testserver die Einträge `baruch/…` des
  eigentlichen Dienstes weder lesen noch überschreiben oder löschen. `--secrets keyring` (`BARUCH_SECRETS=keyring`) nimmt ausdrücklich den
  Schlüsselbund, `--secrets memory` den Speicher; die Startausgabe nennt, was gilt („Geheimnisse: …“).
- **Beispielwerk:** Eine wirklich neue Datenbank bekommt „Beispielwerk: Leise Abhängigkeit (Auszug)“; ein
  bestehender Datenordner wird nicht geändert. Die Hilfe und ihre Bilder zeigen stattdessen das erfundene Werk
  „Ein Jahr auf Mövenoog“ (Bilderliste `site/data/hilfebilder.toml`).
- **Werkwahl:** Adresse `/?work=<id>`; Cookie `baruch_work` (ein Jahr). `/` ohne `?work=` öffnet das gemerkte
  Werk, sonst das älteste, sonst die Werkliste. Links und Formulare tragen das Werk mit.
- **Seiten:** Werkliste `/werke`; Anlegen/Importieren `/werke/neu` (mit JavaScript der Dialog „Importieren…“);
  Ändern `/werke/<id>/bearbeiten`; Löschen `/werke/<id>/loeschen`; Archiv `/werke/archiv`; Export
  `/werke/<id>/export` (lädt `<slug>.typ` herunter, wie `baruch-server export`); „Über Baruch“ `/ueber`, Lizenztext
  `/ueber/lizenz` (reiner Text).
- **Neues Werk:** Die Werkvorlage eines neuen Buchs enthält `#set document(…)` und
  `#set heading(numbering: "1.1")`; Commit „Werk angelegt: <Titel>“. Ein Predigttext aus SWORD steht als
  `#quote(block: true, attribution: [Joh 3,16 (Modul)])[…]`; fehlende Verse meldet `sermon_text.missing`; eine
  ungültige Stelle antwortet ohne JavaScript mit Status 422. Auf `/werke/neu` gilt bei leerer Bibel das Standardwerk,
  im Dialog das gezeigte Werk.
  Runde 39c: Das Formular hat ein Auswahlfeld „Werksprache“ (`language`, Deutsch oder Englisch), das mit der aktuellen
  Oberflächensprache vorbelegt ist; die Wahl ist die Werksprache und bestimmt die Wörter der vorbelegten Kapitel („Erstes Kapitel“ oder
  „First chapter“, die sieben Predigtteile). Ein Formular ohne das Feld (und `work.create` ohne `language`) bedeutet Deutsch.
- **Eindeutige Titel:** Meldung `Es gibt schon ein Werk „…“ (slug).` Hatten zwei Werke vor dem Update denselben
  Titel, bekommt eines beim ersten Start einen neuen: das angelegte Beispielwerk wieder seinen Beispieltitel, sonst das
  jüngere „<Titel> (2)“. Ordnernamen, Spiegel und Texte bleiben.
- **Ändern:** Commit „Werk geändert: <Felder>“, wenn sich eine Datei ändert (z. B. `vorlage.typ`). `#set`, `#show`, `#let` …
  gehören in die Werkvorlage.
- **Löschen:** letzter Commit „Werk gelöscht: <Titel>“, der Spiegel wandert nach
  `ORDNER/works/.geloescht/<slug>-<JJJJ-MM-TT>/`. Der Ordnername (`slug`) bleibt dauerhaft reserviert (neues Werk:
  `<slug>-2`); Werknummern werden nie wieder vergeben. Die Einfälle stehen sofort im Spiegel der Einfälle ohne
  Werk (`<data>/einfaelle/einfaelle.md`); beim Start bringt Baruch diesen Spiegel auf den neuesten Stand (kein Commit ohne Änderung).
- **Wiederherstellen aus dem Archiv:** Der Ordner wandert zurück nach `works/<slug>`, Commit „Werk wiederhergestellt“.
  Über MCP gibt es das nicht.
- **Adresse:** `/?work=<id>&key=<schlüssel>&rev=<nummer>[&view=preview][&pick=…][&lq=<suche>]`. `key` ist der feste
  Schlüssel des Abschnitts (das MCP-Feld `url` nennt dieselbe Adresse). Ältere Adressen mit `node=<stelle>` funktionieren
  weiter; fehlt der Schlüssel, zeigt Baruch den Abschnitt der Stelle `node` oder den Standardabschnitt und
  „Abschnitt mit Schlüssel K gibt es nicht (mehr).“ `rev` ist die Nummer der Revision (mit Lücken nach dem Löschen),
  nicht ihre Position. `pick=<abschnitt>:<nummer>[,…]` setzt probeweise andere Revisionen in die Vorschau;
  ungültige Einträge werden übersprungen. Ohne `work` das Standardwerk (das älteste), ein unbekanntes Werk 404.
  Mit `?lq=` steht unter den Treffern der Kasten „Bilder des Werks“, auch ohne JavaScript.
- **Zeiten:** Ortszeit seit der Nutzerentscheidung vom 2026-11-07 (davor UTC); gespeichert und über RPC/MCP geliefert
  wird weiterhin UTC.
- **„Über Baruch“:** Lizenz `AGPL-3.0-or-later`; Quelltext am Tag `v<Version>`; Website
  `https://baruch.bernau.family/`; `THIRD-PARTY-LICENSES.txt` je Paket (liegt die Datei neben dem Programm, steht ihr
  Pfad dort). Ein aus dem Quelltext gebautes Programm hat die Datei nicht; die Seite nennt den Befehl
  (`scripts/third-party-licenses.py`). Im AppImage liegt der Pfad im eingehängten Image und gilt nur, solange
  Baruch läuft. Nur über das APT-Repository (Debian, Ubuntu) kommen neue Versionen mit `sudo apt upgrade`.
- **Beenden:**

  | Route | Zweck |
  |---|---|
  | `GET /beenden` | Bestätigung mit einem Formular; nicht vom eigenen Rechner 403 mit einer Erklärung |
  | `POST /beenden` | hinter `guard::same_origin`; nur vom eigenen Rechner (auch nicht unter einem `--allow-host`-Namen), sonst 403; Antwort 200 mit der Schlussseite, 500 ms später fährt Baruch herunter |

  Mit JavaScript ruft die Schlussseite `window.close()` auf; Browser erlauben das nur für Fenster, die ein Skript
  oder der Starter als App-Fenster geöffnet hat. Die Starter (`baruch`, AppImage, macOS-Programm,
  Windows-Startmenü) öffnen Baruch daher mit einem Chromium-artigen Browser als `--app`-Fenster;
  `BARUCH_BROWSER=standard` nimmt immer den Standardbrowser, `BARUCH_NO_BROWSER=1` öffnet keinen (Stand der Prüfung
  je Plattform: `docs/Verteilung.md`, „Quitting Baruch“). Als Dienst (`systemd --user`, `Restart=on-failure`)
  bleibt Baruch nach dem Beenden aus. `systemctl --user stop`, `kill` (SIGTERM) und unter Windows Strg+Pause oder
  das Schließen des Konsolenfensters fahren ebenso sauber herunter; letzte Zeile „Baruch beendet.“, Status 0. Baruch lässt sich über RPC
  und MCP nicht beenden.

### „Vorhandene Texte einlesen“ (`einlesen.md`) {#vorhandene-texte-einlesen-einlesenmd}

- **Typst-Import, Regeln:** `=` Kapitel, `==` Abschnitt, `===` Unterabschnitt; der Text einschließlich der Überschriftszeile wird
  Revision 1. Titel und Autor aus `#set document(title: …, author: …)`; auskommentierte `#set document(…)` (`//`,
  `/* … */`) zählen nicht. Titel vergeben: Exit 1 mit `Es gibt schon ein Werk „…“ (slug). Mit --title "…" einen
  anderen Titel wählen.` Ordnername (`slug`) aus dem Titel, höchstens 40 Zeichen, an der letzten Wortgrenze gekürzt
  („Mövenoog – ein ganzes Jahr auf der kleinen Insel“ → `moevenoog-ein-ganzes-jahr-auf-der`), bei Gleichheit `-2`,
  `-3` …; bestehende Werke behalten ihren Ordnernamen.
- **Spiegel nach dem Import:** Commit „Import aus <datei>“ mit `vorlage.typ`, `main.typ` (Werkvorlage + `#include`
  aller Abschnitte) und `abschnitte/NNN-titel.typ`, jeweils mit der ersten Zeile `#import "../vorlage.typ": *` (nur im
  Spiegel, nicht in der Revision; so gelten auch eigene Funktionen wie `#szene` dort). Werke ohne Vorlage haben
  weder `vorlage.typ` noch die Zeile. Ausgabe: Werk, Ordnername, Zahl der Abschnitte, Spiegelordner, Commit.
- **Abgelehnt:** dieselbe Datei ein zweites Mal (auch umbenannt oder mit CRLF; stderr, Exit 1); eine FIFO, ein
  Gerät oder ein Ordner („… ist keine Datei“, ohne zu warten); kein UTF-8; über 16 MiB. Ein BOM bleibt in der
  Werkvorlage, die erste Überschrift wird trotzdem ein Kapitel.
- **Keine Gliederung** sind Überschriftszeilen in Roh-Blöcken, in mehrzeiligem Inline-Roh, in Blockkommentaren, in
  mehrzeiligen Zeichenketten im Code (`#let x = "…"`) und in mehrzeiligen Inhaltsblöcken aus Code (`#block[…]`,
  `#figure(caption: […])`, `#let x = […]`, `#show …: it => […]`). Ein Kommentar hinter einer Überschrift
  (`== Titel // Notiz`) gehört nicht zum Titel. Bekannte Grenze: eingerückte Überschriften, `====` und tiefer und
  `#heading(...)` werden nicht erkannt – außer den Markern `<baruch-part>` und `<baruch-unnumbered>` (genau in
  der dokumentierten Form; `// …` darf dahinter stehen; anderer Abstand, ein anderes Label oder eine Einrückung machen
  sie zu Text oder Werkvorlage). Ein weiteres `#heading(...)` vor der ersten Überschrift bleibt Werkvorlage.
  Bestehende Werke werden für die Marker nicht umgeschrieben.
- **Export auf der Kommandozeile:** `baruch-server export <id|slug>` schreibt die Werkvorlage + aktuellen Texte nach
  stdout (ohne die `#import`-Zeilen des Spiegels); unmittelbar nach dem Import Byte für Byte die Eingabe.
  `--out ORDNER` schreibt die Typst-Datei (unter ihrem Importnamen, sonst `<slug>.typ`) und jedes Bild an seinen
  Pfad (ohne `./` und ein führendes `/`), byteidentisch. Vor der ersten Datei prüft der Export: eine vorhandene Zieldatei,
  einen Zwischenordner als Link oder Datei, zwei Bildpfade, die zugleich Datei und Ordner sind (`a.png` und `a.png/b.svg`)
  → Exit 1 mit einem Grund, nichts geschrieben.
- **Bilder beim Import (CLI):** Was Vorlage und Text mit `image("…")` nennen, wird gelesen (auch in
  `#figure(image("…"))`), relativ zum Ordner der Datei; `./bilder/a.png`, `bilder//a.png` und `/bilder/a.png` sind
  dasselbe Bild `bilder/a.png` (der Pfad im Text bleibt, wie er geschrieben ist). Übersprungen: außerhalb des
  Ordners (über `..` oder einen Symlink), Pfade mit `..`, fehlend, über 8 MiB, kein Bild – je eine Zeile „Bild
  übersprungen: <pfad> (<grund>)“ auf stderr; Ausgabe „Bilder: n“; zusammen höchstens 64 MiB je Datei.
  Das Hochladen einer einzelnen Datei (`/werke/import`) und `work.import` bringen nur den Text.
- **Export → Import, Hinweiscodes** (CLI: `Hinweis (<code>): …` auf stderr): `import-typeset-adopted` (der Block
  `// Baruch: Satz` wird eine Satzeinstellung), `import-typeset-kept` (von Hand geändert, bleibt Werkvorlage; die
  Bundzugabe kommt als Teil des inneren Randes zurück), Show-Regeln `// Baruch: Überschriften ohne Nummer` → Merkmal
  „Nummer aus“ (auch für eine Überschrift mit eigenem Label und unter „Ohne Nummern“), `import-back-matter-cut` (ab
  `// Baruch: Register und Glossar`, nur wenn der erzeugte Block dahinter steht, mit Zeile),
  `import-back-matter-kept`, `import-projection-removed` (ohne Glossarseite trägt die erste Erwähnung den Vermerk
  `/*baruch-erste-erwaehnung*/#emph[Wort]`).
- **Markdown → Typst:** `#`/`##`/`###` → `=`/`==`/`===`, `####`–`######` auf `===` begrenzt; YAML-Kopf → `#set
  document(…)`; `#emph[…]`, `#strong[…]`, `#strike[…]`; `#quote(block: true)[…]`; `#raw("…")`; `#link("…")[Text]`;
  `#footnote[…]`; `#image("…", alt: "…")` oder `#figure(…, caption: […])`; `#table(…)`; `/ Begriff: Erklärung`;
  `<br>` wird ein Umbruch, HTML-Blöcke stehen als Kommentar. Wörtlich bleiben `#`, `*`, `_`, `$`, `@`, `<`, `[`, `]`,
  `` ` ``, `\`, `~` und `=`/`-`/`+`/`/` am Zeilenanfang. Dateien, die die Hauptdatei nicht nennt: `archive-unused`.
  Abgelehnte Archive: Links, `..`, absolute Pfade, doppelte Namen, Zip-Bomben. CLI-Ausgabe:
  `Hinweis (<code>, Zeile n): …`.
- **EPUB auf der Kommandozeile:** `baruch-server export <id|slug> --epub DATEI [--data ORDNER]`; eine vorhandene Datei
  Exit 1; `--epub` zusammen mit `--out`, `--pdf`, `--stichwort` oder `--baruch` Exit 2.
- **PDF/EPUB:** Seite `/werke/import/quelle`; lokal `pdftotext`/`pdfinfo` (Paket `poppler-utils`); EPUB-Fußnoten
  werden `#footnote[…]`, die Zahl am Anfang entfällt nur als Nummer der Fußnotenmarke (Hinweis „Fußnotennummer
  entfernt“; „1 Kor 13,4“ bleibt). Laut seiner Werkzeugbeschreibung nimmt MultiGPT kein EPUB an (eine Annahme aus der
  Beschreibung, nicht gegen einen echten Dienst geprüft). Entwürfe in `import-drafts/`. Kommandozeile mit
  `--data DIR`; ohne `--uebernehmen` und `--bibliothek` schreibt sie nichts, ein vermuteter Teil wird dort ein
  Kapitel; beide Schalter mit `.typ`/`.md`/`.zip` → Exit 2.
- **Word:** Seite `/werke/import/word`; Vorschläge unter `@word:<name>` (Modell „word“); Name: Kleinbuchstaben, ä→ae, ö→oe,
  ü→ue, ß→ss, alles andere ein Bindestrich, höchstens 40 Zeichen, ohne Namen `@word:unbekannt`. Überschriften aus den
  Formatvorlagen „Überschrift 1–3“/„Heading 1–3“ (auch abgeleitete Formatvorlagen und die Gliederungsebene des Absatzes);
  Tabellen als Typst-Tabelle (eine Insel im Satzbild); Bilder PNG, JPEG, GIF, WebP, SVG. Aus einer von Baruch geschriebenen Datei
  wird `@lektor` zu `@word:lektor`; Typst-Inseln kommen wörtlich zurück (Test
  `round32_abnahme`). Hinweiscodes:

  | Code | Bedeutung |
  |---|---|
  | `docx_ueberschrift_tief` | Überschrift 4–6 steht fett im Abschnitt |
  | `docx_text_vor_ueberschrift` | Text vor der ersten Überschrift in einem eigenen ersten Kapitel |
  | `docx_ueberschrift_eingefuegt`, `docx_ueberschrift_geloescht` | vom Bearbeiter eingefügte oder gelöschte Überschrift |
  | `docx_vorspann_eingefuegt` | eingefügter Text vor der ersten Überschrift – nicht übernommen |
  | `docx_formataenderung` | nachverfolgte Formatänderungen: Der Endstand gilt |
  | `docx_verschoben` | verschobener Text als Löschung und Einfügung |
  | `docx_textfeld`, `docx_objekt` | Textfeld, Zeichnung, eingebettetes Objekt – nicht übernommen |
  | `docx_formel` | Formel nur als reiner Text |
  | `docx_tabelle_vereinfacht` | verbundene Zellen, Zeilen ungleicher Länge, eingefügte/gelöschte Tabellenzeilen |
  | `docx_bild_format`, `docx_bild_extern`, `docx_bild_uebersprungen` | Format nicht unterstützt (EMF/WMF), nur verknüpft, zu groß |
  | `docx_endnote`, `docx_fussnote_absaetze` | Endnoten als Fußnoten; mehrere Absätze einer Fußnote als Zeilen |
  | `docx_verborgener_text`, `docx_symbol`, `docx_unbekannt` | verborgener Text, Symbolschriften, unbekanntes Element |
  | `docx_kommentar_ohne_stelle` | Kommentar am ersten Abschnitt |
  | `docx_gliederung_abweichend`, `docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt` | ein Vorschlag oder Beitrag konnte nicht angelegt werden (das Werk existiert trotzdem) |

  Kommandozeile: `baruch-server import DATEI.docx [--uebernehmen] [--title "TITEL"] [--data ORDNER]`; eine nicht lesbare
  Datei Exit 1.
- Einen Import über MCP gibt es nicht (kein Typst, PDF, EPUB, Word).

### „Menüband und Laptop-Modus“ (`menueband.md`) {#menüband-und-laptop-modus-menuebandmd}

- Das Menüband ersetzt die frühere obere Leiste mit den Menüs „Werk“, „Ansicht“ und „Werkzeuge“ und die frühere
  Werkzeugleiste über dem Blatt; kein Eintrag ist entfallen. Umbenannt: „Satz“ → „Satz-Vorschau“, „Als
  PDF/Entwurf/EPUB herunterladen“ → „PDF“, „Entwurf“, „EPUB“; „Editor“ außerhalb der Werkseite → „Zum Text“.
- **Adressen:** Reiter `?tab=start|einfuegen|pruefen|buch|ansicht`, Menüband `?band=…` (Cookie `baruch_band`),
  Laptop-Modus `?laptop=on|off|auto` (Cookie `baruch_laptop`, ein Jahr), Leisten `?lrail=1`, `?rrail=1` (Cookies
  `baruch_lrail`, `baruch_rrail`); Schubladen ohne JavaScript über die Sprungziele `#pane-tree` und
  `#pane-assistants`. Ohne JavaScript sind Menü und Gruppen `<details>`.
- **Ziele:** neues Werk `/werke/neu`; Einstellungen ohne JavaScript `/einstellungen/darstellung`; Suche `/suche`;
  Querverweise `/verweise`; Literatur `/literatur` (auf der Werkseite das Popup); Abgleich `/abgleich`;
  „Literatur“ als Zurück-Gruppe auf `/quelle`. Die Satzseite trägt im Reiter „Buch“ die Gruppen „Zurück“,
  „Herunterladen“, „Seiten“ („‹ Seite n von N ›“) und „Satzlauf“ („Satz abbrechen“).
- **Marken** stehen im gelieferten HTML jeder Seite; das Skript frischt sie nur auf.
- **Zoom:** Stufen im Cookie `baruch_zoom`; Strg+Mausrad wertet nur `ctrlKey` aus (am Mac also Strg, nicht ⌘),
  Strg+0 auch `metaKey`.
- **Hilfe-Eintrag:** ein gewöhnlicher Link auf `about::HELP_URL` (`https://baruch.bernau.family/hilfe/`), neues
  Fenster, ohne `nur-autor` (Lektoren sehen ihn auch); keine Route, kein Eintrag in `roles`.
- **K** und **F** heißen für Screenreader „Hervorheben“ und „Fett“. In der Auswahlliste „Absatzformat“ gehören die
  Pfeiltasten der Liste.
- Früher setzte Baruch die drei Spalten unter 900 px untereinander; das ist entfallen.

### „Gliederung“ (`gliederung.md`) {#gliederung-gliederungmd}

- **Schreibweise im Text:** Teil = `#heading(level: 1, numbering: none)[Titel] <baruch-part>`, Kapitel `= `,
  Abschnitt `== `, Unterabschnitt `=== `; ein Punkt ohne Nummer kann als
  `#heading(level: N, numbering: none)[Titel] <baruch-unnumbered>` stehen. Ändern sich Ebene oder Titel, bekommt der
  Abschnitt eine neue Revision vom Menschen, die sich nur in der Überschriftszeile unterscheidet (Label `<…>`
  und Kommentar `// …` bleiben). Jede Änderung der Gliederung ist genau ein Commit („Gliederung: …“).
- **Assistenten:** Anlegen über MCP `outline_add` (Commit mit ihrem Namen; Teil mit `level: 0`); Umbenennen,
  Verschieben, Ebene ändern, Nummer an/aus, Löschen nur als Antrag. Bei einer Predigt sagt der Antrag „n vorhandene
  Predigtteile wandern unter den neuen Teil“. Setzt ein Assistent mitten in einem Teil einen Teil vor ein Kapitel
  (ohne vorhandene Kapitel zu übernehmen, zählt das als Anlegen), stehen die folgenden Kapitel bis zum nächsten Teil
  unter dem neuen Teil.
- **Markdown-Überschriften** (seit 2026-11-07): 1–6 Rauten und ein Leerzeichen am Zeilenanfang. Nicht umgewandelt:
  `\# …`, eine Raute in Anführungszeichen (`'# …'`, `"# …"`), Typst-Code (`#heading(…)`, `#set`, `#let`), eingerückte
  Zeilen, Zeilen in Roh-Blöcken, Kommentaren und Inhaltsblöcken, die Werkvorlage. Vorschläge der Assistenten bleiben
  wie sie sind; beim Übernehmen gilt dieselbe Regel (der Vorschlag bleibt im Revisionsbaum, gefolgt von der
  umgewandelten Revision). „Fassung ändern“ wandelt um wie „Speichern“.
- **Titelregeln, Meldungen:** „Ein Gliederungspunkt braucht einen Titel.“, „Der Titel darf keinen Zeilenumbruch
  enthalten.“, „Der Titel ist zu lang (höchstens 200 Zeichen, hier N).“ Geprüft wird nur ein geänderter Titel
  (auch bei `revision.save`, Umschalten, Übernehmen, jeder Überschrift, die den Abschnitt teilen würde); ein Punkt
  `== ` ohne Titel bleibt. Import und Export bleiben byteidentisch.
- **Löschen:** Der Commit nennt den Text der aktuellen Revision und was sonst verloren geht – jede andere Revision
  (offene Vorschläge getrennt gezählt) und jeder Beitrag im Wortlaut.
- **Ohne JavaScript:** Gliederungsverwaltung `/gliederung`, Bearbeiten `/gliederung/bearbeiten`; mit JavaScript
  sind die Rückfall-Links verborgen. Die Bestätigung für Teilen/Zusammenführen ist ohne JavaScript eine eigene Seite
  mit „Zurück“.
- **Teile, Einzelheiten:** Umbenennen schreibt die Markerzeile um (ein Kommentar dahinter bleibt; ein Teil mit
  leerem Text bekommt den Marker). Ein Teil ohne Marker (selbst geschriebener Text) behält seinen Text;
  nur der Titel ändert sich, die Meldung sagt das. Ein anderer Titel im Marker benennt den Teil um, auch
  beim Umschalten auf eine ältere Revision; eine Revision ohne Marker schaltet ohne Gliederungsabgleich um.
- **Ältere Teile (Nutzerentscheidung 2026-11-07):** Beim ersten Start wandelt Baruch einmalig alle Teile, deren
  aktuelle Revision leer ist oder nur aus der alten Zeile „= Teil …“ besteht (genau eine `=`-Zeile ohne Label und
  Kommentar, Titel der des Teils oder mit „Teil“ beginnend), in den Marker um: eine neue aktuelle Revision vom
  Menschen mit dem Vermerk „Umwandlung Runde 14 (Nutzerentscheidung)“, ein Commit je Werk „Umwandlung Runde 14
  (Nutzerentscheidung): n Teile …“; die alte Revision bleibt umschaltbar. Teile mit anderem Text bleiben; stderr
  nennt die Anzahl und die Knoten-Ids. Merkmal `parts_marked_r14` in `store_meta`; eine neue Datenbank setzt es sofort
  (ihr Beispielwerk behält seine leeren Teile).
- **Ohne Nummer:** Commit `Gliederung: „…“ ohne Nummer` (`Baruch-Action: outline-numbered`), RPC
  `outline.set_numbered`. Ändert man die Ebene über die Gliederung, bleibt der Marker (nur `level:` ändert sich).
  In Export, Spiegel und Satz steht ein Punkt mit „Nummer aus“ als
  `#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>` – in allen Schemata mit Nummern; der
  gespeicherte Text bleibt (`= Vorwort`). Eine Überschrift mit eigenem Label (`= Vorwort <vorwort>`) bekommt stattdessen
  `#show heading.where(…): set heading(numbering: none)` hinter der Vorlage. Bei „Ohne Nummern“ bleibt alles
  wie gespeichert.
- **Anträge, Commits:** Bestätigen = ein Commit mit dem Menschen als Autor und Committer, Trailer `Baruch-Origin`,
  `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`, Betreff „… (Antrag N)“. Abgelehnt oder veraltet
  (Nutzerentscheidung 2026-11-08, jede Art von Antrag): ein leerer Commit „Gliederung: Antrag N abgelehnt“ oder
  „… veraltet“, Trailer `Baruch-Decision: rejected|stale`, `Baruch-Requested-By`, `Baruch-Origin`,
  `Baruch-Request`, `Baruch-Decided-By`, im Text Antragsteller, Zusammenfassung, Begründung, Grund. Dasselbe, ob
  über Formular, Zustimmungsdialog oder RPC. Zustimmung zu einem Teil: Revision 1 des Teils trägt den Assistenten.
  Die Hinweisleiste verlinkt auf `/gliederung#antraege`; Vorschau `/gliederung/antraege/ID/vorschau`. Predigten nennen
  „Predigtteil II.“ in der Vorschau.
- **Umschalten mit anderer Ebene:** ein Commit („… auf Fassung n umgeschaltet · Ebene 1 → 2“, im Text die
  mit umgestuften Punkte); jeder mit umgestufte Unterpunkt bekommt eine neue Revision vom Menschen. Abgelehnt: eine Revision
  ohne Überschrift, mit weiteren Überschriften, mit Text vor der Überschrift, mit einer Ebene, die an dieser
  Stelle nicht möglich ist (z. B. der erste Punkt als Abschnitt). „Fassung ändern“ und `revision.save` der aktuellen Revision
  verlangen genau eine Überschrift der Ebene des Punkts; eine nicht aktuelle Revision lässt sich frei ändern.
  Assistenten können keine Vorschläge mit anderer Ebene anlegen.
- **Escapes** in Überschriften (`\'`, `\#`, `\*` …) zeigt die Gliederung aufgelöst („Anna's Laube“);
  Dateinamen im Spiegel folgen dem aufgelösten Titel und der Position. Der Export (`work.export`) ist nach Umordnungen gültiges Typst in neuer Reihenfolge,
  byteidentisch zur Importdatei ohne Änderung.
- **Gliederungsschema:** Seite `/werke/<id>/nummerierung`; Vorschau per GET; Commit `Gliederung:
  Gliederungsschema „Juristisch“`; ein ungültiges Muster 422, ein unverändertes Schema 409. Im Export folgt der Vorlage eine
  Zeile `// Baruch: Gliederungsschema „…“` und eine Regel `#set heading(numbering: …)` (bei „Aus der
  Werkvorlage“ nichts); für alle dreizehn Schemata geprüft. Die gespeicherte Vorlage (`vorlage.typ`) bleibt. Im
  Kopf der Gliederung: „I|1“ → „Nur Kapitel römisch“, „1.1“ → „Dezimal (DIN 1421)“, sonst das Muster, das rohe
  Muster im Tooltip; Werte `vorlage`, `keine`.
- **Mustersprache:** Ein Muster mit genau einem Zählersymbol zeigt nur die eigene Nummer der Ebene, eines mit mehreren
  zählt hierarchisch; Ebenen jenseits der Liste haben keine Nummer; `;` und `|` sind reserviert.
- **Nummerierung aus der Werkvorlage:** Es zählt die letzte Regel `#set heading(numbering: …)` der obersten Ebene.
  Ein Textmuster gilt wörtlich wie in Typst (`"1;"` ergibt „2;“, `aa` sind zwei Zählersymbole). Eine Funktion wird
  je Ebene berechnet, wenn sie nur aus Bedingungen über die Tiefe besteht (`n.pos().len()` mit Vergleichen, auch über
  `let p = n.pos()`), `if`/`else`, Text, `numbering("…", ..n)`, `numbering("…", p.at(i))` (`p.first()`,
  `p.last()`), `str(p.at(i))`, `* 2` und `+` – direkt oder über eine `#let`-Funktion. Beispiel:
  `(..n) => if n.pos().len() == 1 { "Kapitel " + str(n.pos().at(0)) } else { numbering("1.1", ..n) }` ergibt
  „Kapitel 3“ · „3.2“ · „3.2.1“. Die Regel, die Baruch für „Teile und Kapitel“, „Kapitel ausgeschrieben“,
  „Juristisch“, „Predigt“, „Buch“ exportiert, wird beim Einlesen wieder dieses Schema (danach zählt „Aus
  der Werkvorlage“ als das Schema). Nicht berechenbar: das erste `numbering("…")` der Funktion für alle Ebenen.
- **Erwähnungen im Text** („im dreizehnten Kapitel“, „Kapitel XLII“) erkennt Baruch bis zur hundertsten
  Ordnungszahl und bei römischen Zahlen in der üblichen Schreibweise, nur wenn genau ein Punkt gemeint sein kann.

### „Schreiben, Bilder und Fassungen“ (`schreiben.md`) {#schreiben-bilder-und-fassungen-schreibenmd}

- **Satzbild:** Der Quelltext bleibt Typst; vor dem Speichern wird das Satzbild in Typst übersetzt, danach gelten die
  Regeln des Quelltexts. Der Server liefert das Satzbild fertig (kein Umweg über den Quelltext); Abschnitte über
  64 KiB kommen ohne fertiges Satzbild. Gemerkte Ansicht: Cookie `baruch_editor`. Kommt man über einen Treffer der
  Literatursuche (`#fundstelle`), startet der Editor im Quelltext; bei geöffnetem Satzbild wird die Fundstelle dort gelb
  hervorgehoben (ändert den Text nicht, verschwindet beim Tippen). Beschreibung von Blöcken, Inseln und RPC:
  `docs/WYSIWYG.md`.
- **Überschrift im Satzbild:** Rücktaste am Anfang eines Absatzes direkt hinter einer Überschrift und Entf am
  Ende einer Überschrift ziehen den Absatz nicht in die Überschrift (ein leerer Absatz entfällt). Das Satzbild
  prüft die Titelregeln mit derselben Meldung wie der Server. Das Satzbild bietet keine Teile an (der Server lehnt
  einen neuen Teil aus dem Satzbild ab); für Teile und Punkte ohne Nummer lässt sich kein Absatzformat wählen,
  der Marker wird erneut gespeichert (ein Kommentar dahinter bleibt).
- **Blöcke:** `#szene[…]`, `#arbeit[…]` als Kasten; Argumente (`#szene(ort: […])`) und ein Label `<name>` in der
  nächsten Zeile stehen klein daneben und bleiben erhalten. `#image("…")` und `#figure(image("…"), caption: […])`
  erscheinen als Bild, sobald das Bild unter diesem Pfad angelegt ist (auch mit `./`); ein Block mit mehreren
  `image(…)` zeigt alle. `1. …` nummerierte Liste, `/ Begriff: Erklärung` Begriffsliste, `\` erzwungener Umbruch
  (↵), `#link("…")[Text]`, `@ziel` als Knopf (ein Klick zeigt das Label; Ändern nur im Quelltext). Inseln:
  `#let`, `#set`, `#show`, eigene Funktionen, Formeln, Rohtext, Kommentare, Listen mit gemischten Markern;
  das Verlassen des Inselfelds nach einer Änderung übernimmt sie ebenfalls. Bibelstelle als Insel `#bibelstelle[…]` (nur
  wenn die Vorlage `#let bibelstelle(…)` oder `#let bibel(…)` definiert, mit dem Namen der Vorlage),
  Literaturverweis `@schlüssel` (nur mit `#bibliography(…)`); Szene nur mit `#let szene`, Werkzuordnung nur mit
  `#let arbeit`; Begriff `/ Begriff: …`, Umbruch `\` am Zeilenende. Im Quelltext beginnen Blöcke an einem
  Zeilenanfang.
- **Zitat-Knopf:** Bis Runde 35 fügte er eine Zeile „– “ ein, die ungefüllt als einzelner Gedankenstrich im Buch stand;
  seit Runde 36 entfällt sie. Einen Beleg direkt unter einem Zitatblock zeigt das
  Satzbild als Zitat und Absatz (keine Code-Insel).
- **Speichern:** Seit 2026-11-27 normales Speichern ohne Größen- und Zeitgrenze (davor: eine neue Revision nach
  30 Minuten Pause oder ab 20 % Änderung). Der Dialog „Seite speichern“ des Browsers erscheint auf der
  Werkseite nicht. Eine eingeschaltete Zeitgrenze zählt ab dem letzten Schreiben der Revision: Die erste kleine
  Korrektur an einem älteren Abschnitt, der nie ergänzt wurde, ergibt eine neue Revision („Zeitgrenze
  überschritten“). Der Editor schickt den geladenen Stand mit; bei einem anderen Stand entsteht eine neue Revision („nicht
  auf der aktuellen Fassung bearbeitet“). Jede Ergänzung ist ein Commit. Rückmeldung mit JavaScript als
  Hinweis unten, ohne es im Editorkopf. Alle anderen Schreibwege (Bild einfügen, Registermarke, Verweis, Ersetzen,
  Gliederung) erzeugen neue Revisionen. Wird das Speichern ohne JavaScript abgelehnt (z. B. ungültige Überschriftenebene),
  zeigt Baruch die Meldung und den Text in einem Textfeld („Erneut speichern“). Die Bestätigung „Teilen/Zusammenführen und
  speichern“ behält den Speichermodus; der Kopf nennt danach die Revision und den Grund („Überschrift geändert“).
- **„Einstellungen → Werk → Fassungen“** (Anker `#fassungen`): Ergänzen an/aus, Prozent 1–100 (Standard 20),
  Minuten 0–1440 (0 = keine Zeitgrenze, Standard 30); nur in der Datenbank (nicht im Austauschformat), jede
  Änderung ein Protokoll-Commit.
- **Verlauf:** `/werke/<id>/fassungen/verlauf`; jedes Rückgängig ist ein Commit. „Fassung ändern“ an der aktuellen
  oder einer ergänzten Revision ist ein Schritt im Verlauf; wurde sie inzwischen in einem anderen Tab ergänzt,
  lehnt Baruch ab („Veraltet: …“, das Formular an `/act` reicht `based_on_amend` durch).
- **Weitere Revisionen:** Link `?rev=`, „aktuell machen“ ohne JavaScript als Formular an `/act`.
- **Revisionen:** Eine neue Revision hat die gezeigte Revision als Elternteil. Der Server prüft das Löschen selbst (ohne das Kontrollkästchen
  „Löschen bitte bestätigen.“); Kinder hängen danach am Elternteil; unbekannte Nummer: „Fassung 3 gibt es nicht;
  vorhanden: 1, 2, 4“. „In der Vorschau probieren“ = `pick` in der Adresse. Jedes Übernehmen, Umschalten,
  Speichern, Ändern und Löschen ist genau ein Commit (`docs/Architektur.md`); Ändern und Löschen schreiben
  davor/danach oder den gelöschten Text in die Commit-Nachricht. Scheitert der Spiegel, bleibt die Änderung
  gespeichert; der nächste Commit holt auf.
- **Ausblenden:** Adresse `&fassungen=alle`; die Marke gilt je Abschnitt, steht nur in der Datenbank (nicht im
  Austauschformat), kein Commit; die Aktion gibt es nur an der aktuellen Revision und darunter; nach dem Umschalten auf
  eine ältere Revision gilt eine höhere Marke nur bis unter der neuen aktuellen. Werkweit: Marke an der letzten
  Revision vor dem Tag (höchstens die aktuelle), ersetzt vorhandene Marken. Über MCP nicht möglich. Der Zähler im
  Menüband zeigt dasselbe wie der im Editorkopf.
- **Vorschläge im Lesefluss:** Linien mit mindestens 3 : 1 Kontrast (WCAG 1.4.11); auf dem Blatt, das auch im dunklen
  Theme hell bleibt (#f4f1ea), gilt die helle Linie. Einklappen: Cookie `baruch_folded`; ohne JavaScript klappt der
  Browser selbst ein (nicht über Neuladen hinweg). Autoren ohne Persona-Eintrag: `@mcp:<name>`. Ein umformulierter
  Absatz neben einem neuen erscheint als zwei Stücke („geändert“, „eingefügt“).
- **Bilder:** Seite `/werke/<id>/bilder`, Suche `?q=…` (Groß-/Kleinschreibung egal, `%` und `_` wörtlich), Vorschau
  `…/vorschau?b=320` (lädt beim Einblenden), ein Bild `/werke/<id>/bilder/<name>[?fassung=n]` mit SHA-256, Bytes
  `/werke/<id>/bilder/<name>/datei[?fassung=n]`, nach Pfad `/werke/<id>/bilder/pfad?p=<Pfad>` (jede Schreibweise,
  nie `..`), Anträge `#antraege`, vorgeschlagener Inhalt `/werke/<id>/bilder/antraege/<nr>/datei` (30 Tage nach
  der Entscheidung entfernt, dann HTTP 410). Leerer Name = aus dem Dateinamen (Kleinbuchstaben, ungültige Zeichen `-`, bei
  Kollision `-2` vor der Endung); „SVG als Base64 speichern“ („(Base64)“ in der Liste). Spiegel: `bilder/<name>`
  oder `bilder/<name>.b64` (76 Zeichen je Zeile), Commit „Bild <name>: …“. Anpassen der Endung: `haus.svg` →
  `haus.png` (vergeben: `haus-2.png`), Pfade bleiben, im EPUB heißt die Datei z. B. `haus.svg.png`. Pfadkollision:
  Commit „Pfad-Kollision … aufgelöst“. Grenzen für Assistenten je Werk in 24 Stunden: 30 Anlagen/Ersetzungsanträge,
  zusammen 64 MiB, höchstens 10 offene Ersetzungsanträge.
- **Bild einfügen:** `#figure(image("<Pfad>", alt: "…"), caption: [...])` wird als der Mensch eingefügt (ein Commit;
  der Text der Bildunterschrift wird maskiert). Ohne Pfad bekommt das Bild `bilder/<name>` zusammen mit der Revision.
  Cursorposition im Quelltext: die nächste Zeilengrenze, nie in ein mehrzeiliges Konstrukt (`#footnote[…]`,
  `#link(…)[…]`). Kapitelbild: erkannt aus einer wörtlichen Liste wie `#let kapitelbilder = ("bilder/a.png", …)` und
  `image(kapitelbilder.at(counter(heading).get().first() - 1))`, wenn Kapitel die erste Ebene sind; sonst
  „Kapitelbild fehlt: <Pfad>“ oder „Bild aus der Vorlage“.
- **Zeitwächter:** Sekunden = Wörter × 60 ÷ Wörter je Minute, aufgerundet; Gesamtzeit = Summe der Punkte. Nur in
  der Datenbank, kein Commit. RPC `work.timing`, `work.timing_set`, `timing.count`; MCP `work_timing` (nur lesend).
  Zählregeln (`baruch_typst::timing::spoken_words`): (1) CRLF → LF; (2) Kommentare `/* … */` und `//` am
  Zeilenanfang oder nach Leerraum zählen nicht; (3) Überschriftszeilen und Teilmarker nicht; (4) Rohtext
  und Mathe `$…$` nicht; (5) `#name`/`#name.feld` samt Argumenten entfallen, folgende Inhaltsblöcke `[…]` werden
  gesprochen – außer `#footnote` und `#arbeit`; Zeilen mit `#set`, `#show`, `#let`, `#import`, `#include` entfallen
  ganz; (6) Labels `<…>` und Verweise `@name` entfallen; (7) `*`, `_` entfallen, `~` wird ein Leerzeichen,
  Listenmarker am Zeilenanfang entfallen, `\x` wird `x`; (8) ein Wort ist eine Folge ohne Leerraum mit
  mindestens einem Buchstaben oder einer Ziffer. Beispiel: `#quote(block: true)[Im Anfang war das Wort.] <zitat>`
  = 5 Wörter = 3 Sekunden bei 110.

### „Assistenten, Chat und Befunde“ (`assistenten.md`) {#assistenten-chat-und-befunde-assistentenmd}

- **Begriffe:** In Code, RPC und MCP heißen die Assistenten „persona“, der Kurzname „handle“. Baruch ruft das Modell
  der Persona über das Modell-Backend Eliezer auf („Einstellungen → Verbindungen“).
- **Aufträge:** Formulare ohne JavaScript an `/assistenten/act`; Fehler zeigen eine Hinweisseite (statt einer
  Fehlerseite) mit „Zurück zum Abschnitt“, im Chat je Persona, die nicht antwortete; bei „komprimiert“ der
  Hinweis und „Weiter zum Abschnitt“. Mit JavaScript die schwebende Meldung (Toast) unten rechts, die stehen bleibt.
  Mit Gemma dauert ein Aufruf etwa 25 s. Fragt Baruch erneut an, verschwindet, was bisher gezeigt wurde („Antwort
  wird neu angefordert (…)“).
- **Länge der Antworten:** höchstens 700 (Frage), 900 (Prüfen), 500 (Chat) Tokens, bei einem Vorschlag nach der Länge des
  Abschnitts (höchstens 4096); `BARUCH_MAX_TOKENS` setzt eine kleinere Obergrenze. Abgeschnitten: „… (Antwort
  abgeschnitten: Längengrenze erreicht)“.
- **Warteschlange:** Eliezer bearbeitet einen Auftrag zur Zeit (sonst lädt es ein Modell doppelt; die Grafikkarte
  des Entwicklungsrechners hat 22 GB), acht warten. Die Kontextlänge der Persona muss zu der passen, mit der
  Eliezer das Modell lädt (Gemma: 32768); Baruch fordert selbst keine an.
- **Lektor:** In der Erprobung seit 2026-11-06 fest `@lektor` (alle Personas auf einem Modell); die Marke sagt
  in ihrem Titel „fest gewählt“ oder „nach größtem Kontext“. Gedächtnis: Einfälle zum Abschnitt und bis zu zehn offene
  des Werks ohne Bezug. Befunde der anderen: die letzten drei je Abschnitt.
- **Prompt-Regeln:** verbotene Anfänge „Als Assistent“, „Als KI“, „Als Sprachmodell“, „Gerne“, „Natürlich“
  (beginnt auch die zweite Antwort so, entfällt ihr erster Satz); Bibelverse im Wortlaut nur von `@schrift`;
  der Hörer liest in der Ich-Perspektive der Zielgruppe. In einem Vorschlag entfernt Baruch Zeilen, die mit „Kritisch:“,
  „Was trägt:“, „Vorschlag:“, „Jetzt wichtig:“, „Kann warten:“ beginnen, einen Codeblock um den Text, und prüft dort
  auf keine verbotenen Anfänge. Der Vermerk „Komprimiert“ steht in der Notiz eines Vorschlags, nie im Buchtext;
  Notiz für ein Kapitel „Vorschlag von @lektor (ganzes Kapitel)“, der Kopf nennt „beruht auf Fassung n“.
- **Lektor-Standard:** verlangt „Was trägt:“ nur bei einer Prüfung; im Chat antwortet der Lektor zuerst auf die Nachricht
  (Regel 8 des Prompts); ein eigener Rollen-Prompt bleibt unberührt (bei der manuellen Prüfung auf den Ton achten).
  „Chat nur Abschnitt“ gilt für alle Werke und steht nur in der Datenbank; die Persona bekommt dann den Abschnitt,
  Titel und Gliederung (nur Nummern und Titel). Prüfen und Fragen im Bereich „Assistenten“ sehen weiterhin das Kapitel.
- **Werkseite ohne JavaScript:** In der Gliederung stehen die Pfeile und „Bearbeiten“ in einer eigenen Zeile
  unter dem Eintrag; in einer schmalen Spalte rutscht die Marke „k offen“ unter den Titel. Die Liste „Zitate prüfen“
  hängt an der ganzen Statuszeile.
- **Erkennung von „Kritisch:“** (Pflicht nur noch für Nathan als Prüfer): eine Zeile, die nach Listenmarkern mit „Kritisch:“ beginnt,
  oder „Kritisch:“ direkt hinter dem Verweis des Teils („- III. Zuspruch: Kritisch: …“, fett, mit dem Zusatz
  „(Schluss)“ oder nur „II. Kritisch: …“); „Unkritisch:“ oder „… ist kritisch: …“ zählen nicht. Fehlt sie nach
  einer Nachfrage, steht unter dem Beitrag ein Hinweis von Baruch. Lösung in fester Form:
  `- statt: „Zitat“`, `- besser: „…“` oder `- Schritt: …`; Fortschritt „fehlender Lösungsvorschlag“; bei einer
  abgeschnittenen Antwort keine Nachfrage.
- **Prüfauftrag erkannt** an einem Wort, das mit „prüf“, „überprüf“, „nachprüf“, „gegenprüf“, „kontrollier“,
  „check“ oder „review“ beginnt, oder an einer Befundnummer („[B12]“). Nur eigene, offene Befunde; fremde,
  unbekannte oder entschiedene Nummern überspringt Baruch; keine doppelte Rückfrage. Sieht die Persona alles
  umgesetzt, sagt sie „Alle meine Befunde zu diesem Abschnitt scheinen erledigt.“ (dann kein Pflichtbefund).
  „Ja, erledigt“ setzt „erledigt ohne Änderung“ (Grund „laut @persona umgesetzt …“); beides ein Commit.
  Übernommene Befunde zeigen „übernommen in Fassung 3 · Stand 2“.
- **Kritikzyklus:** auch über MCP `ask_persona` mit `rev`. Zeigt die aktuelle Revision nur die Überschrift und
  gibt es einen Vorschlag mit Text, prüfen alle den Vorschlag (Hinweis „aktuelle Fassung enthält nur die
  Überschrift – Vorschlag n prüfen?“ bei einer ausdrücklichen Wahl). Nur der Lektor kommentiert fremde Befunde; die
  anderen sehen sie als „schon gemeldet“.
- **Chat:** neue Marke im LocalStorage `baruch_chat_seen` (eigene Beiträge nie neu; erster Besuch: fremde Beiträge
  seit dem letzten Besuch eines beliebigen Abschnitts des Werks; leerer Chat: Merkmal „war leer“; verglichen werden nur
  Serverzeiten). Ganzer Verlauf `/beitraege` (Anker `#chat-neueste`), Beitrag `/beitrag`, Fäden `/faeden?work=…`,
  Faden `/faden?…`. Vervollständigung ohne JavaScript über `datalist`; am Satzanfang nach zwei Buchstaben die
  Anrede („Le“ → „Lektor, “). Erwähnungen: höchstens drei, jede nur einmal, Groß-/Kleinschreibung egal; inaktive oder
  unbekannte werden übersprungen; RFC hebt die Grenze auf. Live: eine Abfrage alle drei Sekunden.
- **Commits:** Löschen eines Beitrags (trägt den ganzen Text; in einem Faden „Fadenbeitrag n: gelöscht“), Bearbeiten
  („Beitrag n: bearbeitet“ mit davor/danach), Umwandeln in einen Einfall. Fäden stehen nie im Spiegel: Das Bearbeiten
  eines Fadenbeitrags ergibt keinen Commit (die alte Revision nur in der Datenbank und im Austauschformat). Nach dem
  Verschieben stehen die offenen Befunde am Zielabschnitt, übernommene bleiben am alten.
- **MCP:** nur eigene Beiträge (`post_edit`, Verschieben, Löschen, Umwandeln in einen Einfall; `docs/MCP.md`). `chat` ohne
  Abschnitt legt einen Faden an; eine Relay-Stufe, höchstens drei Antworten je Nachricht, jede Persona einmal; je
  Client höchstens zwei Aufträge zugleich. Alle Clients ohne Token teilen sich je Werk 300 Beiträge in 24 Stunden,
  100 offene Vorschläge und vier gleichzeitige Modellaufträge (je Name weiterhin zwei). Relay-Antworten werden bei ↻
  mit derselben Anrede wiederholt. Ältere Antworten ohne gespeicherten Auftrag: ↻ wiederholt die letzte
  Nachricht des Menschen davor als Chat.
- **Sprungmarken im Prompt:** `[key:105] 2.3 Titel`; Personas nennen Stellen als `[[key:105]]` oder
  `[[key:105|„Zitat“]]`; ungeprüfte bleiben Text mit „Sprungmarke ungeprüft: …“. HTML aus einer Antwort wird nie
  ausgeführt. Der Schlusshinweis („Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft“) steht als Fußzeile.
- **Befunde:** Seite `/werke/<id>/befunde`; Datei `befunde.md` im Spiegel (nach Teil, Kapitel, Abschnitt, dann
  Fäden); jede Statusänderung, jeder Vorschlag, jede Entscheidung ein Commit. „Angehängt“: Inhaltswörter ab drei
  Buchstaben ohne Füllwörter, dazu jede Zahl, Dice-Maß ab 0,8. Die Matrix zählt höchstens die neuesten 2000 („Gekappt:
  …“); die Liste „Offen, älter als n Tage“ auf 200 begrenzt. Gemeinsame Vorschläge zählen nicht in „k offen“; sind alle
  Ursprünge entschieden: „veraltet (alle Ursprünge entschieden)“. Hinspringen über `find` in der Adresse. Beim
  ersten Start mit dieser Funktion wertet Baruch einmalig alle vorhandenen Persona-Antworten aus. Die Austauschdatei
  (`.baruch`, Nebenversion 4) trägt Befunde einschließlich ihres Verlaufs (`docs/Austauschformat.md`). Lösungen ohne JavaScript an
  `/loesungen/act`; Varianten `/varianten` (Geschwister im Revisionsbaum; lange Abschnitte als Teilvorschlag).
  Mindmap: `?befunde=1`.
- **Websuche:** MultiGPT-Werkzeuge `web_search` (dahinter SearXNG) und `fetch_url`; die ersten drei Rechner der
  Quellenliste je mit „site:<Rechner> <Anfrage>“; [W1]…[W4], höchstens 1 500 Zeichen je Seite; Seiten über 1 MiB
  oder ohne Antwort nach 10 s übersprungen; die ganze Websuche höchstens 30 s. Zitatprüfung: ganze Wörter; mit
  einer Auslassung „…“ mindestens drei Wörter je Stück, höchstens 400 Zeichen übersprungen; „(unbekannte Quelle)“,
  „(nicht abgerufen)“; letzte Zeile „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“. Nicht abgerufen werden
  `localhost`, 127.x, 10.x, 192.168.x, 172.16–31.x, 169.254.x, 100.64–127.x, `fc00::/7`, `fe80::/10`, Namen, die auf
  `.local`, `.internal`, `.home.arpa`, `.localhost` enden, Namen ohne Punkt, Adressen mit Benutzerinformation, andere
  Schemata; Baruch fragt kein DNS (ein öffentlicher Name, der auf eine private Adresse zeigt, wird nicht erkannt). Belegseite
  `/websuche/belege`; ein übernommener Beleg = ein Titel der Art „web“ mit dem Vermerk „Abgerufen am … (Websuche @belege)“.
  Kein MCP-Werkzeug; über `ask_persona`/`chat` sucht @belege nicht im Web. Ein Vorschlag bekommt keine Websuche.
  Annahme: Namen und Parameter der MultiGPT-Werkzeuge wie in `connections/multigpt.rs`; nur der Nutzer prüft gegen einen echten
  MultiGPT-Dienst und eine echte Suchmaschine.

### „Bibel, Literatur und Quellen“ (`bibel-quellen.md`) {#bibel-literatur-und-quellen-bibel-quellenmd}

- **Verszählung:** die Verszählung „German“ des SWORD-Projekts (Schlachter 1951, `docs/Architektur.md`): „Ps 51,21“,
  „Neh 3,38“, „1. Kön 5,32“, „Dan 3,33“, „3. Joh 15“. Baruch kennt alle 18 Verszählungen des SWORD-Projekts
  (Luther, NRSV, Vulg, MT …). Abbildung über RPC `scripture.map` oder MCP `scripture_map`, Standard KJV → German
  („Joel 2,28–32“ → „Joel 3,1–5“, „Mal 4,1–6“ → „Mal 3,19–24“, „Ps 51,1“ → „Ps 51,3“), nur über eine geprüfte
  Regeltabelle. Nicht abgebildet: die Psalmen der griechisch-lateinischen Verszählungen (LXX, Vulgata, Synodal); für
  Catholic, Catholic2, Segond und Calvin gibt es keine geprüften Regeln (anders gezählte Bücher bleiben
  „Versifikation passt nicht“).
- **Module:** Baruch liest SWORD-Module selbst (Treiber zText, RawText; Kompression ZIP, LZSS, BZIP2, XZ;
  Kommentare RawCom, RawCom4, zCom, zCom4; Lexika RawLD, RawLD4, zLD), ohne libsword. Nicht lesbar: HREFCom,
  RawFiles, unbekannte Verszählung. Ein Block, der auf mehr als 64 MiB entpacken würde, wird abgelehnt.
  Modulordner = Ordner mit `mods.d/` und `modules/`; Standard `$SWORD_PATH`, dann `~/.sword`, dann
  `/usr/share/sword`; ein Modulname zählt nur einmal (der frühere Ordner gewinnt). `DataPath` relativ zum Modulordner,
  `..` abgelehnt. Freie Module: CrossWire (`https://www.crosswire.org/sword/modules/`), Installer `installmgr`
  (Paket `sword-utils`), BibleTime, Xiphos; oder ein Modul-ZIP entpacken
  (`https://www.crosswire.org/ftpmirror/pub/sword/packages/rawzip/`); eigene Module mit `osis2mod`.
  Lizenzhinweis aus `DistributionLicense`, `Copyright` der conf. Verschlüsselt = `CipherKey=` in der conf;
  Schlüssel im Schlüsselbund unter `sword-cipher:<modul>`, nie in der conf, der Datenbank oder einer Seite. Gespeichert:
  Modulordner für alle Werke, die Bibel je Werk (Migration 0030), Kommentare/Lexika je Werk (Migration 0037, Formular
  an `POST /einstellungen/werk/bibeln`). RPC `bible.set_dirs`, `bible.set_work`, `bible.cipher_key_set`,
  `bible.cipher_key_delete`, `bible.modules`, `bible.set_work_sources`, `bible.work_sources`, `bible.check`,
  `bible.lookup`, `bible.search`, `bible.marks`, `scripture.check`, `commentary.lookup/search`,
  `lexicon.lookup/keys/search`; MCP nur lesend `bible_modules`, `bible_lookup`, `bible_search`, `scripture_check`,
  `commentary_lookup`, `lexicon_lookup`, `lexicon_search`.
- **Wortlautprüfung:** Gelesen wird reiner Text ohne Fußnoten, Querverweise und Überschriften (kanonische
  Psalmüberschriften bleiben). Zitatformen: „…“, “…”, »…«, "…", `#quote[…]`, Blockzitat
  `#quote(block: true, attribution: [Joh 3,16])[…]` (Stelle in der Attribution, am Ende oder direkt dahinter;
  auch in `#szene[…]`). Zwischen Stelle und Zitat nur Leerraum, Klammern, Satzzeichen, weitere Stellen derselben
  Liste oder kurze einleitende Wörter mit Doppelpunkt. Innere Anführungszeichen und mehrere Absätze (wiederholtes
  Anführungszeichen oder bis zum schließenden Zeichen) zählen; Grenzen: ein Zitat gleicher Art innerhalb eines Zitats, ein vergessenes
  schließendes Zeichen vor einem Absatz mit demselben Zeichen. Normalisiert: Satzzeichen, Groß-/Kleinschreibung, Bindestriche,
  Trennungen, Apostrophe, Leerraum, typografische Zeichen, Typst-Auszeichnung (`_…_`, `*…*`, Escapes,
  Fußnoten im Zitat), zerlegte Umlaute. Steht die Stelle am Anfang eines Absatzes und das Zitat dahinter, gehört Letzteres zur
  Stelle. Trennungsreste im Modul („die ses“): ein zusätzlicher Vergleich ohne Leerraum → „Wortlaut stimmt (Leerraum im
  Modul)“, toleriert nur zusätzliche Wortgrenzen des Moduls; ein Hinweis bei auffallend vielen Wortfragmenten.
- **Stellenerkennung, Einzelheiten:** Bücher mit einem Kapitel („Jud 3“ = Jud 1,3); „4. Mo 25,19“, „5. Mose 6,4“,
  „4Mo 1,1“; „Joh 3, 1. Joh 4“ sind zwei Stellen; nur mit einer Verszahl „Am“, „Mal“, „Mi“, „Ob“, „Hab“, „Ri“, „Tit“,
  „Da“, „Hi“ („Ob“ nur mit Kapitel und Vers oder einem Bereich). Keine Stelle vor einem Zahlwort („Johannes 3 Mal“,
  „Psalm 23 Jahre“, „3x“, „Stück“, „Tage“, „Uhr“) – nur ohne ausdrücklichen Vers: „Joh 3,16 Mal 3“ bleibt
  Joh 3,16, „Joh 3,16 Mal 3,20“ sind zwei Stellen; eine Tageszeit nach einem Wochentag (Mo–So, Stunde 0–23, Minute 00–59; im
  Stellenfeld bleibt „Mi 7:18“ Micha). „Vers im selben Satz“: höchstens 120 Zeichen, kein Satzende, keine
  Leerzeile, keine andere Stelle dazwischen (auch „Verse 23–24“, „V. 23“); ein anderes Kapitel ohne Buchnamen
  beendet die Suche. Englische Verszählungen: „Mal 4,1“, „2. Kor 13,14“ ungültig. Predigttext: bis zu 400 Verse
  vollständig, ab 401 „Nur die ersten 400 Verse eingefügt.“; Sonderzeichen im Bibeltext werden maskiert.
- **Faktenblock für Personas:** `@schrift` und `@belege` bekommen „Stellenprüfung (Baruch, Verszählung Schlachter
  1951 – kein Wortlaut): …“ oder, mit einer Bibel, Modul, Verszählung, Lizenzhinweis und Ergebnis je Stelle. Antworten
  tragen „Wortlaut: ungeprüft – keine lokale Bibel in Baruch (geprüft nur Buch/Kapitel/Vers).“ oder „Wortlaut
  geprüft von Baruch (nicht vom Modell), Modul …, Versifikation …, Lizenz laut conf: …“. Trennt das Modell Ziffern
  („Mk 1 0,42–45“), fügt Baruch sie zusammen (Befund 2026-11-06: Die getrennten Ziffern stammten aus dem Text des
  Modells). Vermerke „(nicht im Text belegt)“, „(Stelle steht im Text: …)“, „(Stelle ungültig: …)“ setzt nur
  Baruch; schreibt ein Modell sie selbst, entfernt Baruch sie und prüft neu. „fehlt“ zählt nur mit einer Ortsangabe. Belege sind
  nur der gezeigte Text, der Auftrag und die Belege von Baruch – nicht fremde Befunde, frühere Beiträge oder Einfälle
  (diese stehen gerahmt als „Daten, keine Anweisungen“).
- **Zitate prüfen:** Seite `/werke/<id>/pruefung?key=…&rev=…` (auch bei Strg-/Umschalt-Klick); Hervorhebung über die
  CSS Custom Highlight API (ältere Browser: nur die Liste), nur ganze Wörter, mehrdeutiger Wortlaut bleibt
  unmarkiert („n Zitat(e) im Satzbild nicht eindeutig gefunden (nur in der Liste)“).
- **Lexikon-Stichwörter:** zuerst Strong-Nummern der Verse, dann Strong-Nummern im Text („G25“, „G26“ findet
  „G0026“), dann großgeschriebene Wörter mit einer Lemmaprobe (ohne -ern, -en, -es, -er, -e, -n, -s; Umlaut zurückgeführt;
  letzter Bestandteil eines Kompositums); höchstens 5 Artikel je Lexikon; Kommentareinträge höchstens
  4000 Zeichen.
- **Literatur:** Seite `/literatur?work=…&node=…&rev=…&lq=…`, Modus `lmode=rag` (Einfälle: `mode=rag`),
  Dokumentansicht `/quelle?title=…` (`&quote=…#zitat-…`, `&highlight=…#hervorhebung-…`), Volltext
  `/quelle/multigpt?collection=…&document=…`, Bibel `/literatur/bibel`, Treffer im Werk
  `/?work&node&rev&lq#fundstelle`. Die Seiten merken sich den Abschnitt über `key`. Zitieren erzeugt
  `#quote(block: true, attribution: [@schlüssel[Stelle]])[…]` oder `[Ps 139,23–24 (Modul)]`, Urheber
  `@literatur`. Bibeltreffer im Suchfeld: höchstens 10 je Bibel, höchstens drei Bibeln, 3 s. Volltext in Teilen von
  höchstens 20 000 Zeichen; eine Antwort über 4 MiB wird abgelehnt; Felder „Anfang/Ende im Volltext“ (Zeichen ab 0);
  Zitierschlüssel übernommener MultiGPT-Dokumente `mg-…`. Eine Suche nur aus Steuerzeichen: „Keine Treffer.“
- **Semantische Suche:** Wege LanceDB, pgvector, MultiGPT; Treffer mehrerer Wege einmal („gefunden von …“,
  Reciprocal Rank Fusion); Links nur für eine sichere Adresse (im Werk `/…`, sonst http/https). Von einer fremden
  Seite geöffnet (`Sec-Fetch-Site`/`Origin`), sucht Baruch nicht nach Bedeutung und lädt keinen Volltext.
  Höchstens vier Suchen fragen die entfernten Wege zugleich an (danach nach 20 s „Zu viele gleichzeitige Suchen …“).
  `weak` in `rag.search` und MCP `rag_search`. Weitere Einzelheiten unter „Einstellungen“.
- **Abgleich:** Seite `/abgleich`. Dokumenttitel „<Nummer> <Titel>“; ein Teil und Abschnitte vor dem ersten
  Kapitel je ein eigenes Dokument; die Sammlung wird beim Einschalten festgelegt. Scheitert das Löschen des
  alten Dokuments, löscht es der nächste Abgleich. Der Abgleich lädt keine Beiträge hoch (anders als im Plan;
  im Backlog vermerkt). Geholte Dokumente: Art `remote`; 30 s je Anfrage; unvollständige Listen löschen keine
  lokale Kopie; Berechtigungen `docs.write`/`docs.read`; ohne Dokument-Id ein Hinweis statt erneut hochzuladen; verlorene
  Antworten meldet der nächste Abgleich; „Verbindung prüfen“ beendet seine Sitzung bei MultiGPT. Annahme: Werkzeuge
  `search_documents`, `read_document`, `list_collections`, `document_info`, `list_documents`, `upload_document`,
  `delete_document` mit den Parametern aus `connections/multigpt.rs`; gegen eine Attrappe geprüft.

### „Einfälle und Mindmap“ (`einfaelle.md`) {#einfälle-und-mindmap-einfaellemd}

- **Seiten:** Liste `/einfaelle` (`?work=<id>|none|all&word=&status=open|done|all&q=&deleted=1`, Bedeutung
  `&mode=rag`), ein Einfall `/einfaelle/{id}`, Mindmap `/werke/{id}/karte` (alte Lesezeichen der Ideenkarte
  funktionieren weiter; `?befunde=1`, eingeklappt `zu=<Kennungen>`), Gliederungsansicht `/werke/{id}/karte/gliederung`,
  Einlesen `/werke/{id}/karte/einlesen`.
- **Einfall:** Doppelte Stichwörter (auch in unterschiedlicher Groß-/Kleinschreibung) fallen zusammen; kein Zeilenumbruch
  in einem Stichwort. Quelle aus der Literatur: „Literatur: <Titel>“. Der Server prüft das Löschen. Verarbeiten: Titel aus
  den Stichwörtern („Laube, Rosen“, höchstens 200 Zeichen, Titelregeln der Gliederung); der Punkt und „verarbeitet“
  entstehen in einem Schritt; ein Beitrag mit dem Autor „Autor“ oder „Notiz“, die Auswahl als `> …`, Quelle „Einfall <id>“
  oder „Einfall 3 von @belege“ / „… von @mcp:claude-code“ (der Autor des Beitrags bleibt, wer verarbeitet).
- **Spiegel:** Jede Aktion (Anlegen, Ändern, Löschen, Wiederherstellen, Verarbeiten, jede Aktion und Rücknahme auf
  der Mindmap) ist ein Commit mit Herkunft: `works/<slug>/einfaelle.md` oder, ohne Werk, `<data>/einfaelle/`
  (`einfaelle.md`); der Lösch-Commit trägt den ganzen Eintrag.
- **Assistenten:** voller Zugriff auf Einfälle (Entscheidung IDEA_CRUD; MCP `idea_create`, `idea_update`,
  `idea_delete`, `idea_move`, `idea_link` …). `idea_create` ohne `parent` und `key` legt in die Wolke. Noch nicht
  umgesetzt: Der Lektor schlägt von sich aus passende Plätze vor.
- **Mindmap:** Die erste Hälfte der Hauptäste steht rechts, die zweite links. Gespeicherte Positionen der alten
  Ideenkarte bleiben in der Datenbank, wirken aber nicht. Ist ein Ende einer Beziehung eingeklappt, endet die
  Linie am sichtbaren Vorfahren; Beschriftungen suchen sich einen freien Platz auf deckendem Grund (in einer
  vollen Karte ein Stück neben der Linie). „widerspricht“ ohne Pfeil, Strich-Punkt. Nähe wird nie gespeichert
  (gespeicherte Embeddings oder gleiche Stichwörter, bestes Paar). „Gruppen benennen (Assistent)“: Vorschläge
  mit Annehmen/Verwerfen, nur bis zum Neustart. Eingeklappter Zustand in `localStorage` je Werk; der Ausschnitt bis
  der Tab geschlossen wird. Umschalt+Tab und Tab ohne gewählten Knoten bleiben beim Browser. Strg-/Umschalt-Klick auf den
  Titel öffnet den Einfall oder Abschnitt als Link. An Kapitel, Teil und Werk wirkt Enter wie Tab (am Werk: ein freies
  Hauptthema). Das Ziehen eines Einfalls auf den oberen/unteren Rand eines Einfalls ordnet ihn davor/danach ein. Dunkles Theme:
  Die Bildlaufleiste am Fensterrand in den Farben der Seite. Menüband der Seite: Gruppen als
  `details.band-group[data-band-group]` – „Knoten“, „Ansicht“, „Ausgabe“, „Einlesen“, „Gliederungsansicht“, „Zurück“.
- **Tasten am Mac (aus `mindmap.ts`):** Alt+E und Alt+L werten auch `event.code` (KeyE, KeyL) aus und funktionieren daher
  mit ⌥; Strg+Z und das Mausrad werten `ctrlKey || metaKey` aus.
- **Stand der Prüfung:** Die Tests prüfen die Aufrufe, die das Widget absetzt, und das Bild, das der Server dann
  liefert; in der Sichtprüfung lief es in Firefox mit synthetischen Ereignissen. Echte Maus und Tastatur, Zoom und
  Pfeiltasten sind eine manuelle Prüfung (`HANDOFF.md`).
- **Ausgabe:** `.xmind` = ZIP mit `content.json`, `metadata.json`, `manifest.json`; `.mm` FreeMind/Freeplane;
  `.svg`; `.pdf` (A4 quer, Titel und Filter); `.md`. In `.svg`/`.pdf` bleibt Eingeklapptes eingeklappt, in `.mm`
  und `.xmind` nie. Übertragen: Titel (Kapitel mit Nummer, „1 Helling“); Notiz (`notes.plain.content` oder
  `richcontent TYPE="NOTE"`, ein Absatz je Zeile); Stichwörter (`labels` oder das Attribut „Schlagwort“);
  Beziehungen (`relationships` mit `end1Id`, `end2Id`, `title` oder `arrowlink` mit `MIDDLE_LABEL`; Beschriftung =
  Art und Notiz; `.mm` Farben je Art); nicht: Status, Herkunft, Bezug (nur in `.md`), Zitat, Quelle, Bibelstelle,
  Positionen, Bilder, Stile, Marker.
- **Einlesen:** HTML-Notizen werden reiner Text; Beziehungen kommen als „führt weiter“ mit der Beschriftung als
  Notiz; das Hauptthema der Datei wird nicht selbst ein Einfall; der eigene Ast „Wolke“ wird an seiner Kennung erkannt
  (`baruch-wolke`), nicht an seinem Titel; „unter Knoten …“ kennt keine Wolke. Ein Commit, Herkunft Mensch.
  Grenzen: `.xmind`-Archiv höchstens 50 Einträge und 20 MiB entpackt; höchstens 20 Stichwörter je Knoten; `.mm`
  nur UTF-8 oder ASCII; XMind 8 (nur `content.xml`) abgelehnt. Nicht gelesen: weitere Blätter (Hinweis),
  Zusammenfassungen, Sprechblasen, Bilder, Marker, Stile, andere Attribute, `richcontent TYPE="DETAILS"`.
- **Annahmen über XMind und Freeplane (nicht geprüft, manuelle Prüfung durch den Nutzer):** Der Aufbau der `.xmind`-Datei
  folgt der offenen Beschreibung des Formats aus dem Gedächtnis; geprüft ist nur die Rundreise mit dem eigenen
  Leser.

  | Annahme | Risiko, wenn sie nicht gilt |
  |---|---|
  | XMind (2020 und neuer) öffnet ein ZIP mit `content.json`, `metadata.json`, `manifest.json` ohne `content.xml` und ohne Vorschaubild | Datei lässt sich nicht öffnen |
  | `content.json` ist ein Array von Blättern mit `id`, `class: "sheet"`, `title`, `rootTopic`, `relationships` | Blatt wird nicht erkannt |
  | Themen: `id`, `class: "topic"`, `title`, `children.attached`, `notes.plain.content`, `labels` | Notizen oder Stichwörter fehlen |
  | `structureClass: "org.xmind.ui.map.unbalanced"` verteilt die Äste rechts und links | eine andere Anordnung |
  | Themen-Ids dürfen beliebige Texte sein (`i:12`, `n:7`, `baruch-wolke`) | Beziehungen oder Themen fehlen |
  | `relationships`: `id`, `end1Id`, `end2Id`, `title` genügen | Beziehungslinien fehlen |
  | `manifest.json` mit `file-entries` für beide Dateien | Warnung beim Öffnen |
  | Beim Einlesen: freie Themen unter `children.detached`, HTML-Notizen unter `notes.realHTML.content` | Inhalte fehlen nach dem Einlesen |
  | Freeplane liest `MIDDLE_LABEL` und zeigt Attribute; FreeMind 1.0.1 überspringt `MIDDLE_LABEL` | die Beschriftung fehlt in der Anzeige |
  | Freeplane schreibt Beschriftungen als `MIDDLE_LABEL` (sonst `SOURCE_LABEL`/`TARGET_LABEL`), Notizen als `richcontent TYPE="NOTE"` | Beschriftung oder Notiz fehlt nach dem Einlesen |

### „Suchen, Verweise, Register und Glossar“ (`suchen.md`) {#suchen-verweise-register-und-glossar-suchenmd}

- **Suche:** Seite `/suche`
  (`/suche?work=W&q=…&range=…&key=…&sel=1,2,3&case=1&word=1&re=1&kinds=text,heading`); Auswahl aus dem Editor
  `range=selection&start=…&end=…` (UTF-16, Bereich „Markierung im Editor (Zeichen a–b)“); gezeigte Revision
  `shown_rev=n` oder `rev=shown` (nur für Abschnitt und Auswahl; Vorschau und Ersetzen gelten immer für die aktuelle
  Revision). Trefferlinks `/?work=W&key=K&find=…&at=…`. Ohne Teile ist der Bereich „Teil“ das ganze Werk.
- **Regeln:** Die Suche läuft im Typst-Quelltext. Im Wortlautmodus findet ein Typst-Sonderzeichen auch seine
  maskierte Form („C#“ findet „C\#“), die Ersetzung wird maskiert. Regulärer Ausdruck: Syntax der Rust-Crate `regex`,
  Gruppen `$1` oder `${name}`, Ersetzung wörtlich im Quelltext. Treffer: standardmäßig 500 gezeigt, höchstens 2000 (mehr
  werden gezählt); nach „abgebrochen“ funktioniert das Ersetzen nur mit einem kleineren Bereich. Die Vorschau zeigt nur ganze
  Abschnitte mit zusammen höchstens 2000 Stellen; über 8 MiB eingefügten Texts: „Zu viele Ersetzungen auf einmal“.
  Die Vorschau startet nicht auf Anfrage einer fremden Seite (403). Ein Abschnitt, dessen aktuelle Revision sich
  seit der Vorschau geändert hat, wird mit einem Grund übersprungen.
- **Commits:** `Ersetzt „x“ → „y“ in n Abschnitten` (ein Commit für alle), „Ersetzen rückgängig: …“. MCP sucht nur
  (`text_search`); Vorschläge dort über `propose_revision`.
- **Tasten (aus `search_keys.ts`):** Strg zählt auch als ⌘ (`ctrlKey || metaKey`); Buchstaben über `code`, daher
  funktionieren Alt+C/W/R/L am Mac mit ⌥. macOS fängt ⌘H meist selbst ab (Fenster ausblenden) – am
  Mac nicht geprüft; Strg+H erreicht den Browser. Nach „Alle“ schaltet „⌶“ wieder aus. Bei geöffnetem Satzbild wechselt der Editor
  nach dem Ersetzen in den Quelltext. Auf `/suche` schalten Alt+C/W/R die Kontrollkästchen um, wenn der Fokus im Such- oder
  Ersetzungsfeld liegt.
- **Sprungmarken:** Jeder Gliederungspunkt hat `<k{Schlüssel}>` (etwa `<k103>`), nur in Export, Spiegel,
  Satzseite und PDF: am Zeilenende der Überschrift (`= Einleitung <k103>`); Teile, Punkte ohne Nummer und
  Überschriften mit eigenem Label behalten ihr Label und bekommen darunter `#metadata("baruch-key") <k103>`. Beim
  ersten Start mit dieser Funktion ändert sich jeder Spiegel einmal (ein Commit je Werk). Eingefügt wird
  `#link(<k103>)[die Einleitung]` oder, mit Seitenzahl,
  `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]`. Ohne JavaScript schreibt man die
  Form selbst; `xref.targets` oder `/verweise` (`?key=K`) nennen die Labels. Strg+Klick funktioniert auch auf
  `<kN>`/`#link(<kN>)` im Quelltext. Sprungverlauf je Browser-Tab (`sessionStorage`); „Vorwärts“ nur als Knopf.
  Wird ein Export wieder eingelesen, verschwinden die Labels aus den Überschriften, die Verweise zeigen auf die
  neuen Schlüssel. Im EPUB interne Links auf die Überschrift in der Kapiteldatei. Gezählte Erwähnungen („das dritte
  Kapitel“) nur für Kapitel mit Nummer. Ist die Werkeinstellung „Verweis mit Seitenzahl“ nicht gesetzt,
  merkt sich der Browser die letzte Wahl; ist sie gesetzt, gilt eine Änderung im Dialog nur für diesen Verweis.
- **Glossar und Register:** Seiten `/werke/{id}/glossar`, `/werke/{id}/register` (`?key=K`: Abschnitt mit „Marke
  setzen“), `/werke/{id}/markierung?key=K&rev=R`; ohne Werk `/werke`. Registermarke im Text:
  `#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>`; nie in Überschriften, Code, Kommentaren, Roh-Text
  oder einer anderen Marke; Meldung „… trägt an der gewählten Fundstelle schon diese Registermarke; nichts
  geändert“; „alle“ setzt nur fehlende. Eigene Register: Kennung aus Kleinbuchstaben, Ziffern, Bindestrich
  (z. B. `orte`), Titel bis zu 80 Zeichen, höchstens 13. Registervorschläge werden „veraltet“, wenn das Wort fehlt
  oder schon die Marke trägt; sie bleiben auf ihrer Instanz. Verworfene Glossarvorschläge hinterlassen einen
  Grabstein (sie kommen beim Austausch nicht zurück). RPC `glossary.suggest`; MCP `glossary_list`,
  `glossary_propose`, `index_propose`. Spiegel: Datei `glossar.md`, jede Änderung ein Commit. Die Austauschdatei
  (Nebenversion 7) trägt das Glossar einschließlich Vorschlägen, eigene Register und die Satzeinstellungen dafür; höchstens
  5000 Glossarbegriffe, Registertitel höchstens 80 Zeichen (sonst wird die Datei ganz abgelehnt).
- **Im Satz:** Register- und Glossarseiten entstehen nur beim Ausschreiben (Typst-Export, Spiegel,
  Satzvorschau, PDF, EPUB) ab der Zeile `// Baruch: Register und Glossar`; der gespeicherte Text bleibt. Der
  Export lässt sich mit `typst compile` ohne Paket und ohne Netz setzen (mit dem eingebetteten Compiler geprüft).
  Im EPUB `Glossar` und je Register eine eigene Datei am Ende. Bibelstellenregister: nicht aus Kommentaren, Code
  oder Überschriften; Reihenfolge der biblischen Bücher. Beim erneuten Einlesen bleiben die Registermarken des
  Menschen; die Hervorhebung der ersten Erwähnung ohne Glossarseite bleibt als gewöhnliche Hervorhebung.
  Ende zu Ende geprüft in `tests/round31_abnahme.rs`.

### „Buchsatz: PDF, E-Book und Umschlag“ (`buchsatz.md`) {#buchsatz-pdf-e-book-und-umschlag-buchsatzmd}

- **Adressen:** Satz `/werke/<id>/satz?key=<schlüssel>` (Abschnitt `#druckpruefung`), PDF `/werke/<id>/pdf`
  (`<slug>.pdf`), Umschlag `/werke/<id>/umschlag`, Umschlag-PDF `/werke/<id>/umschlag.pdf`
  (`<slug>-umschlag.pdf`), EPUB `/werke/{id}/epub` (`?fussnoten=endnoten`), Vorschau `/werke/{id}/epub/vorschau`,
  Prüfung `/werke/<id>/epub/pruefung`, EPUBCheck `POST /werke/epub/pruefen`, Stichwortzettel
  `/werke/<id>/stichwortzettel` und `…/stichwortzettel.pdf?format=…&schrift=…` (`<slug>-stichwortzettel.pdf`),
  Word `/werke/<id>/word` (`<ordnername>.docx`). Entwurf: jede Ausgabe mit `?entwurf=1` oder `?entwurf=0`
  (`/pdf`, `/stichwortzettel.pdf`, `/epub`, `/umschlag.pdf`, `/satz`).
- **Satzseite:** lädt kein JavaScript (das Theme-Symbol führt zu „Einstellungen → Darstellung“, „Einstellungen“ öffnet
  die Seite, kein Alt+, und keine Pfeiltasten). Eine Seite, die sich zwei Kapitel teilen, steht in der Liste beider;
  ein lebender Kolumnentitel zählt nicht als Text des Kapitels. Meldungen mit Zeile und Spalte im Export.
  Fehlende Schrift: `Schrift „x“ fehlt; ersetzt durch eingebettete Schrift`. Pakete (`#import "@preview/…"`):
  „Typst-Pakete werden nicht geladen (offline)“. `image("/x")`, `image("./x")`, `image("x")` finden dasselbe Bild.
  Nach 60 s „Typst brauchte länger als 60 s“ (503); Cache höchstens 8 Stände und 192 MiB Seiten.
  Abbrechen: „Der Satz wurde abgebrochen. Es wurde nicht neu gesetzt.“ / „Es lief kein Satz, der abzubrechen
  war.“; wartende Anfragen 503. Typst lässt sich mitten im Rechnen nicht anhalten: Es hält nach dem Layout an;
  eine lange Schleife der Vorlage läuft bis zum Ende. Von einer fremden Website starten die Satzseite, Seitenbilder und
  PDF keinen Satz (403). Druckprüfung, Beispiel: `Bild „umschlag.png“: 142 dpi bei 14,8 × 21,6 cm – für 300 dpi
  braucht es 1748 × 2552 Pixel`; SVG erscheint nie; es zählt nicht zu den Fehlern von Typst.
- **Satzeinstellungen:** Tabelle `work_typeset`, jede Änderung ein Commit; Regelblock `// Baruch: Satz` hinter
  der Werkvorlage in `main.typ`, Export, Satzvorschau, Buch-PDF und Umschlag – nur wenn mindestens eine
  Einstellung gesetzt ist. Nur der Mensch ändert sie (MCP `typeset_settings` liest). Anker `#satz`. Grenzen und
  Einzelheiten: Bundzugabe leer = 0; Absatzabstand leer = 1 oder 0,5 em; Sprache z. B. `de`, `en`, `de-CH`; Zählung „ab
  dem ersten Kapitel“: Die erste Kapitelseite trägt die 1, ein Querverweis „S. n“ nennt die gedruckte Nummer; der
  Werktitel für den Kolumnentitel stammt aus `#set document(title: …)`; der Umbruch am Kapitelanfang gilt im
  Export auch vor einem Teil; bei eingerückten Absätzen bleibt der erste Absatz nach einer Überschrift ohne Einzug.
  Übernehmbare Regel: `#show heading.where(level: 1): it => { pagebreak(weak: true, to: "odd"); it }` (auch
  variiert, mehrzeilig oder `it => [#pagebreak() #it]`); Übernehmen = eine neue Revision der Werkvorlage ohne die
  Zeile, ein Commit. Vorschaubilder liegen nur im Cache. „Erzeugte Typst-Regeln“ zeigt den Block.
  Die Seitengröße des Umschlags folgt der ersten Buchseite; der Rücken rechnet mit der Seitenzahl einschließlich Leerseiten.
- **Satzbild-Einstellungen** (Anker `#satzbild`, nur SQLite, nicht im Spiegel und nicht im Austauschformat):
  Schrift als Liste wie in CSS (höchstens 200 Zeichen, ohne `; < > { }`), Größe 6–48 pt, Textbreite 6–60 cm mit
  `pt`, `mm`, `cm`, `em`, Farben `#rrggbb`; „Alles aus der Vorlage“ setzt alle zurück.
- **Entwurf:** Text höchstens 40, Zusatz 120 Zeichen; Datum = Tag der Ausgabe in Ortszeit; Deckkraft 5–50 %
  (Standard 12 %); im Umschlag-PDF nur mit dem Kontrollkästchen (dann eine Warnung der Druckprüfung); EPUB: ein Vermerk auf der
  Titelseite und „(Entwurf)“ im Titel; Export, Spiegel und erzeugte Regeln bleiben ohne Wasserzeichen.
- **PDF:** Fehler 422 (Seite mit Meldungen), 409 („Für dieses Werk entsteht gerade ein PDF …“), 503 nach 120 s.
  Kommandozeile `baruch-server export <id|slug> --pdf DATEI [--data ORDNER]`: eine vorhandene Datei Exit 1 („… gibt es
  schon; nichts überschrieben“), Satzfehler Exit 1, `--pdf` mit `--out` Exit 2; Warnungen der Druckprüfung auf
  stderr. `pdfinfo` (poppler 25.03) meldet für alle PDFs „Syntax Error: Suspects object is wrong type
  (boolean)“ – harmlos: Typst schreibt `/MarkInfo << /Marked true /Suspects false >>` (PDF/UA); gemessen im Test
  `round21_bilder`.
- **Umschlag:** Blöcke je Seite in mm ab der Beschnittkante links oben (negativ = im Beschnitt; y zwischen −50 und
  1000); Farbe `light`, `dark` oder `#rrggbb`; freier Text/Motto höchstens 1000 Zeichen; Einrasten bis 1,5 mm. Ohne
  JavaScript trägt die Adresse die Werte als `b.<Art>.<Feld>=…` (z. B. `?b.front_title.y_mm=40`). Schnellwahl und
  Zurücksetzen sind je ein Commit. Sobald ein Block gespeichert ist, setzt Baruch den Umschlag aus den Blöcken
  (Vorschau, Umschlag-PDF, Vorder-/Rückseite im Buch-PDF). Schrift ohne Wahl: die erste aus dem letzten
  `#set text(font: …)` der obersten Ebene. Typst misst die Höhe der Blöcke mit den Text- und Absatzregeln der
  Werkvorlage (`#set text(…)`/`#set par(…)` am Zeilenanfang; nicht Schrift, Größe, Farbe). Das Hochladen eigener Schriftdateien
  funktioniert noch nicht. Namensschilder der Rückenblöcke nur am gewählten Block. Kontrast: WCAG unter 3 : 1,
  im Rasterbild einschließlich Schatten/Verlauf gemessen; SVG nicht gemessen; Bilder über 20 000 Pixel Kantenlänge „nicht
  gemessen“. Rückenberechnung: „Rücken 1,5 mm = 30 Buchseiten (15 Blatt) × 0,1 mm je Blatt (die 3 Umschlagseiten
  im Satz zählen nicht)“. Das Löschen des Umschlagbilds entfernt zuerst den Umschlag (Commit „Umschlag entfernt“); ein
  bestätigter Umschlagantrag ist ein Commit „Bildantrag n bestätigt: …“. RPC `cover.layout`; MCP `cover_layout`
  liest, `cover_layout_set` stellt nur einen Antrag.
- **Buchangaben:** Jahr vierstellig; ISBN-13 mit oder ohne Bindestriche, Präfix 978/979, gespeichert wie
  eingegeben; einzeilige Felder 200 Zeichen, Klappentext 4000, Kurzbiografie 2000, Widmung, Rechtevermerk,
  Bildnachweis 1000, Adressen 500; ungültig 422; Commit „Buchangaben geändert: …“.
- **Vorspann aus der Werkvorlage:** Funktionen `#let schmutztitel(buch) = …`, `haupttitel`, `impressum`, `widmung`;
  `buch` ist ein Wörterbuch aus Zeichenketten: `titel`, `untertitel`, `autor`, `verlag`, `verlag_anschrift`,
  `ort`, `jahr`, `auflage`, `isbn`, `preis`, `reihe`, `druck`, `druck_anschrift`, `bibel`, `bibel_hinweis`,
  `bildnachweis`, `widmung`, `klappentext`, `vita`, `copyright`, `dnb_titel`, `dnb_text`, `gesetzt`,
  `impressum_zeilen`. Reihenfolge: Schmutztitel, Seite 2 (Reihe oder leer), Haupttitel, Impressum (Hinweis der
  Deutschen Nationalbibliothek, ©-Zeile, Auflage, ISBN, Bibelübersetzung, Bildnachweis, „Gesetzt mit Baruch“,
  Druckerei), Widmung einschließlich einer Leerseite. Neue Bücher bekommen die Impressumsvorlage angehängt. Adressen: Verlag nach
  der ©-Zeile, Druckerei hinter „Druck:“, Zeilen mit einem Komma verbunden. Rückseite: Strichcode EAN-13 oder
  Platzhalter.
- **EPUB:** EPUB 3 mit `toc.ncx` für EPUB-2-Reader; ohne Typst-Satz, kein Commit; `break-before` im Stil.
  Umschlagbild: die gesetzte Vorderseite als PNG bis 1600 × 2560 Pixel; scheitert das, die Bilddatei mit der Warnung
  `COVER_RASTER_FAILED`. Sprache: Satzeinstellung, sonst `#set text(lang: …)`, sonst Deutsch. Hinweiscodes:
  `ISLAND_OMITTED`, `IMAGE_NO_ALT`, `IMAGE_MISSING`, `ARGS_OMITTED`, `TEMPLATE_PAGEBREAK_MAPPED`,
  `TEMPLATE_PAGEBREAK_UNMAPPED` (RPC `work.epub`, MCP `epub_export`, CLI stderr). Gleicher Stand, gleiche Bytes;
  ohne Abschnitt 422; fremde Seite 403. Ein Verweis `@name` auf eine Abbildung zeigt „Abbildung n“; eine Fußnote
  in einer Fußnote steht hinter ihrer äußeren. Profile der Vorschau: Apple Books (iPhone, iPad), Google Play
  Books, „Kindle (über EPUB)“, „Kindle-App (über EPUB)“, wählbar tolino, Kobo, Thorium Reader, calibre – Annahmen
  ohne Gerät; Popup-Fußnoten erscheinen als Kasten; unbekannte Reader/Dateien 404 mit einem Hinweis; Pfeiltasten in
  einer Auswahlliste laden nicht neu (Enter bestätigt). Prüfung: `mimetype` als erster unkomprimierter Eintrag,
  `META-INF/container.xml`, Paketdokument, Manifest, Spine, nav, NCX; Wohlgeformtheit mit `xmllint --noout
  --nonet`, sonst quick-xml. EPUBCheck: `epubcheck` im PATH, sonst die konfigurierte `epubcheck.jar` mit
  `java -jar`; höchstens 3 Minuten, eines zur Zeit; MCP `epub_check` bekommt nur die eigene Prüfung.
- **Stichwortzettel:** Marke `// Stichwort: …` (Groß-/Kleinschreibung egal), sonst `*…*` und `#strong[…]`, sonst der erste
  Satz je Absatz. Kein Satzende nach einer Ziffer („1. Kor“) und nach Abkürzungen („z. B.“, „d. h.“, „u. a.“, „bzw.“,
  „vgl.“, „usw.“, „ca.“, „Nr.“, „S.“, „Kap.“, „V.“, „etc.“, „ggf.“, „evtl.“ …). Nicht gezählt: Überschriften,
  Kommentare, `#set`/`#show`/`#let`/`#import`/`#include`, Roh-Text, Mathe, `#footnote[…]`, `#arbeit[…]`; tiefer als
  32 Ebenen wird verworfen; `@name` entfällt (ein `@` in der Wortmitte bleibt). Bibelstellen einheitlich, jede einmal.
  A5 hoch Standard 16 pt, Karten 14 pt. Fehler 409/422/503 wie beim PDF. Kommandozeile
  `baruch-server export <id|slug> --stichwort DATEI [--format a5|karten] [--schrift PT] [--data ORDNER]`
  („Stichwortzettel geschrieben: DATEI (n Seiten, m Bytes)“; falsche Kombinationen Exit 2).
- **Word-Export:** Formatvorlagen „Überschrift 1–3“ (tiefer „Überschrift 4–6“), „Baruch Teil“, „Baruch ohne
  Nummer 1–3“, „Zitat“, „Baruch Zitatquelle“, Zeichenformat „Baruch Zitat“; Tabellen aus
  `#table(columns: …, […], …)` (auch `table.header`); Bilder PNG, JPEG, GIF eingebettet; Typst-Inseln mit
  verborgenem Quelltext. Warnungen (RPC, CLI): `docx_typst_insel`, `docx_bild_fehlt`,
  `docx_tabelle_vereinfacht`, `docx_weitere_vorschlaege`; die Seite im Browser liefert ohne Bericht.
  Änderungsnachverfolgung: Wort für Wort; eine neue/gestrichene Abbildung als Absatz; bei Tabellen bleibt das leere Raster;
  Seitenumbrüche nicht nachverfolgt; eine Ebenenänderung einer Überschrift als gestrichen und neu. Ausgeblendete Revisionen
  zählen nicht. Kommandozeile
  `baruch-server export ID|SLUG --docx DATEI [--vorschlaege] [--data ORDNER]`. Über MCP gibt es das nicht.
- **Typst-Kommandozeile:** `baruch-typst compile buch.typ buch.pdf` oder `baruch-server typst compile …`
  (`docs/Verteilung.md`, „Typst command line“).

### „Austausch, Sicherung und Sicherheit“ (`austausch-sicherung.md`) {#austausch-sicherung-und-sicherheit-austausch-sicherungmd}

- **Austausch:** Format in `docs/Austauschformat.md`. Ausgabe `/werke/{id}/austausch` (ohne Werk zur Werkliste):
  `.baruch` = ZIP-Container mit dem Werk als XML, den Bildern als eigenen Dateien und einem Manifest mit Prüfsummen;
  `.baruch.xml` = eine XML-Datei mit den Bildern als Base64. Umfang auch: Grabsteine gelöschter Teile, die Mindmap mit
  Verbindungen (auch gelöschten) und gemerkten Positionen (Nebenversion 2), Befunde einschließlich ihres Verlaufs (Nebenversion 4),
  Glossar und Register (Nebenversion 7), Verlauf der Ergänzungen. Nicht enthalten: Schlüssel und Token,
  Verbindungen (Modelle, MultiGPT, pgvector), Personas einschließlich Modellen, offene Anträge, die berechnete Nähe, Bibel-
  und Literaturtexte, Konten, Sitzungen, Freigaben, die Einstellungen „Fassungen“ und „Satzbild“, Marken zum
  Ausblenden. Kommandozeile `baruch-server export ID|SLUG --baruch DATEI [--data ORDNER]` (endet `DATEI` auf
  `.xml`, nur XML; eine vorhandene Datei Exit 1; mit `--out`, `--pdf`, `--stichwort` Exit 2; die Ausgabe nennt Werk,
  Kennung, Größe, SHA-256). RPC `work.exchange_export`, MCP `exchange_export` (nur lesend, höchstens 4 MiB).
- **Einlesen:** `/austausch/einlesen`; Abweichungslinks `/?work=W&key=K&rev=R`; Commit „Austausch aus
  <Instanz>“. Eine Verbindung der Mindmap, die es schon gibt (gleiche Einfälle, gleiche Art), kommt nicht doppelt;
  eine Position nur für Einfälle ohne eigene. Wurde ein Punkt dort über die Gliederung umbenannt, reist die Revision
  mit der neuen Überschrift mit: Hinweis „Umschalten genügt“ statt eines Antrags. Unbekanntes aus einer neueren
  Datei wird nicht übernommen. Vorspulen nur für die eigene aktuelle Revision und nur wenn die Schritte der Datei
  ohne Lücke den neuen Text ergeben; Revisionen von Personas oder MCP-Clients, Vorschläge und abgelöste Revisionen
  werden nie überschrieben („abweichende Fassung“). Ein neuer Teil aus der Datei entsteht nach Zustimmung „mit
  Kennung und n Fassungen“ genau wie dort (Text, Herkunft, Status, Verlauf). Befunde, die es hier schon gibt,
  behalten Status, Grund und Verlauf („… 2 Einträge im Verlauf der Datei nicht übernommen“). Umschlagblöcke
  kommen in Normalform an (Schriftname ohne Leerraum an den Rändern, Farbe in Kleinbuchstaben, Zeilenenden `\n`); ein Block, den die
  Prüfung des Editors ablehnt, wird übernommen und steht unter den Hinweisen. Passt die Überschrift einer Revision nicht zur
  Art ihres Punkts (`===` an einem Kapitel), steht dort ein Hinweis. Kommandozeile
  `baruch-server import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title TITEL] [--data ORDNER]`
  (Werk schon vorhanden ohne `--abgleichen`: Exit 1; `--abgleichen` bei einem unbekannten Werk: Exit 1; mit `.typ`: Exit 2).
  Das Einlesen gibt es über MCP nicht.
- **Instanz:** Kennung = UUID, beim ersten Start vergeben; Name ohne Zeilenumbrüche oder Tabulatoren, Leerraum an den Rändern
  entfällt; ohne Namen „ohne Namen (3f2b8c1e)“ (erste 8 Zeichen der Kennung). Kennung des Werks (UUID) unter
  „Werk bearbeiten“.
- **Sicherung:** Ordner `<Name>.sicherung-<JJJJ-MM-TT-HHMMSS>` (Ortszeit), Datenbank über `VACUUM INTO`, dazu
  Spiegel, Einfälle und alles Übrige; symbolische Links werden übersprungen; eine zweite Sicherung in
  derselben Sekunde scheitert; Grenze 10 Sicherungen; nach der Sicherung müssen 512 MiB frei bleiben. Adresse
  nach dem Anlegen `?ok=sicherung`; „Sicherung angelegt“ steht dort nur einmal. Nicht über MCP. Wiederherstellen:
  `docs/Betrieb.md`.
- **Sicherheit:** Schreibende Aufrufe (`/rpc`, `/act`, `/mcp`) nur von der eigenen Seite; ein anderer `Origin` oder
  `Sec-Fetch-Site` → 403 (Skripte ohne Browser senden diese Header nicht). Ebenso 403 für Satz, Satzseiten
  als Bilder, Umschlag (Seite, Bild, PDF), Buch-PDF, Stichwortzettel-PDF und Mindmap als PDF, wenn eine fremde Seite
  sie auslöst. Alle Seiten antworten nur unter einer IP-Adresse oder `localhost` (`http://127.0.0.1:8765/`,
  `http://[::1]:8765/`, `http://localhost:8765/`); ein fremder Name (DNS-Rebinding) → 421. Unter einem Rechnernamen
  nur mit `--allow-host NAME` / `BARUCH_ALLOWED_HOSTS` (genau der Name, beliebiger Port in `Host`; kein
  Platzhalter, kein Port, keine IP – ungültig: Exit 2). Der Proxy muss `Host` unverändert durchreichen (sonst 403
  „Origin passt nicht zu Host“); Baruch wertet `X-Forwarded-*` nicht aus. Unter einem erlaubten Namen zählt eine
  Anfrage nie als „vom eigenen Rechner“ (`/mcp` dort immer mit Token, 401 ohne). Ohne Konten muss der Proxy den
  Zugriff schützen. Beispiele (nginx, Caddy), Probe und TLS zu pgvector (`PGSSLROOTCERT`): `docs/Betrieb.md`.
  `/mcp` ohne Token nur von 127.0.0.1 oder ::1 (sonst 401); mit Token auch von anderen Rechnern, alles andere dort
  403. Über MCP nur Vorschläge, Beiträge und Einfälle; Abschnitte nur über `key` (ein Aufruf mit `node` wird
  abgelehnt, `docs/MCP.md`). Der Import liest nur gewöhnliche Dateien (FIFO oder Gerät werden vor dem Öffnen abgelehnt).
- **Datenordner:** Meldung über Rechte mit einer Zahl (z. B. 0755); `chmod 700 <Ordner>` lässt nur den Besitzer hinein.

### „Einstellungen“ (`einstellungen.md`) {#einstellungen-einstellungenmd}

- **Adressen:** `/einstellungen/<seite>` (`darstellung`, `werk`, `assistenten`, `verbindungen`, `quellen`, `mcp`,
  `daten`, mit Konten `konten`); die alte Adresse `/einstellungen/personas` leitet auf `assistenten` um. Das Popup ist
  bis zu 1480 px breit; ein abgelehntes Formular zählt nicht als gespeichert (Theme, Satzbild und Marken der
  Werkseite werden dann nicht neu geholt). Marken und Popup brauchen JavaScript.
- **Cookies (`baruch_*`):** `baruch_work`, `baruch_editor`, `baruch_pv` (Vorschläge anfangs offen/zu),
  `baruch_folded` (höchstens 200 Einträge `<key>-<rev>`), `baruch_band`, `baruch_laptop`, `baruch_lrail`,
  `baruch_rrail`, `baruch_zoom`, dazu Spaltenbreiten, der offene Bereich rechts, eingeklappte Kapitel und das Theme.
- **Sprache der Oberfläche (Runde 39a):** Englisch oder Deutsch. Ohne Wahl folgt Baruch dem Browser
  (`Accept-Language`, sonst Englisch). Die Wahl erfolgt im Menü „Baruch ▾“ (zwei Knöpfe „English“ und
  „Deutsch“, der aktuelle markiert) oder unter „Einstellungen → Darstellung“ (Block „Sprache der Oberfläche“ mit
  „Wie der Browser“); beides sind einfache Formulare an `POST /einstellungen/sprache` und funktionieren ohne JavaScript. Die Wahl
  wird im Cookie `baruch_lang` und als Einstellung gehalten: ohne Konten für die Instanz, mit Konten je Konto
  (auch Lektoren); auf der Anmeldeseite ist es nur das Cookie. Reihenfolge: die eigene Einstellung, das Cookie, der Browser,
  Englisch. MCP antwortet immer auf Englisch; die Werksprache (Buchausgaben) wird je Werk gesetzt. Noch deutsch auf englischen
  Seiten nach Runde 39a: Bezeichnungen aus anderen Teilen von Baruch (Gliederungsschemata und Satzfelder,
  Umschlagseiten und -blöcke, Befundstatus, EPUB-Reader-Profile), die Meldungen des semantischen Index und mancher
  Importwege und natürlich die eigenen Daten des Nutzers (Beispielwerk, Personas, Bibeln).
- **Werksprache (Runde 38):** Block „Werksprache“ auf der Seite
  `/einstellungen/werk` (`id="sprache"`, unmittelbar vor dem Zeitwächter `id="zeit"`): ein `<select name="language">`
  mit „Deutsch“ (`de`) und „Englisch“ (`en`) und dem Knopf „Speichern“; ein einfaches Formular, ohne JavaScript benutzbar.
  Route `POST /einstellungen/werk/sprache` (Felder `work`, `language`), Antwort 303 zurück auf die Seite mit
  `ok=sprache#sprache`, bei einem ungültigen Wert `sprachfehler=1#sprache`; bei einem unbekannten oder fehlenden Werk `sprachfehler=werk#sprache`
  (303 auf `/einstellungen/werk?sprachfehler=werk#sprache`) mit der Meldung „Die Werksprache ließ sich nicht speichern: dieses
  Werk gibt es nicht; nichts geändert.“ (nichts wird geändert); für Lektoren gesperrt (nicht in `LEKTOR_ROUTES`).
  RPC `work.language` und `work.language_set` (nur der Mensch, nicht über `/act`, nicht über MCP); `work.get`,
  `work.meta`, `work.list` und MCP `list_works`/`outline` tragen `language`. Nur in der Datenbank gespeichert
  (`work.language`, Migration 0055), kein Commit im Spiegel. Die Assistenten antworten in der Werksprache; die
  Sprache der Benutzeroberfläche ändert sich nicht, ebenso wenig die Silbentrennungssprache der Satz-
  einstellungen. Hilfeseite: `site/content/de/hilfe/einstellungen.md`.
- **Werk:** gespeichert in SQLite und im Git-Spiegel. Schalter für den Abgleich: Kontrollkästchen „Ja, die aktuellen Fassungen
  dieses Werks dürfen nach MultiGPT“; das Umschalten selbst sendet nichts.
- **Assistenten** (SQLite, alle Werke): Reiter ohne JavaScript als `/einstellungen/assistenten?persona=<handle ohne
  @>` (Anker `#persona-<handle>`; ein unbekanntes `?persona=` zeigt den ersten Reiter mit einem Hinweis). Grenzen: Name 1–80,
  Rolle und Modell 1–200 Zeichen, Kontext 1–10 000 000 Tokens, Prompt 1–8000 Zeichen, Farbe `#rrggbb` oder `#rgb`
  (ein ausgefülltes Textfeld hat Vorrang); ungültig 422. Palette: acht Farben in der Reihenfolge der Personas;
  Hintergründe werden auf mindestens 4,5 : 1 Kontrast gebracht (WCAG AA); Farben stehen nicht im Spiegel und
  lassen sich über MCP nicht ändern. Bei Gleichstand des Kontexts gilt die frühere Persona. Warnungen „Modell …
  ist unbekannt bei Eliezer“, „Alter Name: Alias → …“, ein Hinweis zum Grafikspeicher bei verschiedenen Modellen,
  „Modellliste nicht verfügbar: …“. Die Kontextlänge dient nur zum Kürzen; Baruch fordert nie eine an. Formulare:
  `action=rag` (bis zu fünf Treffer `[R1]`…`[R5]`, der geprüfte Abschnitt ist nie darunter), `POST /websuche/act`,
  `action=backend`, `action=bible` (Standard: @belege an, Lektor aus). Wirksamer Kontext: Eliezer und Ollama
  melden die Länge, OpenAI-kompatible Dienste nicht; bei Ollama zählt `num_ctx` aus dem Modelfile, sonst die Länge
  des geladenen Modells, sonst 4096 Tokens. Beim Öffnen fragt Baruch keine Gegenstelle (nur der gezeigte Reiter auf
  Anfrage, höchstens 10 s, 60 s gemerkt); Beispiel „wirksam 4096 Tokens (Einstellung 32768), Gegenstelle meldet
  4096“.
- **Verbindungen** (SQLite; Geheimnisse im Schlüsselbund, Dienst `baruch`):

  | Verbindung | Felder | Geheimnis | Standard |
  |---|---|---|---|
  | Eliezer (Modell-Backend) | Adresse | Schlüssel `baruch/eliezer` | `http://127.0.0.1:1234` |
  | MultiGPT (MCP) | Adresse | Token `baruch/multigpt` | leer (= nicht eingerichtet); Platzhalter `https://multigpt.example/mcp/` |
  | pgvector (PostgreSQL) | Rechner, Port, Datenbank, Benutzer, TLS (`disable`/`prefer`/`require`/`verify-full`); Tabelle (`tabelle` oder `schema.tabelle`), Spalten Text, Vektor, Sammlung, Dokument (Pflicht), Position, Titel, Adresse, Modell; Embedding-Modell; Anfragepräfix | Passwort `baruch/pgvector` | Rechner leer, Port 5432, TLS `prefer`; `chunks`, `content`, `embedding`, `collection`, `document`, `nomic-embed-text` |
  | LanceDB (lokal) | Ordner (relativ zum Datenordner oder absolut), Embedding-Modell, Präfix der Anfrage und der Dokumente | – | `lancedb`, `nomic-embed-text` |

  MultiGPT hatte früher eine Intranet-Adresse als Standard; seitdem ist sie leer. Der Platzhalter wird nie
  gespeichert und nie angefragt; eine gespeicherte Adresse bleibt (keine Migration); wer nur den Standard
  benutzte, trägt die Adresse einmal ein. Ohne Adresse: Weg „aus“, Volltext „Keine Adresse für MultiGPT eingetragen
  (Einstellungen → Verbindungen)“. Aufgabenpräfixe (für nomic-embed-text `search_query: ` und
  `search_document: `, das Leerzeichen gehört dazu; höchstens 64 Zeichen); ein geändertes Dokumentpräfix verlangt
  „Index neu aufbauen“. TLS-Hinweis bei einem Passwort mit `prefer`/`disable`, milder bei `require`; nur `verify-full`
  ohne Hinweis. Spaltennamen: Buchstaben, Ziffern, `_`, höchstens 63 Zeichen, gegen
  `information_schema` bestätigt. Geheimnis: nicht leer, eine Zeile, höchstens 4096 Zeichen; gebunden an Schema, Rechner
  und Port (pgvector: Rechner, Port, TLS) – sonst „Nicht geprüft: …“ oder „Schlüssel gilt für ein anderes Ziel;
  nicht gesendet“. Adressen nur `http://`/`https://`, ohne Benutzerinformation und ohne Token in der Abfrage.
  **„Verbindung prüfen“** (zusammen 10 s, Antwort höchstens 1 MiB, keine Weiterleitung – eine Weiterleitung ist
  ein Fehler, Geheimnisse gehen nie an ihr Ziel): Eliezer `GET <Adresse>/v1/models` mit `Authorization: Bearer`,
  danach ein Vergleich der Modelle aller Personas (Liste 60 s gültig); MultiGPT-MCP `initialize` (Protokoll
  `2025-06-18`, Client `baruch`), `notifications/initialized`, `tools/list` mit `Mcp-Session-Id` über höchstens
  10 Seiten (`nextCursor`), `list_collections`, JSON oder `text/event-stream`; pgvector-Anmeldung (nie aus
  `PGPASSWORD`/`~/.pgpass`), Version der Erweiterung `vector`, Tabelle, Sammlungen, Dimension, Modelle (Werte
  gekürzt: Sammlungsnamen 200 Zeichen, höchstens 1000 Sammlungen); LanceDB-Ordner, Tabellen, Einträge, Modell,
  Dimension und Probe-Embedding „Baruch prüft die Einbettung.“. HTTP 401: „Zugang verweigert … Schlüssel
  baruch/… prüfen“. Kurztest: höchstens 16 Tokens, vorgewählt `google/gemma-4-26b-a4b`. Modellverbindungen
  (Abschnitt `#modelle`): Arten Eliezer, OpenAI-kompatibel (`/v1/models`, `/v1/chat/completions`), Ollama;
  Schlüssel `baruch/model-<id>`; lokal „nach der Adresse“ sind Loopback (`127.…`, `localhost`, `::1`), private
  Adressen (`10.…`, `172.16–31.…`, `192.168.…`, `fd…`), Namen ohne Punkt und unter `.local`, `.lan`, `.internal`,
  `.home.arpa`; eine Anfrage zur Zeit je Verbindung; ist Eliezer selbst extern, bettet Baruch nichts ein.
  Formulare an `POST /einstellungen/verbindungen/act` oder `POST /einstellungen/modelle/act`, Antwort 303 mit
  `?ok=<code>` / `?fehler=<code>[&kind=…]` / `?modellfehler=…`; Eingaben stehen nie in der Adresse.
- **Semantische Suche (RAG):** Abschnitt `#rag`; Standard ohne Wahl: LanceDB; eine Anfrage (RPC, MCP, Persona) kann
  nur Wege weglassen. Sammlungen höchstens 100, je 1–200 Zeichen. Der lokale Index (Standard
  `<Datenordner>/lancedb`) bettet nur geänderte Absätze neu ein (über Eliezer, eigene Warteschlange, höchstens 32
  Texte je Anfrage), holt beim Start alle Werke nach, merkt sich Modell und Dimension („Der Index ist mit dem
  Einbettungsmodell … aufgebaut, eingestellt ist …“). Scheitert das Einbetten dreimal, bleiben Aufträge liegen
  (`n Aufträge liegengeblieben – erneuter Versuch nach einer Änderung oder „Index neu aufbauen“`); eine Suche
  versucht es nicht erneut. Mit einem eigenen `--data` ohne gespeichertes Eliezer gibt es beim Start keine
  Embeddings. „Index neu aufbauen“ antwortet sofort und bricht einen hängenden Auftrag ab; nur der Mensch. Je
  Suche alle Wege zugleich (je 20 s). pgvector: Kosinusabstand, nur lesend. MultiGPT: `search_documents`,
  `read_document`.
- **Quellen:** Titel 1–200 Zeichen, Adresse http/https; Links mit `rel="noopener noreferrer"`; MCP
  `sources_list`; Formulare an `POST /einstellungen/quellen/act`. Eine Lesezeichenliste lässt sich nicht einlesen.
- **MCP-Zugang:** nur der SHA-256-Hash wird gespeichert; Adresse `http://<Rechner>:<Port>/mcp`; Token `baruch_…`, Name
  1–64 Zeichen; Autor `@mcp:<tokenname>` (aus `clientInfo` zählt nur die Version; die Tabelle nennt „früher
  `@mcp:…`“, wenn die Namensregel früher einen anderen Autor ergab; abgelehnt werden Namen, die denselben Autor
  wie ein vorhandenes oder widerrufenes Token ergäben oder mit „lokal-“ beginnen). Befehl:
  `claude mcp add -s local --transport http baruch http://<IP>:8765/mcp --header "Authorization: Bearer <Token>"`.
  Voraussetzung: `--listen 192.168.…:8765`; Ansprache über die IP-Adresse (sonst 421). Von anderen Rechnern
  ist nur `/mcp` erreichbar (sonst 403 „Von anderen Rechnern nur /mcp mit Token.“); ein falsches Token wird
  auch lokal abgelehnt (401). Formulare an `POST /einstellungen/mcp/act` (Anlegen 200, Widerrufen 303 `?ok=widerrufen`).
  Hinter einem Reverse-Proxy zeigt die Seite `https://<Name>/mcp` (`https` ist eine Annahme); Basic-Auth des Proxys
  muss `/mcp` ausnehmen (`docs/Betrieb.md`).
- **Daten:** EPUBCheck-Pfad absolut, Endung `.jar`, eine vorhandene Datei; `epubcheck` im PATH hat Vorrang.

### „Konten und Lektoren“ (`konten.md`) {#konten-und-lektoren-kontenmd}

- **Adressen:** `/einrichten` (nur am eigenen Rechner, sonst 403), `/anmelden`, `/konto`,
  `/werke/<id>/mitwirkende`, „Einstellungen → Konten“. Mit Konten verlangen Seiten, Formulare und `/rpc` eine
  Anmeldung; `/mcp` bleibt bei Token.
- **Rechte:** Die einzige Stelle der Prüfung ist `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`); alles nicht Erlaubte
  antwortet 403 „Abgelehnt: …“. Herkunft `@lektorat:<name>`; jeder Vorschlag, Beitrag und Befund ist ein Commit.
- **Gespeichert:** Passwort als Argon2id-Prüfwert, Sitzung als Hash; nicht im Austauschformat und nicht im
  Spiegel. Der Browser sendet das Sitzungs-Cookie bei einem Sprung von außen nicht („Schon angemeldet?
  Weiter“). Anmeldung und Einrichtung folgen dem Theme-Cookie ohne JavaScript.
- **Kommandozeile:** `baruch-server passwort NAME [--data ORDNER]` liest das Passwort als eine Zeile von der
  Eingabe, beendet alle Sitzungen und hebt die Sperre nach Fehlversuchen auf.
- **„Am eigenen Rechner ohne Anmeldung“:** Schreibt ein Reverse-Proxy auf demselben Rechner `Host` um,
  sähe jeder Besucher wie der eigene Rechner aus (`docs/Betrieb.md`).
- Für Lektoren ist das Umschlagbild in der Werkliste kein Link; sie erreichen „Einstellungen → Darstellung“ auch
  ohne freigegebenes Werk. Offen: ob Lektoren das Buch als PDF lesen dürfen (derzeit nicht).

### „Tasten und Maus“ (`tasten.md`) {#tasten-und-maus-tastenmd}

- Die Mac-Spalte ist aus `crates/baruch-server/web/src` abgeleitet, nicht an einem Mac geprüft: `ctrlKey || metaKey` in
  `panes.ts` (Strg+S), `wysiwyg.ts` (Strg+I, Strg+B, Strg+Enter), `search_keys.ts`, `threads.ts`, `xref.ts`,
  `cover_editor.ts`, `mindmap.ts`, `zoom.ts` (nur Strg+0). Nur `ctrlKey`: das Mausrad in `zoom.ts`. Alt-Kürzel
  mit `event.code`: Alt+E (`ideas.ts`, `mindmap.ts`), Alt+L in der Mindmap, Alt+C/W/R/L der Suche. Nur über
  `event.key`: Alt+L für die Literatur (`literature.ts`) und Alt+, für die Einstellungen (`settings.ts`,
  `aria-keyshortcuts="Alt+Comma"` in `base.html`) – am Mac erzeugt ⌥ dort ein anderes Zeichen, die Kürzel
  wirken daher vermutlich nicht (ein Kandidat für eine Korrektur).
