# Backlog

Wünsche und Vorhaben, die noch keiner Stufe zugeordnet sind. Entscheidungen stehen in `docs/Plan.md`,
der Arbeitsstand in `HANDOFF.md`.

## Nächstes Vorhaben: Lektorat (Entscheidung des Nutzers 2026-11-24, nach dem Release 0.9.0)

Der Nutzer will Baruch zum Werkzeug für das Lektorat weiterentwickeln. Anlass war eine Einschätzung aus der Sicht
eines Lektors: Die Haltung „Mensch entscheidet, Assistenten schlagen nur vor“ trägt, aber für die Arbeit mit
Verlagen und mit menschlichen Lektoren fehlen Word, ein zweiter Mensch und ein einfacher Einstieg.
Entscheidungen des Nutzers:

- **Word (.docx), Import und Export.**
  - *Import:* Ein Manuskript wird zu einem Werk mit Kapiteln (aus den Überschriften-Formatvorlagen),
    Hervorhebungen, Fußnoten, Zitaten und Bildern. Die Änderungsverfolgung und Kommentare eines Lektors aus
    Word werden zu Vorschlägen und Beiträgen unter seinem Namen, nie zu aktuellen Fassungen.
  - *Export:* Das Werk wird ein .docx für den Verlag. Offene Vorschläge gehen auf Wunsch als
    Word-Änderungen mit.
  - Nur erfundene Testdokumente verwenden.
- **Menschlicher Lektor mit eigenem Konto.**
  - Baruch bekommt Anmeldung und Rollen: **Autor** darf alles, **Lektor** liest, schlägt Fassungen vor,
    schreibt Befunde und Anmerkungen und übernimmt nichts. Freigabe je Werk.
  - Die bisherige Regel „Ändern/Löschen nur Mensch“ wird zu „nur der Autor“.
  - Die Herkunft aller Änderungen steht weiter im Spiegel.
  - Ohne Konten (eigener Rechner) bleibt es wie heute.
- **Ebenfalls aus der Einschätzung:**
  - ein schlanker Einstieg: Manuskript hinein, Vorschläge annehmen, PDF heraus;
  - Befunde nach Gewicht sortiert, Kleinkram zugeklappt (Nutzerfrage 14);
  - Anmerkungen zum ganzen Buch (Aufbau, Spannungsbogen, Zielgruppe) als eigene Ansicht, nicht nur im Chat.
- **Kein eigenes Zusammenspiel mit Logos Bible Software.** Viele Pfarrer arbeiten mit Logos, doch für die
  Recherche ist MultiGPT mächtiger (Nutzer 2026-11-24). Logos-Manuskripte kommen über den Word-Import herein.
- **Start nach dem Release 0.9.0.** Die Runden plant der PM nach der Stromregel in AGENTS.md. Über die
  Versionsnummer entscheidet der Nutzer.

## Rundenplan (Stand 2026-11-24, nach R24 – alle geplanten Runden erledigt)

Jeder offene Punkt weiter unten trägt seine Runde: „(→ R15b)“ = Runde 15, Aufgabe b. „(→ Nutzer)“ = Handprüfung im
Browser des Nutzers, „(→ Nutzerfrage)“ = braucht eine Entscheidung und ist nicht eingeplant, „(überholt: …)“ mit Beleg,
„(keine Aufgabe)“ = nur zur Kenntnis. Erledigtes ist unverändert.

Je Runde drei parallele Aufgaben (eigener Worktree, Zweig `team/<id>`) mit möglichst getrennten Dateien, ein Thema je
Runde; niedrige Gutachtenbefunde gehen in die Runde, die ihre Dateien ohnehin berührt. Reihenfolge: zuerst, was beim
Schreiben des echten Buchs (Werk 3 mit Teil I–IV) und der Predigten hilft – Gliederung, Lektor, Predigt, Bibelstellen,
Editor –, dann Pflege und Ausgabe (RAG, EPUB, Import), zuletzt Revisionen, Härtung und Verteilung. **EPUB rückt von
Runde 14 auf Runde 20:** das Buch-PDF steht seit Runde 11, EPUB hilft erst beim Veröffentlichen, und die Vorschau hängt
an Nutzerfragen (Kindle, EPUBCheck). Schätzung nach den bisherigen Dauern (`docs/Fahrplan.md`: Runden 7–10 80–120 min,
mit neuen Backends oder Abhängigkeiten 150–180 min, Runde 12 180 min; Gutachten und Nachbesserung eingerechnet).

| Runde | Thema | Aufgaben | Minuten |
|---|---|---|---|
| R13 | Quellen über MultiGPT, Websuche, Abgleich – **erledigt 2026-11-07** (≈ 165 min) | R13.0 bau (Vorlauf) · a quellen · b websuche · c abgleich | 210 |
| R14 | Teile und Gliederung – **erledigt 2026-11-07** (≈ 110 min) | a teile-typst · b teile-gliederung · c nummerierung | 120 |
| R15 | Lektor und Personas – **erledigt 2026-11-08** (≈ 300 min mit Vorlauf Nutzerfrage 12) | a lektor · b anbieter · c auftraege | 150 |
| R16 | Predigt (B4) – **erledigt 2026-11-08** (≈ 125 min) | a predigt-vorlage · b zeitwaechter · c stichwortzettel | 150 |
| R17 | Bibelstellen (SWORD-Rest), Reste Lektor – **erledigt 2026-11-08** (≈ 105 min) | a verszaehlung · b stellen · c pruefanzeige | 150 |
| R26 | Austauschformat zwischen Instanzen (XML), vorgezogen – **erledigt 2026-11-08** (≈ 133 min) | a ids · b export · c import | 180 |
| R25 | Gedankenkarte (Mindmap der Einfälle), vorgezogen – **erledigt 2026-11-08** (≈ 127 min) | a karte-daten · b karte-bild · c karte-widget | 150 |
| R18 | Editor und Menüband – **erledigt 2026-11-08** (≈ 135 min) | a menueband · b konstrukte · c satzbild-werk | 150 |
| R27 | Umschlag-Editor und Impressum in der Werkvorlage, vorgezogen vor R19 – **erledigt 2026-11-09** (≈ 130 min) | a umschlag-daten · b umschlag-satz · c umschlag-widget | 170 |
| R28 | Befunde: Befundliste je Abschnitt, Übersicht übers Werk, Aufträge im Chat, vorgezogen vor R19 – **erledigt 2026-11-09** (≈ 130 min) | a befunde · b befund-uebersicht · c chat-auftrag | 170 |
| R30 | Suchen/Ersetzen, Querverweise, Chat: Lösungsvorschläge, Varianten, Bedienung, vorgezogen vor R29 und R19 – **erledigt 2026-11-09** (≈ 200 min) | a suchen-verweise · b chat-loesungen · c chat-bedienung | 190 |
| R29 | Buchsatz in den Einstellungen, vorgezogen vor R19 – **erledigt 2026-11-09** (≈ 180 min) | a satz-daten · b satz-typst · c satz-oberflaeche | 160 |
| R19 | RAG-Pflege, Reste der Stellen – **erledigt 2026-11-09** (≈ 146 min) | a index · b pgvector · c treffer | 120 |
| R20 | EPUB-Vorschau – **erledigt 2026-11-09** (≈ 270 min ab PM-Abschluss R19, ≈ 115 ab Backlog-Commit 1bba744) | a epub · b reader · c epub-pruefung | 180 |
| R31 | Register und Glossar, vorgezogen vor R21 – **erledigt 2026-11-09** (≈ 195 min ab Plan `0f12415`) | a glossar-daten · b register-satz · c kontextmenue | 170 |
| R21 | Import und Bilder (B3), dazu EPUB-Cover mit Titel – **erledigt 2026-11-09** (≈ 170 min ab PM-Abschluss R31 `716b076`) | a pdf-import · b markdown-import · c bilder | 180 |
| R22 | Revisionen und Spiegel, dazu Fortschreiben, Rückfrage erledigter Befunde, Entwurf als Wasserzeichen – **erledigt 2026-11-10** (≈ 155 min ab S1 `dfff467` mit Gutachten und Nachbesserung, Vertrag davor ohne Commit) | fassungen · befund-rueckfrage · spiegel-entwurf | 160 |
| R23 | Härtung (mit Befund-Resten aus R28, R30, R29, R19, R20, R31, R21 und R22) – **erledigt 2026-11-24** (≈ 160 min mit Gutachten, Nachbesserung und PM-Abschluss) | a web · b mcp-namen · c geheimnisse | 200 |
| R24 | Verteilung (B5) – **erledigt 2026-11-24** (≈ 175 min ab der Vorarbeit `3028194` mit Gutachten, Nachbesserung und PM-Abschluss; ohne die niedrigen Reste aus R23) | a pakete · b ci · c webseite (statt betrieb) | 200 |

### R13 – Quellen über MultiGPT, Websuche, Abgleich (210 min) – erledigt 2026-11-07

Alle vier Aufgaben umgesetzt (`9b9d01c` … `1251a5b`, PM-Abschluss); Reste unter „Nach Runde 13 offen“, Abweichungen
dort und in `docs/Architektur.md`.

- **R13.0 `bau` (Vorlauf, ein Entwickler im Hauptordner, etwa 30 min, vor den Worktrees) – erledigt 2026-11-07
  (`target/` 38 → 9,0 GB):** Server-Tests zu wenigen
  Test-Binärdateien bündeln (`tests/main.rs` mit Modulen; die pgvector-Szenarien teilen einen Container), `du -sh
  target` vor und nach einem vollen Lauf messen; nur wenn das nicht reicht, LanceDB hinter ein Cargo-Feature. Erst
  danach drei Worktrees. – „Nach Runde 12 offen“ (Größe von `target/`, pgvector-Container).
- **R13a `quellen` – erledigt** (Rest: Verbindungsprüfung ohne `DELETE`, erledigt R19a): Volltext über MultiGPT `read_document` in der Dokumentansicht (`MultigptWay::read_full`,
  `literature.rs`, `source.html`), Prüfung eines Bereichs ohne Zitat gegen den Volltext (`baruch-library`
  `highlights.rs`); MultiGPT-Client: eine Sitzung statt einer je Suche, Werkzeugliste zwischenspeichern, Nachträge mit
  eigenem Zeitlimit, Sitzung mit `DELETE` beenden, ein gemeinsames Zeitlimit für die Verbindungsprüfung (`check.rs`);
  `round10_models.rs` nachziehen. Besitzt `connections/multigpt.rs`. – „Nach Runde 12 offen“, „Nach Runde 9 offen“,
  „Literatur als Popup (Rest)“.
- **R13b `websuche` – erledigt** (Lesezeichenliste weiter Nutzerfrage 2): @belege mit Websuche über MultiGPT (`web_search`, `fetch_url`) nach den Regeln aus „Werkzeuge
  der Personas“ (erst abrufen, dann nennen; nur Wortlaut der Seite mit URL und Abrufdatum; sonst „nicht belegt“),
  bevorzugte Quellen zuerst (`site:`, gekennzeichnet), gefundene Quellen auf Wunsch in die Autorenbibliothek; STUB
  `connections/sources.rs`; drei Zusicherungen aus Runde 9. Eigenes Modul neben `multigpt.rs` (Client nur benutzen). –
  „Werkzeuge der Personas“, „Nach Runde 9 offen“.
- **R13c `abgleich` – erledigt** (Kommentare/Befunde nicht hochgeladen, Nutzerfrage 11): Abgleich nach Plan „RAG-Abgleich mit MultiGPT“: `sync_state` (Migration), Baruch → MultiGPT je
  Kapitel in die Sammlung „Baruch: <Werk>“, MultiGPT → Baruch gewählte Sammlungen in Bibliothek und LanceDB-Index, Menü
  „Abgleichen“ mit „zuletzt abgeglichen“ (STUB `base.html`); Bibliothekstitel melden sich beim Index. Vorab prüfen, ob
  `list_documents` Änderungszeit oder Hash liefert. – „Nach Runde 12 offen“.

### R14 – Teile und Gliederung (120 min) – erledigt 2026-11-07

Alle drei Aufgaben umgesetzt und zusammengeführt (`1b4a14e`, `96d3df7`, `3f302d8`, Integration `b0fc5e4` `d235577`),
Gutachten (13 Befunde: 0 hoch, 5 mittel, 8 niedrig), Nachbesserung `bba23db`, PM-Abschluss; Reste unter „Nach Runde 14
offen“.

Anlass: Werk 3 „Herr prüfe mich“ hat Teil I–IV nur als Plan-Beiträge in den Kapiteln.
- **R14a `teile-typst`:** Teil als Markierung im Typst-Text, die `baruch_typst::parse` wieder als Teil erkennt
  (Rundreise byte-gleich), `baruch-wysiwyg` zieht mit; Export, Spiegel und Satz. – „Nach Runde 8 offen“ (Echte Teile).
- **R14b `teile-gliederung`:** ersten Teil vor einem Kapitel anlegen, auch mit Werkvorlage (Baum, RPC `outline.add`,
  MCP `outline_add` mit `level: 0`); Umbenennen ändert den Text des Teils, Umschalten gleicht die Gliederung ab;
  Gliederungs-Vorschläge der Assistenten mit Vorschau (auf den Anträgen aus 8+); `idea.process` als Gliederungspunkt
  atomar. – „Nach Runde 8 offen“, „Eine Gliederung (Rest)“, „Gliederungsmanager (Rest)“.
- **R14c `nummerierung`:** Nummerierung je Ebene aus einer Funktion, Schemata mit Wörtern beim Re-Import (juristisch,
  Predigt), Kapitel ohne Nummer in Typst, `;`/`|` in Vorlagenmustern, Abstände der Seite „Gliederungsschema“. – „Nach
  Runde 6 offen“, „Eine Gliederung (Rest)“.

### R15 – Lektor und Personas (150 min) – erledigt 2026-11-08

Vorlauf Nutzerfrage 12 (`ca04c58`), Schnittstellen R15-S1/S2, drei Aufgaben (`9ee4362`, `8699f18`, `dfda162`),
Zusammenführung `b440f36` `e1d307f` `58f4325`, Integration `ebb85c2`, Gutachten (15 Befunde: 0 hoch, 7 mittel,
8 niedrig), Nachbesserung `ace0e36` (alle mittleren), PM-Abschluss. Dazu die Nutzerwünsche 2026-11-07 „Bibeln in der
Literatur“ und „SWORD als Recherchewerkzeug“ (Kommentare, Lexika, Faktenblock). Reste: „Nach Runde 15 offen“.

- **R15a `lektor`:** ganzes Kapitel bzw. ganze Predigt im Kontext (soweit die Kontextlänge reicht), Befunde der anderen
  Personas gesammelt, Prüfen/Vorschlag für ein Kapitel; „Kritisch:“ prüfen statt nur erbitten (fehlt er, nachfordern;
  im Vorschlag entfernen), verbotene Einleitungen nicht beim Vorschlag; neun Zusicherungen; Handprüfung mit Werk 3,
  Kapitel I und langen Antworten (`assist/context.rs`, `assist/prompts.rs`). – „Nach Runde 10 offen“.
- **R15b `anbieter`:** Verbindung je Persona (Eliezer, OpenAI-kompatibel, Ollama), Modellliste aus `/v1/models` der
  Verbindung, „lokal“/„extern“ je Persona mit Freigabe (Vorgabe aus), Kontextlänge gegen das Backend abgleichen,
  Modellliste aus dem abgelaufenen Zwischenspeicher bei Fehler, Kurztest gespeichert, ein HTTP-Client je Endpunkt,
  `persona.list` ohne Vorgabewerk (`models.rs`, `personas.rs`, `baruch-llm`, Einstellungen → Assistenten; Migration). –
  „Nach Runde 10 offen“, „Nach Runde 8 offen“, „Modelle der Personas“.
- **R15c `auftraege`:** Chat-Strom zeigt den eigenen Beitrag sofort, ohne JavaScript Seite mit Hinweis statt 422,
  Anzeige bei neu angeforderter Einleitung, richtige Meldung beim Abbrechen; Einfall verarbeiten mit Persona-Auswahl
  und Herkunft beim Einfall als Beitrag; Literatur „an Persona geben“ (STUB `literature.rs`/`literature.ts`),
  Persona-Hervorhebung als eigene übernehmen; MCP `chat` mit Faden ohne Abschnitt und Relay (STUB `mcp/tools.rs`),
  Warteschlange je Client, Abzeichen „neu“. – „Nach Runde 10 offen“, „Nach Runde 8 offen“, „Literatur als Popup
  (Rest)“.

### R16 – Predigt (B4) (150 min) – erledigt 2026-11-08

Schnittstelle R16-S1 `1db283d` (per cherry-pick `8061b24`), drei Aufgaben (`136bffd`, `1d6178a`, `7f0a579`),
Zusammenführung `7c7ecbb` `f8de6a1` `a071a46`, Integration `b7228b4` mit zwei Anfragen an Eliezer, Gutachten
(13 Befunde: 0 hoch, 4 mittel, 9 niedrig), Nachbesserung `7dc257c` (alle mittleren), PM-Abschluss. Dazu Entscheidung 2
des Nutzers nach Runde 15 und die „(→ R16)“-Reste aus „Nach Runde 15 offen“. Reste: „Nach Runde 16 offen“.


- **R16a `predigt-vorlage`:** Werkvorlage Predigt (Text, Einleitung, Teile, Anwendung, Schluss) mit Gliederungsschema,
  Anlegen mit dem Predigttext aus der SWORD-Bibel des Werks. – `docs/Plan.md` „Stufen“ B4.
- **R16b `zeitwaechter`:** Sprechdauer je Abschnitt und gesamt aus Wörtern je Minute (je Werk einstellbar), Zielzeit
  mit Warnung, Anzeige in Gliederung und Editorkopf, RPC und MCP. – Plan B4.
- **R16c `stichwortzettel`:** Export als Stichwortzettel (Typst-Satz über `baruch-render`, PDF), Download, RPC, CLI. –
  Plan B4.
- (Nutzer 2026-11-08, Entscheidung 2 nach Runde 15) **Protokoll-Commit beim Ablehnen für alle Antragsarten:** Wie bei
  `add` schreibt auch das Ablehnen bzw. „veraltet“ von Umbenennen, Verschieben, Umstufen, Nummer an/aus und Löschen
  einen leeren Spiegel-Commit „Gliederung: Antrag N abgelehnt“ mit `Baruch-Decision` und `Baruch-Requested-By`.
  Entscheidung 1: Der Zustimmungsdialog bleibt mit „Später entscheiden“ und Escape schließbar (so umgesetzt).

### R17 – Bibelstellen (SWORD-Rest) (150 min) – erledigt 2026-11-08

Schnittstellen R17-S1 `c9777da`, R17-S2 `2e12c10`, R17-S3 `8b6f65e` (je per Cherry-pick in den anderen Zweigen), drei
Aufgaben (`99e0811`, `85ce28b`, `13a18b3`), Zusammenführung `df455b4` `bc76d1a` `3175ba5`, Integration mit zwei
Anfragen an Eliezer `188989f`, Gutachten (16 Befunde: 1 hoch, 8 mittel, 7 niedrig), Nachbesserung `322c889` (alle
hohen und mittleren behoben), PM-Abschluss (zwei Begründungen, `cargo fmt`, Backlog). Reste: „Nach Runde 17 offen“.

Nach Runde 16 auf 150 min angehoben: Zu den Bibelstellen kommen die Lektor-Reste aus „Nach Runde 16 offen“
(Zitatprüfung, Stellenangaben in Befunden, wirksame Kontextlänge, Rahmung des Gedächtnisses).


- **R17a `verszaehlung`:** weitere Verszählungen und die Abbildung zwischen ihnen (STUB `versification.rs`), englische
  Angaben auf die deutsche Zählung. – „SWORD-Bibeln“, „Nach Runde 10 offen“.
- **R17b `stellen`:** Verslisten mit Komma („Joh 3,16, 18“), „Johannes 3 Mal“, Uhrzeiten („Mi 19:30“), Zitat-Zuordnung
  über Absätze, verschachtelte Anführungszeichen und `#szene`. – „Nach Runde 10 offen“, „SWORD-Bibeln“.
- **R17c `pruefanzeige`:** Ergebnis der Wortlautprüfung im Editor (Zitate markiert „stimmt“/„weicht ab“),
  verschlüsselte Module (STUB `baruch-sword`, Schlüssel im Schlüsselbund), BZIP2/XZ, Lizenzvermerk sichtbar. –
  „SWORD-Bibeln“.

### R26 – Austauschformat zwischen Instanzen: XML (180 min) – vorgezogen vor R25 – erledigt 2026-11-08

Plan `f43c44f`, Schnittstelle R26-S1 `a0c0c86` direkt auf `development` (die Zweige zweigten davon ab, kein
cherry-pick), drei Aufgaben (`59ff36f` ids, `255cb6a` export, `14a5af1` import), Zusammenführung `0a1e010` `e24bcdf`
`ea6ed61` ohne Konflikte, Integration `8d0fa86` mit Ende-zu-Ende-Prüfung über zwei Wegwerf-Instanzen und
Sichtprüfung, Gutachten (11 Befunde: 1 hoch, 6 mittel, 4 niedrig), Nachbesserung `1c4a3ca`, PM-Abschluss. Reste:
„Nach Runde 26 offen“.

Wunsch des Nutzers (2026-11-08): „Vorher brauchen wir ein Austauschformat, das zwischen Instanzen lebt. Ich schlage
XML vor.“ Läuft **vor** der Gedankenkarte (R25), damit deren Verbindungen und Lagen gleich ins Format gehören. Die
Nummer ist nur ein Name.

**Befund:** Heute gibt es nur den Typst-Export (`main.typ`, eine Fassung je Abschnitt) und den Git-Spiegel. Beides
trägt weder alle Fassungen noch Einfälle, Beiträge, Buchangaben oder Herkunft. Alle Schlüssel sind Zahlen der
eigenen Datenbank (`work.id`, `node.id` = key, `idea.id` …); auf einer zweiten Instanz bedeuten sie etwas anderes.
Es gibt keine UUIDs und keine XML-Bibliothek im Workspace.

**Ziel:** Ein Werk verlässt eine Baruch-Instanz vollständig und kommt auf einer anderen (oder später wieder auf
derselben) so an, dass nichts verloren geht und ein zweiter Austausch dasselbe Werk **abgleicht**, statt ein Doppel
anzulegen.

**Festlegungen (Vorschlag des Hauptagenten, änderbar):**

- **Container `.baruch`** (ZIP wie ODF/EPUB): zuerst unkomprimiert `mimetype` = `application/vnd.baruch.werk+zip`,
  dann `werk.xml`, `bilder/<name>` (Bilder als Dateien, nicht Base64), `MANIFEST.xml` mit SHA-256 je Datei. Daneben
  „nur XML“ (`.baruch.xml`, Bilder Base64 im XML) für kleine Werke und zum Lesen.
- **XML:** Namensraum `urn:baruch:werk:1`, Attribut `version="1"`, UTF-8, ein **XSD** im Repo
  (`docs/schema/baruch-werk-1.xsd`), Tests prüfen jede Ausgabe mit `xmllint --schema`. Spätere Versionen nur
  abwärtsverträglich ergänzen; Unbekanntes beim Einlesen melden, nicht stillschweigend verwerfen.
- **Globale Kennungen:** UUID v4 für Werk, Gliederungspunkt, Fassung, Vorlagenfassung, Einfall, Beitrag, Bild(fassung)
  (und ab R25 Verbindung). Neue Spalten `uuid` (Migration 0041, eindeutig, für Bestandsdaten beim Start vergeben).
  Zahlen-ids bleiben intern; im XML verweist alles über UUIDs.
- **Inhalt:** Werk (Titel, Untertitel, Autor, Art, Zielgruppe, Bibel, Schema samt eigenem Muster, Buchangaben,
  Umschlag); Werkvorlage mit allen Fassungen; Gliederung als Baum (Teil/Kapitel/Abschnitt, Nummer an/aus) mit **allen
  Fassungen** je Abschnitt (Text, Autor, Herkunft, Zeit, „aktuell“-Markierung); Vorschläge der Assistenten als das,
  was sie sind; Einfälle mit Schlagwörtern, Bezug, Status, Herkunft, gelöschte mit Zeitpunkt; Beiträge und Fäden am
  Abschnitt; Bilder mit Fassungen, Bildunterschrift, Alternativtext; Sprechdauer-Einstellungen. Ab R25: Verbindungen
  und Lagen der Gedankenkarte.
- **Nicht im Format:** Schlüssel, Token, Verbindungen, Personas mit Modellen (Instanz-Einstellung; nur Handles als
  Herkunft), offene Anträge (Arbeitsablauf der Instanz), Bibel-, Kommentar- und Literaturtexte (nur Verweise:
  Modulname, Stelle, Titel), Suchindex.
- **Einlesen in zwei Arten:** (1) **neu** – unbekannte Werk-UUID → neues Werk (Titelregeln wie beim Import);
  (2) **abgleichen** – bekannte Werk-UUID → fehlende Gliederungspunkte, Fassungen, Einfälle, Beiträge, Bilder kommen
  dazu; **nie wird etwas gelöscht oder überschrieben** (Regel „Ändern/Löschen nur Mensch“): weicht die aktuelle Fassung
  ab, wird die eingelesene eine weitere Fassung, die Markierung „aktuell“ bleibt, wie sie ist, und eine Liste
  „Abweichungen“ zeigt, wo der Mensch umschalten kann. Gliederungsänderungen (Umbenennen, Verschieben) aus der
  anderen Instanz werden **Anträge**. Vorher gibt es immer eine **Vorschau** (was käme dazu, was weicht ab), erst
  „Übernehmen“ schreibt, in einer Transaktion und einem Commit im Spiegel mit Herkunft „Austausch aus <Instanz>“.
- **Sicherheit:** kein DTD, keine Entitäten, keine externen Verweise (XXE), Grenzen für Größe, Tiefe, Zahl der
  Elemente und entpackte Bytes (wie die 64-MiB-Grenze von R17), Pfade im ZIP ohne `..`/absolute Pfade, Prüfsummen aus
  dem Manifest müssen stimmen; Bilder durch dieselbe Prüfung wie beim Hochladen.
- **Wege:** Werk ▾ → „Austausch: Ausgeben“ / „Austausch: Einlesen…“ (ohne JavaScript, Upload mit Vorschauseite);
  RPC `work.exchange_export`, `work.exchange_preview`, `work.exchange_import`; CLI `baruch-server export <id> --baruch
  DATEI` und `baruch-server import DATEI.baruch [--abgleichen]`; MCP nur `exchange_export` (lesend) – Einlesen ändert
  Werke und bleibt beim Menschen.

**Aufgaben:**

- **R26a `ids`:** Migration 0041 (UUID-Spalten, eindeutig, Bestand beim Start füllen, neue Zeilen bekommen sofort
  eine), Store-Zugriff über UUID, Instanzkennung (`store_meta.instance_uuid`, Name in Einstellungen). Schnittstelle
  R26-S1 direkt auf `development` vor den Worktrees: Rust-Typen des Formats (`Exchange*`), XSD-Entwurf, Fehlertypen.
- **R26b `export`:** Crate `baruch-exchange` (XML schreiben mit `quick-xml`, ZIP mit `zip`, Manifest), XSD fertig,
  Ausgabe über Menü/RPC/CLI/MCP, Prüfung mit `xmllint --schema` in Tests, Doku `docs/Austauschformat.md` (jedes
  Element mit Beispiel, Deutsch).
- **R26c `import`:** Einlesen sicher (Grenzen oben), Vorschau, „neu“ und „abgleichen“ nach den Regeln oben, Anträge
  für Gliederungsänderungen, Commit mit Herkunft; Rundreise-Tests: Export → Import (neu) → Export ist bis auf
  Zeitstempel der Ausgabe gleich; zweiter Import gleicht ab und legt nichts doppelt an; geänderte Fassung auf
  Instanz B kommt auf A als weitere Fassung an.

**Abnahme:** zwei Wegwerf-Instanzen (`--data`), erfundenes Werk mit Teilen, mehreren Fassungen, Einfällen, Beiträgen,
Bildern und Umschlag; Hin- und Rückweg ohne Verlust; manipuliertes ZIP (`..`-Pfad, falsche Prüfsumme, XML-Bombe,
DTD) wird abgelehnt, ohne etwas zu schreiben.

**Nutzerfragen (nicht blockierend, Vorgabe in Klammern):** (1) Soll das Format auch die Autorenbibliothek (Titel,
Zitate, Hervorhebungen) tragen? (nein, eigenes Format später); (2) Ist ein Abgleich in beide Richtungen ohne Datei,
direkt Instanz zu Instanz über das Netz, gewünscht? (später, über dieselben Elemente).

### R25 – Gedankenkarte: Mindmap der Einfälle (150 min) – vorgezogen vor R18 – erledigt 2026-11-08

Schnittstelle R25-S1 `c0e5e8a` direkt auf `development`, drei Aufgaben (`be8c243` karte-daten, `0cb92f6` karte-bild,
`8baa26a` karte-widget), Zusammenführung `1d5ec7f` `4e6d9e7` `9d70fec` ohne Konflikte, Integration `ff8d3d6` mit
Abnahmetest `tests/round25_abnahme.rs`, Sichtprüfung `61eae1b`, Gutachten (13 Befunde: 4 mittel, 9 niedrig),
Nachbesserung `fc2ca78`, PM-Abschluss. Abweichend von der Planung unten: Migration 0043 (nicht 0042), „neuer Teil als
Antrag mit Kennung und Fassungen“ nicht in R25 (erledigt R22: Antrag mit Kennung und Fassungen). Reste: „Nach Runde 25 offen“.

Wunsch des Nutzers (2026-11-08): „Plane das Mindmap-Tool, es wird ob der Einfälle existenziell.“ Die Nummer R25
ist nur ein Name; die Runde läuft nach R26 (Austauschformat) und vor R18 (die Nummern der späteren Runden bleiben).
Verbindungen (`idea_link`) und Lagen bekommen eine UUID und gehören ins Austauschformat (Element in `werk.xml`).

**Befund (Daten 2026-11-08, nur lesend):** Werk 3 hat 33 Einfälle mit 155 Schlagwörtern, davon 142 verschieden; nur
„schwarzes Schaf“ (4) und „Festung“ (3) kommen öfter als zweimal vor. Werk 2 hat 47 Einfälle. 5 Einfälle in
Werk 3 hängen an keinem Abschnitt, einer widerspricht einem anderen (48 „Für CLV …“ gegen 79 „Verlag“). Eine Karte
nur über gleiche Schlagwörter bliebe darum fast ohne Kanten. Was trägt: der Bezug zur Gliederung, die
Bedeutungsnähe (Einfälle liegen schon im LanceDB-Index, `idea.search` mit `mode=rag`) und Verbindungen, die man von
Hand zieht.

**Ziel:** eine Seite **Werkzeuge ▾ → Gedankenkarte** (`/werke/<id>/karte`), die alle Einfälle eines Werks als Karte
zeigt, damit man sieht, wo Gedanken sich ballen, was fehlt, was sich widerspricht, und Einfälle per Ziehen ordnet.

**Festlegungen (Vorschlag des Hauptagenten, änderbar):**

- **Aufbau radial:** in der Mitte das Werk, im ersten Ring Teile und Kapitel (Gliederung), außen die Einfälle an
  ihrem Abschnitt. Einfälle ohne Abschnitt stehen in einer Wolke „Ohne Kapitel“, Einfälle ohne Werk erscheinen nicht.
  Abschnitte unter Kapiteln werden zu ihrem Kapitel zusammengefasst (aufklappbar).
- **Kanten, drei Arten, unterscheidbar nicht nur durch Farbe** (Strichart und Beschriftung):
  1. **Bezug** Einfall → Abschnitt (aus `idea.node_id`, durchgezogen);
  2. **Verbindung** Einfall ↔ Einfall, von Mensch oder Assistent gezogen, mit Art: „führt weiter“, „gehört zu“,
     „Beispiel für“, „widerspricht“ (neue Tabelle, siehe a), mit kurzer Notiz;
  3. **Nähe** (gestrichelt, nur Anzeige, nicht gespeichert): je Einfall die bis zu 3 ähnlichsten anderen über
     LanceDB, ab einer Schwelle, ein Schalter blendet sie aus. Ohne Einbettungsmodell fehlen sie, die Seite sagt es.
     Gleiche Schlagwörter zählen als Nähe mit (Schlagwort-Schlüssel `word_key`).
- **Zeigen, was fehlt:** Kapitel ohne Einfälle hervorgehoben, Kapitel mit vielen offenen Einfällen mit Zahl;
  „widerspricht“ rot und als Liste „Widersprüche“ unter der Karte.
- **Filter:** offen/verarbeitet/alle, Schlagwort, Herkunft (Mensch, Persona, MCP), Volltext (hebt Treffer hervor,
  blendet den Rest ab).
- **Ordnen per Ziehen:** einen Einfall auf ein Kapitel ziehen setzt den Bezug (`idea.update`, ein Commit wie bisher);
  vom einen Einfall zum anderen ziehen legt eine Verbindung an (Art wählen). Kein Buchtext ändert sich, darum ohne
  Antrag (Einfälle sind CRUD für alle, AGENTS.md). Positionen: Anordnung automatisch; von Hand verschobene Einfälle
  behalten ihre Lage je Werk (gespeichert), „Anordnung zurücksetzen“ verwirft sie.
- **Ohne JavaScript:** Der Server rechnet die Anordnung in Rust und liefert die Karte als SVG im HTML (Links auf
  `/einfaelle/<id>` und Abschnitte), darunter dieselbe Karte als **Gliederungsliste** (Kapitel → Einfälle →
  Verbindungen) mit Formularen „Bezug setzen“ und „Verbinden mit …“. JavaScript bringt Zoomen, Verschieben, Ziehen,
  Tastatur (Pfeile wandern über Knoten, Enter öffnet, `V` verbindet) und Seitenleiste mit der Einfallkarte.
- **Ausgabe:** SVG und PDF der Karte; Markdown-Gliederung; **FreeMind `.mm`** zum Weiterbearbeiten in freien
  Mindmap-Programmen (nur Ausgabe, Einlesen später).
- **MCP:** `idea_map` (Karte als JSON: Knoten, Bezüge, Verbindungen, auf Wunsch Nähe), `idea_link`,
  `idea_unlink`, `idea_links` – Assistenten dürfen verbinden (Herkunft im Spiegel, wie bei Einfällen). Damit kann ein
  Assistent Gruppen und Widersprüche vorschlagen, ohne Buchtext zu berühren.
- **Spiegel:** Verbindungen stehen in `einfaelle.md` beim Einfall („Verbindungen: → 79 widerspricht“), jede Änderung
  ein Commit mit Herkunft.

**Aufgaben:**

- **R25a `karte-daten`:** Migration 0042 `idea_link` (`from_id`, `to_id`, `kind` mit CHECK, `note`, Herkunft, `created`,
  `deleted` wiederherstellbar; eindeutig je Paar und Art, kein Selbstbezug; ON DELETE CASCADE) und `idea_map_pos`
  (Werk, Einfall, x, y); Store, RPC `idea.link/unlink/links`, `idea.map` (Knoten, Kanten, Nähe über LanceDB mit
  Schwelle und Höchstzahl, ehrlich „ohne Einbettung“), `idea.map_pos_set/reset`; MCP `idea_map/idea_link/idea_unlink/
  idea_links`; Spiegel `einfaelle.md`; gelöschte Einfälle fallen aus Karte und Verbindungen und kommen mit ihnen
  zurück. Doku RPC/MCP/Architektur.
- **R25b `karte-bild`:** Anordnung in Rust (radial, deterministisch, keine Überlappung bis 300 Einfälle, gemessen),
  Seite `/werke/<id>/karte` mit SVG im HTML und Gliederungsliste, Filter per GET, Formulare ohne JavaScript, Menüpunkt
  Werkzeuge ▾ und Link von `/einfaelle`, Ausgabe SVG/PDF/Markdown/FreeMind (`/werke/<id>/karte.svg|.pdf|.md|.mm`),
  Druck-CSS, Hell/Dunkel. Doku Bedienung.
- **R25c `karte-widget`:** TypeScript-Widget `data-widget="idea-map"` auf dem SVG: Zoomen/Verschieben, Ziehen auf
  Kapitel (Bezug) und auf Einfall (Verbindung mit Auswahl der Art), Lage merken, Seitenleiste mit Einfall bearbeiten,
  Tastatur und `aria`, Nähe ein/aus, Schnellerfassung Alt+E legt neue Einfälle direkt in der Karte an. Doku Bedienung.
- **Schnittstelle R25-S1 direkt auf `development` vor den Worktrees** (Lehre aus R17): Typen `IdeaMap`, `IdeaLink`,
  `LinkKind`, RPC-Signaturen, leere Migration-Nummer reserviert.

**Abnahme:** Werk mit 40 erfundenen Einfällen (Tests nie mit echten Buchdaten): Karte ohne JavaScript vollständig
bedienbar; Ziehen setzt Bezug mit Commit; Verbindung „widerspricht“ erscheint in Karte, Liste, Spiegel und MCP;
Nähe mit MockEliezer-Einbettungen; 300 Einfälle ohne Überlappung unter 200 ms Anordnung; FreeMind-Datei von
`xmllint` als wohlgeformt erkannt.

**Nutzerfragen (nicht blockierend, Vorgabe in Klammern):** (1) Auch werkübergreifende Karte (alle Werke, Einfälle
ohne Werk)? (später); (2) Soll ein Assistent auf Wunsch Cluster benennen („Gruppe: Scham“) als Vorschlag in der
Karte? (ja, als Stub `// STUB:` in R25, umgesetzt in R19 mit der RAG-Pflege).

### R18 – Editor und Menüband (150 min) – erledigt 2026-11-08

- **R18a `menueband`:** Menüband für Editor und Satz-Ansicht aus `fragments/edit_toolbar.html`, Formate der Vorlage als
  Knöpfe (`#szene`, `#arbeit`, Fußnote, Zitat, Liste); Satz-Seite: `aria-label`, „neu“ nicht nur als Farbe, ohne
  `main.js`, Typst-Lauf abbrechbar und nicht von fremden Seiten auslösbar, Reste der Typst-Vorschau; Kopfzeile in
  schmalen Fenstern samt Test; Gedankenkarte: Kanten mit Beschriftung und Pfeil, Seitenleiste ohne Überdeckung. –
  „Menüband“, „WYSIWYG-Editor“, „Nach Runde 8 offen“, „Nach Runde 9 offen“, „Nach Runde 25 offen“.
- **R18b `konstrukte`:** mehr Konstrukte statt Inseln (Aufzählungen, Begriffslisten, geschachtelte Listen, `\`,
  `#link`, Verweise), Inseln an Ort und Stelle bearbeitbar, mehrere `image()` in einem Block, `image_ref` mit
  führendem `/`, `guardHeadingMerge` über `.pv` (`baruch-wysiwyg`, `wysiwyg.ts`). – „WYSIWYG-Editor“, „Nach Runde 8
  offen“.
- **R18c `satzbild-werk`:** Satzbild-Einstellungen je Werk (Schrift, Farben, Satzbreite; Migration), Stil schon im HTML,
  lange Abschnitte nur auf Anfrage; Bild an der Cursorstelle einfügen, Pfad erst mit der Fassung; Reste der Vorschläge
  (Alle einklappen im Satzbild, leere „−“-Zeile, LCS); Literatur-Treffer im Satzbild markieren; Verhaltenstest
  „Satzbild als Vorgabe“, Zusicherungen `round8_*`. – „WYSIWYG-Editor“, „Nach Runde 11/8/6 offen“, „Bilder“.

### R27 – Umschlag-Editor und Impressum in der Werkvorlage (170 min) – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (Schnittstelle `cd54cbb`, Zweige `f9cb541`, `d39145f`, `1d72a86`, Zusammenführen und
Abnahme des Integrators `d2f19b9`, Nachbesserung `e2fe08a`, PM-Abschluss); Reste unter „Nach Runde 27 offen“.
**Abweichung vom Plan:** Der Satzbild-Punkt aus „Nach Runde 18 offen“ war „→ R26-Rest bzw. nächste Unterversion“
eingeplant, nicht für R27; er kam in R27a mit, weil Unterversion 3 ohnehin anstand (Austauschformat `<satzbild>`).
Gutachten: 13 Befunde (0 hoch, 2 mittel, 11 niedrig); beide mittleren in `e2fe08a` behoben (Grundschrift wie Typst
über `baruch_wysiwyg::template_font`, Handprüfung in neun Schritten), drei niedrige vom PM (MCP-Hinweis nennt
`cover_layout`/`cover_layout_set`, „im Aufbau“ in Bedienung, Begründung einer Zusicherung), einer irrig (Herkunftstest
für die neuen Routen besteht: `round27_widget.rs` `foreign_sites_get_403_without_typesetting`), die übrigen eingeplant.

Wunsch des Nutzers (2026-11-08): „Wir brauchen einen Cover-Editor, der den Text verschieben lässt. Fonts sind
optional.“ Die Nummer ist nur ein Name; die Runde läuft nach R18 und vor R19.

**Befund:** Seit Runde 11 setzt Baruch Vorderseite, Rücken und Rückseite fest: Titel und Untertitel nur in einem von
drei Bereichen (`title_area` oben/Mitte/unten), Rückseite mit Klappentext, Kurzbiografie, ISBN/EAN-13, Verlag und
Preis an festen Stellen. Beim echten Buch (Werk 3, Bild mit dunklem oberen Drittel) reicht das nicht.

**Ziel:** Auf der Umschlagseite (`/werke/<id>/umschlag`) lassen sich alle Textblöcke des Umschlags frei setzen, und
Vorschau, Buch-PDF und Umschlag-PDF zeigen genau dieselbe Lage.

**Festlegungen (Vorschlag des Hauptagenten, änderbar):**

- **Blöcke:** Vorderseite Titel, Untertitel, Autor, Verlag (optional); Rücken Titel, Autor, Verlag; Rückseite
  Motto/Bibelwort (optional), Klappentext, Kurzbiografie (mit Foto), ISBN mit Strichcode, Verlag und Preis. Jeder Block
  hat Lage (x, y in mm relativ zu seiner Fläche: Vorderseite, Rücken, Rückseite), Breite, Ausrichtung
  (links/Mitte/rechts/Blocksatz), Schriftgröße (pt), Zeilenabstand, Farbe (hell/dunkel/eigene), sichtbar ja/nein.
  Rücken: Laufrichtung (von unten nach oben / oben nach unten).
- **Schriften optional:** Vorgabe ist die Schrift der Werkvorlage. Wahl aus den Schriften, die Typst kennt
  (eingebettet und System, `baruch-render::fonts`), mit Schnitt (normal/fett/kursiv); fehlt eine Schrift auf dem
  Rechner, fällt der Satz ehrlich auf die Vorgabe zurück und die Druckprüfung sagt es.
- **Lesbarkeit:** Je Block optional ein Schatten oder ein Verlauf dahinter (wie heute „heller Text auf dunklem
  Verlauf“). Die Druckprüfung misst den Kontrast unter dem Block im Umschlagbild und warnt bei zu wenig.
- **Hilfslinien:** Beschnitt, Schnittkante, Sicherheitsabstand (Vorgabe 5 mm), Rückenkanten, Mitte; Blöcke rasten an.
  Warnung, wenn ein Block in Beschnitt oder Sicherheitsabstand ragt, über den Strichcode läuft oder aus dem Rücken
  fällt (Rückenbreite aus der Seitenzahl wie bisher).
- **Vorlagen:** „oben/Mitte/unten“ bleiben als Schnellwahl (setzen die Blöcke der Vorderseite), dazu
  „Anordnung zurücksetzen“. Ein Werk ohne eigene Anordnung bleibt genau wie bisher (Tests).
- **Bedienen:** Mit JavaScript ziehen und an den Ecken die Breite ändern, Pfeiltasten verschieben um 0,5 mm (mit
  Umschalt 5 mm), Seitenleiste für die Werte, Rückgängig im Editor. Ohne JavaScript dieselben Werte als Formular mit
  Zahlenfeldern und Vorschau per GET, Übernehmen per POST. Die Vorschau ist die von Typst gesetzte SVG (dieselbe
  Quelle wie das PDF), die Blöcke liegen als Rahmen darüber.
- **Speichern:** neue Tabelle `cover_block` (Werk, Block, Werte; UUID mit Fülltrigger und Grabstein wie 0041/0043),
  ein Commit „Umschlag: Anordnung geändert“ im Spiegel; Teil des Austauschformats (Unterversion 3, abwärtsverträglich).
- **Rechte:** Anordnung ist kein Buchtext, aber Umschlag (Runde 11: Umschlag setzen nur als Antrag). Darum: Mensch
  ändert direkt; MCP liest (`cover_layout`) und stellt höchstens einen Antrag `cover_layout_set` (bestehende
  Antragsmechanik der Bilder), den der Mensch bestätigt.

**Impressum in der Werkvorlage (Wunsch 2026-11-08: „Weiterhin muss die Seite mit den rechtlichen Angaben gesetzt
werden im Template.“):** Heute baut Baruch die Impressumsseite fest im Code (`baruch-typst/src/cover.rs`, Zeilen des
Impressums), nur mit „Titelei setzen“, und die Werkvorlage hat darauf keinen Einfluss. Künftig:

- Baruch reicht die Buchangaben als Typst-Wörterbuch an die Werkvorlage (`buch` mit Titel, Untertitel, Autor, Verlag,
  Anschrift, Ort, Jahr, Auflage, ISBN, Druck, Bibelübersetzung mit Rechtehinweis, Bildnachweis, ©-Zeile, Hinweis der
  Deutschen Nationalbibliothek, „Gesetzt mit Baruch“); Texte werden als Typst-Zeichenketten übergeben, nie als Code.
- Die Impressumsseite setzt eine Funktion `impressum(buch)` **in der Werkvorlage**. Neue Werke bekommen sie in ihrer
  Vorlage mit; bestehende Werke ohne diese Funktion bekommen die bisherige Seite (Rückfall, byte-gleich), und ein Knopf
  „Impressum-Vorlage in die Werkvorlage übernehmen“ legt sie als neue Vorlagenfassung an (Mensch, Commit). Ebenso für
  Schmutztitel, Haupttitel und Widmung (`schmutztitel(buch)`, `haupttitel(buch)`, `widmung(buch)`).
- Die Impressumsseite steht im Satz an fester Stelle (Rückseite des Haupttitels), auch wenn die Vorlage sie umgestaltet.
- **Pflichtangaben:** neue Felder Anschrift des Verlags bzw. bei Selbstverlag des Verfassers/Herausgebers und
  Anschrift der Druckerei (Landespressegesetze verlangen Name und Anschrift von Drucker und Verleger); die
  Druckprüfung warnt, wenn bei eingeschalteter Titelei Verlag/Herausgeber mit Anschrift, Druckerei, Jahr oder bei
  Bibelzitaten der Rechtehinweis der Übersetzung fehlen. Baruch gibt keine Rechtsberatung; die Doku nennt die
  Angaben als üblich und verweist auf die Prüfung durch den Verlag.
- Aufgabenverteilung: Felder und Migration in R27a, Typst-Übergabe, Vorlagenfunktionen, Rückfall und Druckprüfung in
  R27b, Knopf und Formularfelder in R27c (Einstellungen → Werk und Umschlagseite).

**Aufgaben:**

- **R27a `umschlag-daten`:** Migration (nach der höchsten vorhandenen) `cover_block`, Store, RPC `cover.layout`,
  `cover.layout_set`, `cover.layout_reset`, `cover.fonts`; MCP `cover_layout` (lesen) und Antrag
  `cover_layout_set`; Austauschformat Unterversion 3 (XSD, Doku, Rundreise/Abgleich); Spiegel.
- **R27b `umschlag-satz`:** Typst-Satz von Vorderseite, Rücken und Rückseite aus den Blöcken (`place` mit dx/dy, Breite,
  Drehung im Rücken), Schriften mit Rückfall, Schatten/Verlauf, gleich in Umschlagseite, Umschlag-PDF und Buch-PDF;
  Druckprüfung (Beschnitt, Sicherheitsabstand, Strichcode, Rücken, Kontrast, fehlende Schrift); ohne Anordnung
  byte-gleich wie bisher.
- **R27c `umschlag-widget`:** Seite/Editor auf `/werke/<id>/umschlag` (Hilfslinien, Rahmen über der Typst-SVG,
  Seitenleiste), TypeScript-Widget `data-widget="cover-editor"` (Ziehen, Breite, Pfeiltasten, Einrasten, Rückgängig),
  Formular ohne JavaScript, Schnellwahl und Zurücksetzen, Hell/Dunkel, Tastatur und aria.

**Abnahme:** erfundenes Werk mit erfundenem Umschlagbild (dunkles oberes Drittel): Titel ins obere Drittel ziehen,
Autor nach unten, Klappentext auf der Rückseite verschieben und schmaler machen → Vorschau, Umschlag-PDF und Buch-PDF
zeigen dieselbe Lage (Koordinaten im PDF gemessen); Block im Beschnitt und zu wenig Kontrast werden gewarnt; fehlende
Schrift fällt zurück mit Hinweis; ohne JavaScript dieselben Ergebnisse; Austausch zwischen zwei Instanzen trägt die
Anordnung; ein Werk ohne Anordnung setzt unverändert.

**Nutzerfragen (nicht blockierend, Vorgabe in Klammern):** (1) Eigene Schriftdateien hochladen? (später, mit Lizenzhinweis);
(2) Freie Zusatztexte auf dem Umschlag (z. B. ein Zitat auf der Vorderseite)? (ja, ein Block „Freitext“ je Fläche).

### R28 – Befunde: Befundliste, Übersicht übers Werk, Aufträge im Chat (170 min) – vorgezogen vor R19 – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (Schnittstelle `cb7b169`, Zweige `f46a7a5` befunde, `5e46999` befund-uebersicht,
`07a986f` chat-auftrag, Zusammenführung `ee7a5af` `5f3f656` `afe1399` – Konflikt nur in einer Tabellenzeile von
`docs/MCP.md` –, Integrationscommit mit Abnahme `tests/round28_abnahme.rs`); Reste unter „Nach Runde 28 offen“.
Nutzerfragen mit den Vorgaben umgesetzt: Lob wird gespeichert, ist aber ausgeblendet; der Lektor bildet beim RFC
Sammelvorschläge mit Verweis auf die Ursprünge.


Wunsch des Nutzers (2026-11-09) nach dem ersten RFC am echten Buch (Werk 3, „Der Kreidekreis“, Beiträge 41–49):
„Wie können wir festhalten, was von der Kritik eingeflossen ist und was nicht aus Autorensicht berechtigt ist?“ –
„Befundliste je Abschnitt“ – „Größere Werke verlieren sich. Das muss parallel visuell aufgearbeitet werden.“ Die
Nummer ist nur ein Name; die Runde läuft nach R27 und vor R19. Die drei Punkte „R19 `chat-auftrag`“ (Bitte im Chat wird
Vorschlag; RFC/`@alle`; Lektor prüft fremde Befunde) wandern hierher; „R19 `trennfehler`“ bleibt in R19.

**Befund:** Persona-Antworten sind Fließtext-Beiträge. Was der Autor daraus übernimmt oder verwirft, steht nirgends
als Entscheidung; die Personas melden verworfene Punkte erneut (der Lektor dreimal hintereinander, key 103, Beiträge
35–39). Über viele Kapitel hinweg sieht niemand, wo Kritik offen ist.

**Festlegungen (Vorschlag des Hauptagenten, änderbar):**

- **Befund als Datensatz:** Baruch zerlegt Antworten von Personas (Prüfen, Chat, Faden, MCP `ask_persona`) in einzelne
  Befunde (Aufzählungspunkte bzw. „Kritisch:“-Zeilen; Gliederung „Was trägt / Was fehlt / Was stört“). Je Befund:
  Abschnitt, Persona, Art (Lob, Hinweis, kritisch), Text, zitierte Stelle (falls belegt, Prüfvermerk aus R17), Beitrag,
  Zeit, UUID. Status: **offen**, **übernommen** (mit Fassung), **verworfen** (Begründung des Autors, Pflicht),
  **verschoben** (Ziel: Abschnitt/Kapitel oder Einfall), **erledigt ohne Änderung**. Nur der Mensch setzt den Status;
  Assistenten dürfen „übernommen in Fassung n“ **vorschlagen**, wenn ihr Vorschlag einen Befund umsetzt.
- **Personas sehen Entscheidungen:** Verworfene und erledigte Befunde des Abschnitts gehen gerahmt als Daten in jeden
  Aufruf („vom Autor verworfen: … – nicht erneut melden“); ein neuer Befund, der einem verworfenen gleicht (Ähnlichkeit
  über Text/Einbettung), wird nicht als neu gezählt, sondern dort angehängt.
- **Protokoll:** Jede Statusänderung ein Commit im Spiegel (`befunde.md` je Werk), Teil des Austauschformats
  (Unterversion 4, abwärtsverträglich), MCP `findings` (lesen) und `finding_propose_status` (Vorschlag).
- **Übersicht übers Werk (visuell, „größere Werke verlieren sich“):**
  - Seite **Werk ▾ → Befunde** (`/werke/<id>/befunde`): Matrix Kapitel × Persona mit Zählern je Status, farbig **und**
    mit Zeichen/Zahl (nicht nur Farbe), Teile als Gruppen, Klick öffnet die Befunde der Zelle; Filter Status, Persona,
    Art, Zeitraum; Liste „offen, älter als …“.
  - **Gliederung** und Editorkopf: Abzeichen je Punkt „k offen“ (wie bei Vorschlägen), aufsummiert für Kapitel/Teile.
  - **Gedankenkarte:** Ebene „Befunde“ zuschaltbar (Kapitel mit vielen offenen Befunden hervorgehoben).
  - **Verlauf:** je Kapitel, wie viele Befunde eingeflossen/verworfen sind und in welchen Fassungen (aus dem Status
    „übernommen in Fassung n“); im Fassungsbaum an der Fassung „setzt Befund x, y um“.
  - Ohne JavaScript vollständig (Tabelle, Links, Formulare); mit JavaScript Filter ohne Neuladen und Tastatur.
- **Aufträge im Chat (aus R19 hierher):** Bitte um Text im Chat → Vorschlag statt Befund; `@alle`/„RFC“ ruft alle
  aktiven Personas, Lektor zuletzt mit Sammlung und Gewichtung, ohne fremde Befunde ungeprüft zu übernehmen;
  Chatverlauf als Daten im Aufruf. Die RFC-Antworten erzeugen sofort Befunde.

**Lektor als Begleiter statt Prüfer (Nutzer 2026-11-09: „Der Lektor kritisiert nur. Das ist entmutigend für den
Autor.“):** Ursache im Code: Seit Runde 15 müssen Lektor, Exeget und Nathan jede Antwort mit mindestens einem
„Kritisch:“-Befund versehen (`assist/prompts.rs`, `needs_critical`, Nachforderung `CRITICAL_RETRY_NOTE`), auch im Chat
– darum antwortet der Lektor selbst auf „So falle ich mit der Tür ins Haus“ mit Kritik. Künftig:

- Die Pflicht zu „Kritisch:“ gilt nur noch für `@nathan` (Auditor). Lektor und Exeget dürfen kritische Befunde nennen,
  müssen aber nicht; die Belegpflicht (wörtliches Zitat, Prüfvermerk aus R17) bleibt für jeden kritischen Befund.
- Vorgabe-Prompt des Lektors: zuerst, was trägt (konkret, mit Zitat), dann höchstens die zwei wichtigsten Punkte als
  „Vorschlag:“ mit einem konkreten nächsten Schritt, kollegial und ermutigend; im Chat zuerst auf die Nachricht des
  Autors eingehen (Gesprächspartner, nicht Prüfbericht); verworfene Befunde nicht wiederholen (R28a). Beim RFC sammelt
  er die Befunde der anderen und gewichtet sie in „jetzt wichtig“ und „kann warten“.
- Gegenprobe im Test mit MockEliezer: Eine Lektor-Antwort ohne „Kritisch:“ wird nicht nachgefordert und bekommt keinen
  Vermerk; eine Nathan-Antwort ohne „Kritisch:“ schon.
- Ein eigener Prompt des Nutzers (Einstellungen → Assistenten) ersetzt die Vorgabe wie bisher.
- Aufgabe: gehört zu R28c `chat-auftrag` (Dateien `assist/*`).

**Chat bedienbar machen (Nutzer 2026-11-09: „Texte lassen sich weder kopieren noch editieren im Chat. Es fehlt das
Scrollen in der Chat-Historie.“):**

- **Kopieren:** Beitragstexte lassen sich markieren und kopieren (prüfen, ob ein Klick-/Zieh-Handler oder
  `user-select` das verhindert); dazu je Beitrag „Kopieren“ (Text in die Zwischenablage, ohne JavaScript: Text in
  einem markierbaren Feld der Beitragsseite) und „Zitieren“ (als `> …` in das Eingabefeld).
- **Bearbeiten:** eigene Beiträge (Autor bzw. der eigene MCP-Client) lassen sich ändern; die alte Fassung bleibt
  erhalten und ist einsehbar („bearbeitet“, Verlauf), jede Änderung ein Commit wie bisher bei Beiträgen. Beiträge von
  Personas bleiben unverändert (nur kopieren, zitieren, in Einfall oder Befund umwandeln).
- **Scrollen:** Die Chat-Historie bekommt eine feste Höhe im Akkordeon und rollt (heute `.acc[data-section="chat"]
  .acc-body { overflow: hidden }` in `static/app.css` bei `.chat { overflow: auto }` ohne begrenzte Höhe); beim Öffnen
  steht sie am neuesten Beitrag, „Neue Beiträge ↓“ springt dorthin, ältere lassen sich nachladen. Ohne JavaScript:
  die Seite rollt, Anker auf den neuesten Beitrag.
- Gilt ebenso für Fäden ohne Abschnitt (`/faden`). Aufgabe: R28c `chat-auftrag` (Dateien Chat-Fragment, `chat.ts`,
  `posts`, CSS).

**Aufgaben:**

- **R28a `befunde`:** Migration (Befund, Status-Verlauf; UUID/Grabstein wie 0041ff.), Zerlegen der Antworten (bestehende
  Beiträge nachträglich einmal zerlegen, ohne Beiträge zu ändern), Status-RPC, Spiegel `befunde.md`,
  Austauschformat u4, MCP; Personas bekommen Entscheidungen als Daten; Ähnlichkeitsabgleich.
- **R28b `befund-uebersicht`:** Seite `/werke/<id>/befunde` (Matrix, Filter, Listen), Abzeichen in Gliederung und
  Editorkopf, Akkordeon „Befunde“ am Abschnitt mit Status-Formularen, Ebene in der Gedankenkarte, Verlauf am
  Fassungsbaum; Widget für Filter/Tastatur.
- **R28c `chat-auftrag`:** die drei Punkte oben; Tests mit MockEliezer (Schreibauftrag → Vorschlag, RFC → alle,
  verworfene Befunde werden nicht erneut gemeldet bzw. angehängt).

**Abnahme:** erfundenes Werk mit 3 Teilen, 12 Kapiteln, 6 Personas (MockEliezer): RFC in einem Kapitel → Befunde aller
Personas; Autor setzt übernommen/verworfen/verschoben (mit Begründung) → nächste Prüfung meldet die verworfenen nicht
erneut; Matrix und Abzeichen zeigen die Zahlen; Fassung „setzt Befund x um“; Rundreise der Befunde zwischen zwei
Instanzen; alles ohne JavaScript bedienbar.

**Nutzerfragen (nicht blockierend, Vorgabe in Klammern):** (1) Sollen auch Lob-Befunde („Was trägt“) in die Liste?
(ja, aber standardmäßig ausgeblendet); (2) Darf der Lektor offene Befunde anderer Personas zusammenfassen und als
eigene Sammelbefunde führen? (ja, als Vorschlag mit Verweis auf die Ursprünge).

### R30 – Suchen/Ersetzen, Querverweise, Chat: Lösungsvorschläge, Varianten, Bedienung (190 min) – vorgezogen – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (Schnittstelle `aa6808c` mit Migration 0047, Zweige `4a209fc` suchen-verweise,
`d3a2a8f` chat-loesungen, `6d92305` chat-bedienung, Zusammenführung `29a7efa` `37240d1` `62947a8` ohne Konflikte,
Integrationscommit `0043b88` mit Abnahme `tests/round30_abnahme.rs`, Nachbesserung `88df145`); Reste unter „Nach
Runde 30 offen“. Abweichungen vom Wortlaut unten: Alt+→ blättert weiter zur nächsten Fassung (Vorwärts-Sprung nur per
Knopf, Nutzerfrage); „Markierung im Editor“ auf `/suche` nur mit `start`/`end` in der Adresse, im Editor über „⌶“
(Alt+L); der Austausch u5 trägt zusätzlich das Attribut `schluessel` an `<punkt>` (`docs/Austauschformat.md` §6).

Gesammelte Wünsche des Nutzers vom 2026-11-09 (während R28 lief), Reihenfolge danach: R30 → R29 → R19. Aufteilung:
**a `suchen-verweise`** (Suchen/Ersetzen mit VS-Code-Tasten und einstellbarem Bereich, Querverweise und
Sprung-Infrastruktur), **b `chat-loesungen`** (Lösungsvorschläge als Ziel, „Als Vorschlag übernehmen“, Lektor setzt
Lösungen in Varianten um, gleiche Prüfgrundlage, keine gegenseitigen Kommentare außer Lektor), **c `chat-bedienung`**
(↻ Wiederholen, ✕ Löschen, gegliederte Beiträge, Anrede mit Namen, Antworten auf Anrede durch Personas mit Relay,
Live-Anzeige, Sprungmarken der Personas). Wortlaut der Wünsche und Festlegungen (Arbeitsnotizen des Hauptagenten):

Nachtrag nach R28 (Nutzer 2026-11-09, während R28 lief): „Für die Wiederholung nach Korrektur fehlt das Repeat und Delete als Kreuz.“
- Je Persona-Beitrag im Abschnitts-Chat (und in Fäden) ein Knopf ↻ „Wiederholen“: stellt dieselbe Anfrage (Persona, Auftrag/Frage, Umfang) erneut an die jetzt gezeigte/aktuelle Fassung; die alte Antwort bleibt, die neue verweist auf sie („Wiederholung von Beitrag n nach Fassung m“); Befunde der alten Antwort, die nicht mehr kommen, werden als „erledigt?“ vorgeschlagen (R28-Befundliste).
- Je Beitrag ein ✕ „Löschen“ direkt sichtbar (statt nur unter „Verwalten“), mit Rückfrage; Regeln wie bisher (Mensch löscht jeden, MCP nur eigene), Commit im Spiegel.
- Ohne JavaScript: beides als kleine Formulare; Tastatur/aria-label („Antwort von @lektor wiederholen“, „Beitrag löschen“).

NEUE RUNDE R30 „Suchen und Ersetzen, Chat-Nachtrag“ (Nutzer 2026-11-09: „Ich vermisse schmerzlich ein Suchen/Ersetzen-Werkzeug“). Läuft direkt nach R28, vor R29; nimmt den Chat-Nachtrag (↻ Wiederholen, ✕ Löschen) als Aufgabe c mit.
- a `suchen`: Im Editor Strg+F (Abschnitt) und Strg+Umschalt+F (ganzes Werk); Optionen: Groß/klein, ganzes Wort, regulärer Ausdruck (mit Grenzen), in Fußnoten/Bibelzitaten/Typst-Code ja/nein, nur Kapitel/Teil; Trefferliste über alle aktuellen Fassungen mit Kontext, Sprung an die Stelle im Satzbild und im Quelltext (markiert); ohne JS als Seite mit Trefferliste.
- b `ersetzen`: Einzeln (nächster, ersetzen, überspringen) und „alle“ mit Vorschau als Diff je Abschnitt (abwählbar); Ersetzen legt je betroffenem Abschnitt eine neue Fassung vom Menschen an (ein Commit „Ersetzt „x“ → „y“ in n Abschnitten“), Rückgängig = auf die vorige Fassung umschalten (ein Klick für alle); Überschriften ändern die Gliederung wie beim Speichern; Bibelzitate standardmäßig ausgenommen (Wortlautprüfung danach). Nur der Mensch ersetzt; MCP nur suchen (lesen) und höchstens Vorschläge je Abschnitt.
- c `chat-nachtrag`: ↻ Wiederholen und ✕ Löschen (siehe oben).
- Tests: Typst-Sonderzeichen, Unicode/Akzente, regulärer Ausdruck ohne Katastrophe (Zeitgrenze), Ersetzen in 50 Abschnitten mit einer Fassung je Abschnitt und Rückgängig.
- Zusatz Nutzer: „die Ranges sind einstellbar“ → Bereich frei wählbar: Markierung im Editor, Abschnitt, Kapitel samt Unterabschnitten, Teil, „von hier bis zum Ende“, beliebige ausgewählte Gliederungspunkte (Häkchen im Baum), ganzes Werk, alle Werke (dort nur suchen, ersetzen werkweise); dazu Fassung (aktuelle | gezeigte) und Textart (Fließtext, Überschriften, Fußnoten, Zitate/Bibelzitate, Typst-Code) einzeln an/aus. Der gewählte Bereich steht in der Adresse und bleibt beim Blättern erhalten.
- Zusatz Nutzer: Tastenbelegung wie in VS Code: Strg+F Suchen, Strg+H Ersetzen (im Abschnitt), Strg+Umschalt+F Suchen im Werk, Strg+Umschalt+H Ersetzen im Werk; Enter/F3 nächster, Umschalt+Enter/Umschalt+F3 voriger, Strg+Umschalt+1 ersetzen (aktueller Treffer), Strg+Alt+Enter alle ersetzen; Alt+C Groß/klein, Alt+W ganzes Wort, Alt+R regulärer Ausdruck; Escape schließt. Kleine Suchleiste oben rechts im Editor wie bei VS Code (Pfeil zum Aufklappen der Ersetzen-Zeile), Werk-Suche als Seitenleiste mit Trefferbaum.

Zusatz R30c (Nutzer 2026-11-09: „Die Personas sollten Sprungmarken durch href ausgeben können. So wird die Navigation intuitiver.“):
- Baruch macht Bezüge in Persona-Antworten (Chat, Prüfen, Fäden, Befunde) zu Links – im Code, nicht vom Modell erfunden: (1) wörtliche Zitate, die die Belegprüfung (R17) im Werk findet → Link auf Abschnitt und Stelle (Anker, Zitat beim Öffnen im Satzbild/Quelltext markiert); (2) Nennungen von Gliederungspunkten („Kapitel III“, „„Die getragene Scham““, „Einleitung“, „Vorwort“) → Link auf den Punkt (Abgleich mit Nummer/Titel der Gliederung, eindeutig oder gar nicht); (3) Bibelstellen → Link in die Bibelansicht/Literatur mit dem Modul des Werks; (4) Einfälle/Befunde mit Nummer („Einfall 55“, „Befund 12“) → Link.
- Personas bekommen im Prompt die Gliederung mit festen Schlüsseln und dürfen Sprungmarken in der Form `[[key:105]]` bzw. `[[key:105|„Zitat“]]` setzen; Baruch prüft jede Marke (Schlüssel im Werk? Zitat dort?) und rendert nur geprüfte als Link, ungeprüfte als Text mit Vermerk. Kein rohes HTML/`href` aus dem Modell (XSS) – Links baut nur Baruch.
- Ohne JavaScript normale Links; mit JavaScript öffnet ein Klick den Abschnitt im Editor ohne Seitenwechsel und markiert die Stelle; „zurück zum Chat“.

Zusatz R30a (Nutzer 2026-11-09: „Wir haben keine Möglichkeit derzeit, Querverweise anzuspringen.“ – Anlass: Satz „Das ist die Gestalt, von der die Einleitung sprach …“):
- Jeder Gliederungspunkt hat eine feste Sprungmarke (Typst-Label aus dem festen Schlüssel, z. B. `<k103>`, unsichtbar im Satzbild, im Export stabil); eigene Labels der Werkvorlage/des Texts bleiben.
- Querverweis einfügen: Menüband „Verweis“ → Auswahl eines Gliederungspunkts (Suche im Baum) → setzt `#link(<k103>)[die Einleitung]` bzw. `@k103` (mit Seitenzahl im PDF: „Einleitung, S. 7“ nach Einstellung); im Satzbild als Link-Block bearbeitbar.
- Anspringen: Im Satzbild und Quelltext öffnet Strg+Klick (bzw. Klick auf den Verweis-Block) den Zielabschnitt, „Zurück“ (Alt+←) kehrt an die Stelle zurück (Verlauf wie in VS Code); in Vorschau/PDF/EPUB sind Verweise interne Links.
- Erkennen ohne Verweis: Baruch findet Nennungen wie „die Einleitung“, „im dritten Kapitel“, „Kapitel III“, „„Die getragene Scham““ im Fließtext und bietet sie in einer Liste „mögliche Querverweise“ zum Umwandeln an (nur Vorschlag, Mensch bestätigt; neue Fassung). Kaputte Verweise (Ziel gelöscht/umbenannt) zeigt die Druckprüfung.
- Dieselbe Sprung-Infrastruktur nutzen Suche (Treffer) und Persona-Sprungmarken (R30c).

Zusatz R30c (Nutzer 2026-11-09, Bildschirmfoto eines @exeget-Beitrags mit „- …“-Zeilen: „Die Gliederungspunkte müssen besser voneinander abgesetzt sein.“):
- Beiträge im Chat/Faden werden als sicheres Markdown-Teilset gesetzt (serverseitig, ohne rohes HTML): Absätze, Aufzählungen „- “/„* “/„1. “ als echte Listen mit Abstand zwischen den Punkten, **fett**, *kursiv*, Zwischenüberschriften („Was trägt:“ / „Was fehlt:“ / „Was stört:“ bzw. „**…**“ als kleine Überschrift), Zitate „> …“.
- Jeder Befund (aus R28) erscheint als eigener Block mit Abstand, links eine Markierung nach Art (trägt / Hinweis / kritisch – Zeichen und Wort, nicht nur Farbe) und rechts sein Status (offen, übernommen, verworfen …) mit den Knöpfen; „Kritisch:“ als Etikett statt als Textanfang.
- Der Schluss-Hinweis („Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft“) klein und abgesetzt als Fußzeile des Beitrags.
- Gilt ebenso für die Befund-Akkordeons und die Matrix-Detailansicht; Kopieren liefert den Klartext mit „- “-Zeilen.

Zusatz R30c (Nutzer 2026-11-09 „Die Personas reagieren nicht“; Befund key 126, Beiträge 59–61): Der Lektor prüfte die aktuelle Fassung 1 (nur Überschrift) und meldete „Abschnitt leer“, @exeget/@schrift prüften offenbar den gezeigten Vorschlag (Fassung 2) und widersprachen ihm – ein Streit über verschiedene Texte. Außerdem kommentierten @exeget und @schrift den Befund des Lektors statt den Text.
- Jede Persona-Antwort nennt sichtbar ihre Grundlage („geprüft: Fassung 2 (Vorschlag von …)“ bzw. „aktuelle Fassung 1“) – im Kopf des Beitrags, aus dem Aufruf, nicht vom Modell.
- Ist am Abschnitt ein offener Vorschlag und die aktuelle Fassung nur eine Überschrift (oder der Mensch zeigt den Vorschlag), prüfen alle Personas desselben Aufrufs/RFC dieselbe Fassung: Vorgabe die gezeigte; ohne JS Auswahl im Formular.
- Nur der Lektor (beim RFC zuletzt) bezieht sich auf Befunde anderer; die übrigen Personas bekommen fremde Befunde nicht als Prüfgegenstand (nur als Daten „schon gemeldet“, um Doppelungen zu vermeiden) und kommentieren sie nicht.
- „Abschnitt ist leer“ wird nicht als Befund ausgegeben, wenn ein Vorschlag mit Text vorliegt; stattdessen Hinweis „aktuelle Fassung enthält nur die Überschrift – Vorschlag n prüfen?“.

Zusatz R30c (Nutzer 2026-11-09: „Sie antworten nicht automatisch, wenn sie angesprochen werden.“). Befund aus der Datenbank: Alle Erwähnungen des Autors mit „@handle“ wurden in 1–4 s beantwortet (Beiträge 53/54, 55/56, 57/58). Nicht beantwortet werden (a) Anreden ohne „@“ („Lektor, was meinst du?“), (b) Anreden einer Persona durch eine andere im Abschnitts-Chat („@lektor: Der Befund ist falsch“, Beitrag 61), (c) möglicherweise erscheinen Antworten erst nach Neuladen.
- (a) Anrede mit Namen am Satz- bzw. Beitragsanfang („Lektor, …“, „Exeget:“) zählt wie „@lektor“; im Fließtext nicht (sonst löst „der Lektor meint“ aus). Vervollständigung zeigt beides.
- (b) Spricht eine Persona eine andere direkt an (Anrede am Zeilenanfang „@lektor:“), antwortet die angesprochene einmal – Relay-Stufe wie bei MCP (Runde 15): höchstens eine Stufe, höchstens drei Antworten je Ausgangsnachricht, jede Persona höchstens einmal, keine Schleifen; sichtbar „antwortet auf Beitrag n von @…“. Unaufgefordertes Kommentieren fremder Befunde bleibt aus (siehe oben); direkte Anrede ist ein Auftrag.
- (c) Neue Beiträge erscheinen ohne Neuladen (Abfrage oder Ereignisstrom), mit Anzeige „@lektor schreibt …“ und Fehlermeldung, wenn ein Modellaufruf scheitert; ohne JS nach dem Absenden 303 auf den Chat mit der Antwort.

Zusatz R30c, hohe Priorität (Nutzer 2026-11-09: „Was aus dem Chat nicht hervorgeht, sind Lösungsvorschläge. Das sollte das Ziel sein.“):
- Ziel jeder Persona-Antwort ist eine Lösung, nicht der Befund. Regel für alle Personas (Vorgabe-Prompts, im Code geprüft wie heute „Kritisch:“): Jeder kritische Punkt kommt mit einem konkreten Lösungsvorschlag – Stelle (wörtliches Zitat), „statt: …“ / „besser: …“ als Formulierung, oder ein klarer nächster Schritt, wenn eine Formulierung nicht passt (z. B. „Absatz streichen“, „nach Kap. XI verschieben“). Ein kritischer Punkt ohne Lösung wird einmal nachgefordert, sonst mit Vermerk „(ohne Lösungsvorschlag)“ gekennzeichnet.
- Jede „statt/besser“-Formulierung, deren Zitat im gezeigten Text steht, bekommt im Chat den Knopf „Als Vorschlag übernehmen“: Baruch setzt die Ersetzung in die gezeigte Fassung ein und legt einen Vorschlag (neue Fassung, Herkunft Persona, Verweis auf Beitrag/Befund) an; mehrere Lösungen eines Beitrags auf einmal wählbar. Übernehmen der Fassung bleibt Mensch. Befund wird dabei auf „übernommen in Fassung n“ vorgeschlagen (R28).
- Der Lektor schließt beim RFC mit „Mein Vorschlag“: die zwei, drei Lösungen, die er jetzt umsetzen würde, als ein Sammel-Vorschlag (eine Fassung).
- Befundliste (R28) zeigt je Befund seine Lösung(en); die Matrix zählt „mit Lösung / ohne Lösung“.
- Zusatz (Nutzer: „Wir sollten den Lektor befähigen, die Lösungen als Vorschläge (mehrere) umzusetzen.“): Der Lektor kann aus Befunden und Lösungen selbst Vorschläge schreiben – auf Knopfdruck („Lösungen umsetzen“) am RFC bzw. an der Befundliste oder per Chat-Auftrag. Er legt **mehrere Varianten** an (Vorgabe 2, höchstens 3), jede ein eigener Vorschlag (Geschwister im Fassungsbaum, gleiche Basis) mit Etikett „Variante A/B/C“ und einer Zeile, was sie anders macht (z. B. „A: knapp, nur die zwei wichtigsten Lösungen“ – „B: alle Lösungen, Absatz umgestellt“). Jede Variante nennt die Befunde, die sie umsetzt. Vergleich der Varianten nebeneinander (Unterschied zur Basis und untereinander), Übernehmen einer Variante schließt die anderen nicht automatisch, verwirft sie aber auf Wunsch mit einem Klick. Vorschlag bleibt Vorschlag – Übernehmen nur Mensch. Lange Abschnitte: Varianten nur für die betroffenen Absätze (Teilvorschlag), damit der Kontext reicht. Bezug R22a „Alternativen“ (Fassungsbaum) – dort die allgemeine Vergleichsansicht, hier der Lektor-Auftrag.


### R29 – Buchsatz in den Einstellungen (160 min) – vorgezogen vor R19 – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (`09f4f9d` … `066ab17`, PM-Abschluss); Reste unter „Nach Runde 29 offen“. Abweichungen vom
Wortlaut unten: „Seitenzahlen (ab welcher Seite)“ kennt nur „ab der ersten Seite“ und „ab dem ersten Kapitel“ (keine
frei wählbare Startseite; Titelei und Teil-/Leerseiten tragen keine Zahl); Abgleich der Satz-Einstellungen überschreibt
nie (Abweichung als Hinweis, `docs/Austauschformat.md`); EPUB nur als Datenmodell (`epub_style`, erledigt R20a); die
Baruch-Kapitelregel ist wörtlich die übernehmbare Vorlagenregel und bricht im Export auch vor Teilen um.

„Buchsatz in den Einstellungen“ (Nutzer 2026-11-09: „Solche Entscheidungen gehören in die Settings. Das Werkzeug ist mandatory.“ – Anlass: Kapitelumbruch musste per Show-Regel in die Werkvorlage). Läuft nach R28 + Nachtrag, vor R19.
- Einstellungen → Werk → „Satz“ (ohne JS bedienbar, Vorschau): Kapitel beginnt (fortlaufend | neue Seite | neue rechte Seite), Teil-Seite (rechts | beliebig, Rückseite leer ja/nein), Seitenformat (A5, 12×19 cm, 13,5×21,5 cm, A4, eigenes), Ränder (innen/außen/oben/unten, Bundsteg), Grundschrift, Schriftgröße, Zeilenabstand, Absatz (Einzug | Abstand), Blocksatz und Silbentrennung (Sprache), Seitenzahlen (Lage, ab welcher Seite), Kolumnentitel (aus | Werktitel | Kapitel), Fußnotenstil, Schusterjungen/Hurenkinder vermeiden.
- Baruch erzeugt daraus Typst-Regeln hinter der Werkvorlage (wie beim Gliederungsschema, Kommentar „// Baruch: Satz“), gleiche Regeln für Satz-Vorschau, Buch-PDF, Umschlag (Rückenbreite aus Seitenzahl!) und – abgebildet – EPUB (Kapitel als Dateien, CSS). Werkvorlage bleibt für Fortgeschrittene und gewinnt bei Widerspruch nicht still: Hinweis „Die Werkvorlage setzt X selbst“.
- Migration (Spalten/Tabelle je Werk, UUID wie üblich), Austauschformat-Unterversion, RPC/MCP lesen (Ändern nur Mensch), Spiegel.
- Werk 3: Die Show-Regel aus Vorlagenfassung 2 (vom Hauptagenten am 2026-11-09 gesetzt) wird erkannt und als Einstellung „neue rechte Seite“ übernommen; Angebot, die Vorlagenzeile zu entfernen (neue Vorlagenfassung, Mensch bestätigt).
- Tests: jede Einstellung wirkt im PDF (Seitenzahl, Lage gemessen), ohne Einstellungen byte-gleich wie bisher.

Aufteilung: **a `satz-daten`** (Migration, Store, RPC/MCP, Austauschformat, Spiegel), **b `satz-typst`** (Regeln hinter der Werkvorlage, PDF/Vorschau/Umschlag, Erkennen der Vorlagenregel aus Werk 3, Tests mit Messung), **c `satz-oberflaeche`** (Einstellungen → Werk → Satz mit Vorschau, ohne JS, Hinweise bei Widerspruch zur Werkvorlage).

### R19 – RAG-Pflege, Reste der Stellen (120 min) – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (`800da6a` … `ec25ac9`, PM-Abschluss); Reste unter „Nach Runde 19 offen“. Abweichungen vom
Wortlaut unten: Gruppen benennen und `near_limit`/`near_threshold` über MCP kamen in R19b statt R19a; ältere Fassungen
und Befunde im Index (Nutzerfrage 1) sowie Kommentare/Befunde nach MultiGPT (Nutzerfrage 11) bleiben wie vereinbart
offen; MCP `idea_search` bekam zusätzlich `mode: "rag"` (nur lesend, PM-Vorgabe unter „Nutzerfragen“); der TLS-Hinweis
gilt nach dem Gutachten auch für `require` (sqlx prüft dort das Zertifikat nicht), empfohlen ist nur `verify-full`.

**Zusatz 2026-11-08 (Fund am echten Buch, Nutzer: „GO“):**

- (erledigt R28c, nach R28 vorgezogen; Rest „neuer Abschnitt per Schreibauftrag“ unter „Nach Runde 28 offen“) **R19 `chat-auftrag`: Bitte im Chat wird Vorschlag.** Bittet der Mensch eine Persona im Abschnitts-Chat um Text
  („@lektor Erstelle ein Vorwort und lösche die persönlichen Abschnitte“), antwortet sie heute nur mit „Kritisch:“-
  Befunden und wiederholt dieselben Befunde bei jeder Nachfrage (Werk 3, Abschnitt key 103, Beiträge 33–39). Künftig:
  Erkennt Baruch einen Schreibauftrag (oder wählt der Mensch „als Auftrag“), schreibt die Persona einen **Vorschlag**
  (neue Fassung des Abschnitts; neuer Abschnitt wie „Vorwort“ nur als Antrag/Anlegen nach den Regeln von
  `outline_add`) und einen kurzen Beitrag dazu; Löschen bleibt Mensch. Der Verlauf des Chats geht als Daten in den
  Aufruf, damit die Persona auf Antworten („So falle ich mit der Tür ins Haus“) eingeht statt Befunde zu wiederholen;
  bereits gemeldete Befunde nicht erneut melden (Abgleich mit den eigenen letzten Beiträgen).
- (erledigt R28c, nach R28 vorgezogen) **R19 `chat-auftrag` (Zusatz): „RFC“ bzw. `@alle` ruft alle aktiven Personas.** Der Autor schrieb „RFC für den
  Abschnitt“ in den Chat (Werk 3, key 102, Beitrag 41) und erwartete, dass sich alle melden; ohne Erwähnung antwortete
  niemand. Künftig: `@alle` oder „RFC“ am Anfang ruft jede aktive Persona einmal, der Lektor zuletzt mit Sammlung und
  Gewichtung; die Grenze „höchstens drei Antworten je Nachricht“ gilt dafür nicht, die Warteschlange (eine Anfrage zur
  Zeit) schon. Der Lektor darf Befunde anderer nicht ungeprüft übernehmen (am 2026-11-09 machte er aus einer
  Fehldeutung von @belege einen „Kritisch“-Befund).
- (erledigt R19c: „Wortlaut stimmt (Leerraum im Modul)“, `module_note`) **R19 `trennfehler`: Wortlautprüfung gegen Module mit Silbentrennresten.** Das selbst aus einem PDF gebaute Prüfmodul
  enthält Trennreste („die ses“, „Men schen“, „gerechtfer tigt“); @belege meldet dann bei korrektem Buchtext „weicht
  ab“ (Werk 3, key 103, Beitrag 32). Abhilfe: (a) beim Vergleich Trennreste tolerieren (Zitat ohne Leerraum gegen Modul
  ohne Leerraum vergleichen, wenn nur Leerraum abweicht → „stimmt (Leerraum im Modul)“), (b) Hinweis in der Prüfanzeige
  und bei @belege, wenn ein Modul auffällig viele Wortfragmente hat. Das Modul selbst bleibt beim Nutzer (nicht im Repo,
  nicht in Tests; Tests mit erfundenem Modul mit eingebauten Trennresten).

- **R19a `index`:** Index-Worker ohne neuen Versuchszyklus je Suche (`Job::Prune`), `rag.status` ehrlich,
  `rag.reindex` ohne lange Sperre, `Index::replace` auf den Bereich beschränkt, Einfälle nach `work.delete` sofort;
  ältere Fassungen und Befunde im Index erst nach der Nutzerfrage 1; Gedankenkarte: Gruppen durch einen Assistenten
  benennen (Stub, Nutzerfrage 2 zu R25), `near_limit`/`near_threshold` über MCP. – „Nach Runde 12 offen“, „Nach Runde 25 offen“.
- **R19b `pgvector`:** alle Felder in SQL gekürzt, Schema-Namen sicher gequotet, Hinweis bei Passwort mit `tls:
  prefer`/`disable`; Aufgaben-Präfixe der Einbettung je Weg einstellbar. – „Nach Runde 12 offen“.
- **R19a/c zusätzlich aus Runde 13:** Abgleich-Härtung und Unicode-Normalform, siehe „Nach Runde 13 offen“.
- **R19c `treffer`:** Untergrenze bzw. „schwach“ bei geringer Ähnlichkeit (Literatur, Einfälle, Belege), Fehlercode
  von `idea.search` im Modus `rag`. – „Nach Runde 12 offen“. Dazu die fünf Reste der Stellen aus Runde 17
  (Zitat-Bindeglied, Lektor-Hinweis zur Wortlautprüfung, Rückabbildung der Bibelseite, Lemma-Probe, „Mal“ als
  Zahlwort) und ein Menüband-Knopf „Bibelstelle“ bzw. `@cite`, falls die Vorlage sie hergibt (PM R18: R19 war mit
  100 min die kürzeste Runde, die Stellen sind ebenfalls Trefferqualität). – „Nach Runde 17 offen“, „Nach Runde 18
  offen“.

### R20 – EPUB-Vorschau (180 min) – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (`8da1f7a` … `b1a8788`, Nachbesserung und PM-Abschluss); Reste unter „Nach Runde 20
offen“. Abweichungen vom Wortlaut unten (PM bestätigt): Der Druckerei-Umschlag mit den Textregeln der Werkvorlage kam
in R20c statt R20a (§0.6); die Dicke je Blatt (`paper_sheet_mm`) wird ohne Migration aus der gespeicherten Dicke je
Seite gerechnet (§0.7); das EPUB-Cover ist die rohe Umschlag-Bilddatei ohne Titel aus der Umschlag-Anordnung (erledigt R21c:
Rasterbild der Vorderseite mit Titel); der Integrator hat nach der Sichtprüfung Dateien von reader (`from_own_frame`, E-Ink-CSS)
und Tests von epub-pruefung geändert (belegter Referer-Fehler im Rahmen, Folgen des Zusammenführens) – bestätigt.
Kindle: Annäherung über Reader-CSS und den Hinweis „Send to Kindle nimmt EPUB an“ (Nutzerfrage 6, PM-Vorgabe).


- **Zusatz 2026-11-09:** Nutzer 2026-11-09: „als EPUB wird es ignoriert“ (Seitenumbruch vor Kapiteln). Festlegung: EPUB setzt jedes Kapitel (Ebene 1) und jede Teil-Seite als eigene XHTML-Datei im Spine (natürlicher Umbruch in allen Readern), zusätzlich CSS `break-before: page` / `page-break-before: always` für Reader, die eine Datei fortlaufend zeigen; Show-Regeln der Werkvorlage mit `pagebreak` werden erkannt und auf diese Aufteilung abgebildet (nicht stillschweigend ignoriert); EPUB-Vorschau zeigt die Kapitelgrenzen.

- **R20a `epub`:** EPUB aus den aktuellen Fassungen und der Werkvorlage, Umschlag als Cover (STUB `cover.rs`; dabei
  den Umschlag für die Druckerei mit den Textregeln der Werkvorlage setzen, siehe „Nach Runde 27 offen“),
  Fußnoten, Inhaltsverzeichnis, Download/RPC/CLI. – „EPUB-Vorschau“, „Werke (Rest)“, „Umschlag und Bilder über MCP“.
- **R20b `reader`:** Reader-Profile mit eigenem CSS (`static/readers/*.css`), Vorschau nebeneinander, Menü
  „EPUB-Vorschau“ (STUB `base.html`); Pflicht-Reader nach „EPUB-Vorschau“ (vorinstallierte Android-Reader vorher
  prüfen), Kindle nach Nutzerfrage 6. – „EPUB-Vorschau“.
- **R20c `epub-pruefung`:** EPUBCheck und Barrierefreiheit in der Vorschau, Warnungen je Reader; Buchangaben-Reste
  (ISBN-Präfix und Schreibweisen, Rückenbreite je Blatt, Umschlagbild löschen mit Spiegel-Commit, ein Commit je
  bestätigtem Umschlag-Antrag). – „EPUB-Vorschau“, „Nach Runde 11 offen“.

### R31 – Register und Glossar (170 min) – vorgezogen vor R21 – erledigt 2026-11-09

Wunsch des Nutzers (2026-11-09): „Baruch braucht noch die Indices und Glossare. Im Text das Kontextmenü ‚in Glossar aufnehmen‘.“

**Festlegungen (Vorschlag des Hauptagenten, änderbar):**

- **Glossar je Werk:** Begriffe mit Erklärung (Typst-freier Text, kurze Form und ausführliche Form), Schreibvarianten/Beugungen („Gnade, Gnaden“), Quelle, Bezug auf Bibelstellen; Seite **Werk ▾ → Glossar** (Liste, anlegen, bearbeiten, löschen mit Spiegel-Commit; ohne JS bedienbar).
- **Kontextmenü im Text** (Satzbild und Quelltext, Rechtsklick bzw. Menütaste/Umschalt+F10 und Menüband): markiertes Wort → **„In Glossar aufnehmen“** (Formular mit Begriff vorbelegt, Erklärung, Varianten), **„In Register aufnehmen“** (Stichwort, Unterstichwort, Registerart), **„Im Glossar nachschlagen“**. Ohne JS: gleiche Aktionen über ein Formular an der Markierung bzw. über die Glossar-Seite.
- **Register (Indices):** Stichwortregister, Personenregister, Bibelstellenregister (automatisch aus den erkannten Stellen, R17, mit eigener Sortierung nach biblischer Buchreihenfolge), auf Wunsch weitere Register. Einträge als unsichtbare Marke im Typst-Text (eigene Funktion `#idx[…]` bzw. Metadaten mit Label, ohne fremde Typst-Pakete, kein Netz); Baruch erzeugt die Registerseiten am Buchende mit Seitenzahlen (Typst `query`/`locate`), zusammengefasste Seitenbereiche („12–14“), Unterstichwörter, „siehe auch“. Satzbild zeigt Marken dezent, Menüband „Register“ zeigt alle Einträge eines Abschnitts.
- **Glossar im Satz:** Glossarseite am Buchende (Einstellung: an/aus, Lage vor/nach den Registern), erste Erwähnung eines Begriffs je Kapitel wahlweise markiert (Einstellung unter Satz, R29); im EPUB (R20) als Glossar-Datei mit Links bzw. Popup-Fußnote (epub:type="glossterm"/"glossdef").
- **Vorschläge der Assistenten:** Personas dürfen Glossarbegriffe und Registereinträge **vorschlagen** (Liste „vorgeschlagen“, Mensch übernimmt), MCP `glossary_list`, `glossary_propose`, `index_propose`; Lektor: „Begriffe, die erklärt werden sollten“.
- **Daten:** Migration (Glossar, Registereinträge, Varianten; UUID/Grabstein wie üblich), Austauschformat-Unterversion, Spiegel `glossar.md`, Typst-Export enthält die Marken und die Registererzeugung (bleibt mit `typst compile` setzbar).
- **Aufgaben:** a `glossar-daten` (Migration, Store, RPC/MCP, Austausch, Spiegel, Vorschläge), b `register-satz` (Marken, Registerseiten mit Seitenzahlen, Bibelstellenregister, Glossarseite, EPUB-Abbildung, Satz-Einstellungen), c `kontextmenue` (Kontextmenü im Satzbild/Quelltext, Formulare, Glossar-Seite, Register-Übersicht, Tastatur/aria).
- **Abnahme:** erfundenes Werk: Markierung → „In Glossar aufnehmen“ → Begriff erscheint in der Glossarseite im PDF und im EPUB; Registereinträge mit Seitenbereichen und Unterstichwörtern gemessen im PDF; Bibelstellenregister in biblischer Reihenfolge; Typst-Export setzt mit `typst compile` dieselben Register; Rundreise zwischen zwei Instanzen; alles ohne JS bedienbar.
- **Stand Integration (Integrator, 2026-11-09):** a → b → c zusammengeführt, Abnahme `tests/round31_abnahme.rs` grün;
  Import eines Exports mit Buchende nachgezogen. Reste für den PM-Abschluss („Nach Runde 31 offen“): Bearbeiten einer
  Marke ohne JavaScript nur als Entfernen/neu Setzen; Stichwörter verschiedener Groß-/Kleinschreibung werden nicht
  zusammengeführt; Typst-Meldungen aus dem Buchende zeigen auf den letzten Abschnitt (`ExportMap::section_at_line`);
  `typst compile` nur mit dem eingebetteten Compiler geprüft (CLI fehlt); Registervorschläge nicht im Austauschformat;
  MCP `index_propose` meldet `node` statt `key` als isError, die übrigen schreibenden Werkzeuge als -32602.
- **Nachbesserung nach Gutachten (2026-11-09):** keine doppelten Registermarken (`mark_word`, `markInSource`),
  „Begriffe vorschlagen lassen“ als Formular auf Glossarseite und „An der Markierung“, S1-Zweige der R31-Tests
  entfernt, Handprüfung mit Werk 3 in `HANDOFF.md`; auch das Satzbild (`runIndexMark`, `sameMarkBehind`) lehnt
  dieselbe Marke ein zweites Mal ab (nur im Browser prüfbar, kein Node-Test). Offen: kein MCP-Werkzeug für
  `glossary.suggest`.
- **PM-Abschluss (2026-11-09):** Gutachten mit 11 Befunden (4 mittel, 7 niedrig); die vier mittleren hat die
  Nachbesserung `877d485` behoben, der PM zwei niedrige (Rustdoc von `back_matter`/`strip_back_matter` richtig
  zugeordnet; Abschnitt „Integration“ in `docs/MCP.md` gefüllt) und eine falsche Zusage in `docs/Austauschformat.md`
  (Grenzen beim Einlesen) berichtigt; übrige Reste unter „Nach Runde 31 offen“. PM-Entscheidungen: Stichwörter bleiben
  nur bei wörtlicher Gleichheit gleich (Groß-/Kleinschreibung ist Absicht des Autors, z. B. „Wort“/„wort“ selten
  gleich gemeint); kein MCP-Werkzeug für `glossary.suggest` (ein MCP-Client schlägt mit `glossary_propose` selbst vor);
  Registervorschläge bleiben außerhalb des Austauschformats (sie hängen an einer Fassung der Quellinstanz).

### R21 – Import und Bilder (B3) (180 min) – erledigt 2026-11-09

Alle drei Aufgaben umgesetzt (`842b0ed` … `fc6b3f6`, PM-Abschluss); Reste unter „Nach Runde 21 offen“. Gutachten
17 Befunde (1 hoch, 5 mittel, 11 niedrig); die Nachbesserung `fc6b3f6` behob den hohen und fünf mittlere
(Backlog/Fahrplan machte der PM-Abschluss), der PM sechs niedrige (CLI „1 Seiten“/„1 Abschnitte“, Fehlerausgabe von
Poppler wird ganz geleert, kein vorgewähltes Bild bei „Pfade mit zwei Bildern“, Token `--warning` statt des fehlenden
`--accent`, Exit 2 in Bedienung, drei Begründungen in `round21_pdf_import.rs`) und die Doku des MCP-Kontingents.
Abweichungen der Entwickler (zusätzliche Felder der Gliederungspunkte, `--bibliothek` allein, Entwurf bleibt nach
MultiGPT-Fehler, „Ohne Nummern“ über die Rückfall-Show-Regel, EPUB-Satz in Bedienung, `format:` beim Satz) bestätigt.

- **R21a `pdf-import`:** PDF/EPUB über MultiGPT `upload_document`/`read_document` (lokal als Rückfall), Gliederung mit
  Rückfrage, Quellen in die Bibliothek. – Plan B3, „Werke (Rest)“.
- **R21b `markdown-import`:** Import von Markdown, Hochladen mit Bildern, `export --out` prüft Zwischenordner (Symlink)
  und Datei/Ordner-Kollisionen vorab (`import.rs`); Buchende beim Import nur schneiden, wenn erzeugt, und melden;
  erste Erwähnung ohne Glossarseite. – „Werke (Rest)“, „Bilder“, „Nach Runde 6/31 offen“.
- **R21c `bilder`:** Bild in der Literatur, Typwechsel beim Ersetzen, Altbestand `/x.png`/`x.png`, Maskierungs- und
  SVG-Tests, Tests der Kopfzeilen der Antragsdatei, Kontingent für MCP `asset_*` und Aufräumen entschiedener Anträge,
  `ImageView` als eigenes Feld; Meldung von `pdfinfo` beim PDF der Karte prüfen; **EPUB-Cover als Rasterbild der
  Umschlag-Vorderseite mit Titel, Untertitel und Autor** (Gutachten R20). – „Bilder“, „Nach Runde 11/8/6/25/20 offen“.

### R22 – Revisionen und Spiegel (160 min) – erledigt 2026-11-10

Umgesetzt in drei Aufgaben (fassungen, befund-rueckfrage, spiegel-entwurf; Vertrag 2026-11-10 mit PM-Ausgleich: Archiv an
befund-rueckfrage, „Spiegel setzen“ an spiegel-entwurf). Reste: „Nach Runde 22 offen“.


- **Zusatz 2026-11-09 (Nutzer): Entwurfsmodus als Wasserzeichen.** „Ich brauche einen DRAFT-Modus als Wasserzeichen.“
  Festlegung (Vorschlag): Satz-Einstellung (Einstellungen → Werk → Satz, R29) **„Entwurf“** an/aus mit Text (Vorgabe
  „ENTWURF“), optional Datum und Stand (z. B. „Stand 9.11.2026 · Fassungen von heute“), Deckkraft (hell, unauffällig),
  Lage (diagonal über die Seite | Kopf-/Fußzeile). Wirkt auf Satz-Vorschau, Buch-PDF, Stichwortzettel und auf Wunsch den
  Umschlag (Vorgabe: Umschlag ohne); das EPUB bekommt bei „Entwurf“ einen Hinweis auf der Titelseite und
  „Entwurf“ im Titel-Metadatum. Schalter auch direkt beim Download („Als Entwurf herunterladen“), ohne die Einstellung
  zu ändern; die Druckprüfung warnt, wenn ein PDF für die Druckerei (Umschlag-PDF) mit Wasserzeichen entsteht.
  Typst-Regel hinter der Werkvorlage (`#set page(background: …)` bzw. `foreground`), Text nur als Zeichenkette;
  ohne Einstellung byte-gleich. Teil des Austauschformats wie die übrigen Satz-Einstellungen.


- **Zusatz 2026-11-09 (Nutzer, am echten Buch): Kleine Änderungen ohne neue Fassung.** „Jede Speicherung wird zu einer
  neuen Revision, aber bei kleinen Änderungen lohnt sich das nicht. Gerade die Kritiken der Personas sind als Autor
  ungeeignet. Revidieren die Personas, legen sie eine neue Version an.“ Festlegung (Vorschlag): Speichert der Mensch auf
  seiner **eigenen** aktuellen Fassung, wird diese **fortgeschrieben** statt eine neue Fassung anzulegen (jede
  Fortschreibung bleibt als Commit im Git-Spiegel nachvollziehbar und im Verlauf der Fassung sichtbar; Rückgängig über
  den Verlauf). Eine **neue Fassung** entsteht, wenn (a) der Mensch ausdrücklich „Als neue Fassung speichern“ wählt,
  (b) die aktuelle Fassung von einer Persona/einem MCP-Client stammt oder ein übernommener Vorschlag ist, (c) die
  Änderung groß ist (Schwelle einstellbar, z. B. mehr als 20 % des Textes oder eine Überschrift/Ebene ändert sich),
  oder (d) seit der letzten Fortschreibung eine Zeit vergangen ist (einstellbar, z. B. 30 min, „Sitzung“). Kleine
  Korrekturen des Autors nach einer Persona-Kritik schreiben also seine Fassung fort; Personas legen immer eine neue
  Fassung (Vorschlag) an. Einstellungen → Werk → „Fassungen“. Befunde (R28), die durch eine Fortschreibung umgesetzt
  wurden, verweisen auf Fassung n samt Stand der Fortschreibung. Regel AGENTS.md bleibt: genau eine aktuelle Fassung;
  Ändern nur Mensch, protokolliert im Spiegel.
- **Zusatz 2026-11-09: Personas fragen, ob eine Kritik erledigt ist.** „Personas fragen, ob eine Kritik erledigt ist,
  wenn sie der Meinung sind, allem genüge getan zu haben.“ Beim erneuten Prüfen (↻ Wiederholen, RFC, Prüfen) gleicht
  jede Persona ihre eigenen offenen Befunde des Abschnitts mit dem neuen Text ab; sieht sie einen Befund umgesetzt,
  schreibt sie keinen neuen Befund, sondern eine kurze Rückfrage „Befund n scheint erledigt (Stelle …) – als erledigt
  markieren?“ mit Knöpfen **Ja, erledigt** / **Nein, noch offen** (setzt der Mensch; Status-Vorschlag wie R28). Sieht
  sie alle ihre Befunde erledigt, sagt sie das ausdrücklich statt neue Kritik zu suchen.


- **R22a `alternativen`:** Hinweis „weitere Fassung“ am Abschnitt im Lesefluss, Umschalten per Klick. –
  „Revisionsbaum (Rest)“.
- **R22b `seitenzweige`:** übrige Fassungen, offene Vorschläge und Alternativen als `refs/revisions/<kapitel>/<n>` im
  Spiegel (`baruch-gitmirror`); dazu aus R21b: Titel mit Code an einem gleichlautenden nummerierten Knoten bekommt
  eine Show-Regel über die Sprungmarke `<kN>` (ändert die Exportform, fremde Tests nachziehen). – „Revisionsbaum
  (Rest)“, „Nach Runde 21 offen“.
- **R22c `archiv`:** archiviertes Werk aus der Oberfläche wiederherstellen; Spiegel der Einfälle ohne Werk beim Start und
  nach `work.delete`. – „Werke (Rest)“, „Nach Runde 8 offen“.

### R23 – Härtung (200 min) – **erledigt** 2026-11-24 (mit Gutachten, Nachbesserung und PM-Abschluss)

Ergebnis je Punkt: Berichte der Zweige (siehe „Nach Runde 23 offen“); Reste dort mit der Marke für Runde 24 (seit der Nachbesserung R24: „offen nach R24“) bzw. „→ Nutzer“.

- **R23a `web`:** ~~Schutz gegen Einbetten (`frame-ancestors`)~~ (erledigt Nachbesserung R15: `guard::no_framing`), feste Meldecodes statt `?ziel=…`,
  `baruch:settings-changed` nur nach Erfolg, Abzeichen und „Anträge zur Gliederung“ ohne JavaScript, Test für
  `settings.ts`; Obergrenze bzw. `spawn_blocking` für die Nähe der Gedankenkarte; Umschlag-Reste aus R27 (Zeitgrenze
  für `book_page_mm`/`block_heights`, Pixelgrenze für die Kontrastprüfung, Normalform beim Einlesen von Blöcken,
  `StoreError::CoverBlock`, Vorgabeblöcke auf 0,01 mm, Namensschilder auf dem Rücken); Reste aus R30 (Relay-Wege
  zusammenlegen, ↻ mit Relay-Rahmen und Herkunft, Teilvorschlag byte-gleich, Treffer-Markierung nach Offsets,
  Browser-Verhaltenstest der neuen Widgets). – „Nach Runde 9 offen“, „Nach Runde 25 offen“, „Nach Runde 27 offen“,
  „Nach Runde 28 offen“, „Nach Runde 30 offen“. Reste aus R19 (Gedankenkarte: Übernehmen eines veralteten
  Vorschlags, Stellenparser „Joh 3,16 Mal 3“, Leerraum-Vergleich nur bei auffälligem Modul bzw. mit Wortgrenzen,
  `module_note` über `spawn_blocking`, Test der hängenden Einbettung) – „Nach Runde 19 offen“. Reste aus R20
  (verschachtelte Fußnoten in der Reihenfolge, Linktext bei `@label` auf Abbildungen, Temp-Ordner für EPUBCheck mit
  0700 und Zufallsnamen, EPUB-Vorschau ohne Neuladen bei Pfeiltasten in Auswahllisten, Inhalt eines komprimierten
  mimetype prüfen) – „Nach Runde 20 offen“. Reste aus R31 (Themen-Tokens der Registermarke, Hell/Dunkel im Bild,
  `section_at_line` fürs Buchende, Test für `sameMarkBehind`) – „Nach Runde 31 offen“. Reste aus R21
  (`with_image_formats` nur im Code statt in Kommentaren/Raw, `format:` bei berechneten Pfaden, Export nach Typwechsel,
  Test für `markdownTitle`, Kasten „Bilder des Werks“ bei `?lq=`, geschachtelte Begriffslisten, `p.at(i)±k`,
  Spalten der EPUB-Vorschau, Sichtprüfung in beiden Themen) – „Nach Runde 21 offen“.
- **R23b `mcp-namen`:** Handles eindeutig (Kollisionen, widerrufene Tokens, 32 Zeichen), Umlaute umschreiben, Zuordnung
  ohne Token, Titelregel U+2028 über MCP mit Test; Obergrenze für `index_propose`, `node` statt `key` einheitlich; MCP-Kontingent für Bilder
  nicht über neue Namen umgehbar. – „Nach Runde 10/8/31/21 offen“.
- **R23c `geheimnisse`:** Test „Prüfung folgt keiner Weiterleitung“, `scrub` auch für kodierte Geheimnisse (Base64,
  URL-Kodierung); Grenzen beim Einlesen von Glossar und Registern (u7). – „Nach Runde 9/31 offen“.

### R24 – Verteilung (B5) (200 min) – **erledigt** 2026-11-24 (mit Gutachten, Nachbesserung und PM-Abschluss)

Ergebnis je Aufgabe: „Nach Runde 24 offen“. Die Aufgaben hießen in der Runde `pakete`, `ci` und `webseite` (statt
`betrieb`; mit Hugo-Seite und MultiGPT-Vorgabe).

- **R24a `pakete`:** .deb/AppImage, .dmg, .msi mit AGPL-Hinweisen, Aktualisierung (Signierung nach Nutzerfrage 10). –
  Plan B5. *Ergebnis:* .deb und AppImage gebaut und gestartet, .dmg/.msi nur als CI-Aufträge (nie gelaufen); keine
  Aktualisierungsprüfung, Über-Seite.
- **R24b `ci`:** CI-Arbeitsablauf (heute liegt keiner im Repo, kein `.github/`), `protoc` für `lance`,
  Plattform-Runner, Lizenzprüfung der Abhängigkeiten, `typst compile` des Exports mit installiertem `typst`. – „Nach
  Runde 12/9/8/31 offen“. *Ergebnis:* drei Arbeitsabläufe liegen im Repo und sind lokal nur über Regelprüfung und
  Selbsttest geprüft; **kein Arbeitsablauf ist je gelaufen**, auch `typst compile` mit der Kommandozeile nicht.
- **R24c `betrieb`:** Betrieb hinter Reverse-Proxy/TLS (erlaubte Rechnernamen, Doku), Git-Historie vor einer
  Veröffentlichung prüfen. – „Nach Runde 10 offen“, „Kleinigkeiten“. *Ergebnis (als Aufgabe `webseite`):*
  `--allow-host`, `docs/Betrieb.md`, Hugo-Seite; die Historie hat das Gutachten geprüft, die Entscheidung liegt beim
  Nutzer („Schritte des Nutzers“).

### Beim Nutzer (Handprüfung, nicht eingeplant)

Zusammengefasst aus „Nach Runde 6/8/9/11 offen“ und „Gliederungsmanager (Rest)“: Satzbild, Baum, Menüs, Popup und
Literatur mit Maus und Tasten im eigenen Browser; Vorschaubilder (`loading="lazy"`) in Bilderliste und Werkübersicht;
echter Schlüsselbund (`baruch/eliezer` erscheint als „hinterlegt“); leerer Abschnitt (`== `) im Buch umbenennen oder
löschen.

- (→ Nutzer) Gedankenkarte nach Runde 25 (vorher Sicherung, Migration 0043): Werk 3 mit seinen 33 Einfällen in der
  Karte ordnen (die 5 ohne Abschnitt in der Wolke „Ohne Kapitel“, Teile mit Zahl, leere Kapitel „leer“), Lagen
  ziehen und zurücksetzen, „Bezug setzen“ ohne JavaScript, Widerspruch 48 ↔ 79 von Hand anlegen, Nähe mit und ohne
  Bedeutungsindex, Ausgabe nach FreeMind (Freeplane) sowie SVG/PDF/Markdown, Umzug eines verbundenen Einfalls in ein
  anderes Werk (Verbindungen gelöst). Ablauf in `HANDOFF.md` (Runde 25, „Handprüfung durch den Nutzer“).
- (→ Nutzer) Editor mit Menüband nach Runde 18 (vorher Sicherung, Migration 0044): Werk 3 – Knöpfe Szene/Arbeit nach
  der Werkvorlage, Formate im Quelltext und im Satzbild, Insel per Doppelklick, Einstellungen → Werk → Satzbild samt
  „Alles aus der Vorlage“, Bild an der Cursorstelle, „Satz abbrechen“, Kanten der Gedankenkarte. Ablauf in
  `HANDOFF.md` (Runde 18, „Handprüfung durch den Nutzer mit Werk 3“).

### Nutzerfragen (nicht eingeplant)

1. **RAG-Inhalt:** welche älteren Fassungen in den Index (alle oder nur benannte), und Befunde/Beiträge der Personas
   auch? (danach in R19a)
2. **Bevorzugte Quellen:** Ort und Format der vorhandenen Lesezeichenliste (Import in R13b; bis dahin von Hand).
3. **Abgleich (R13c, gebaut):** MultiGPT-Schlüssel mit `docs.write` anlegen und das Hochladen der Kapitel in die
   Sammlung „Baruch: <Werk>“ je Werk einschalten (Einstellungen → Werk, mit Bestätigung; ohne Schlüssel nur die
   Richtung MultiGPT → Baruch). Dazu die Annahmen über das echte MultiGPT in `docs/Architektur.md` prüfen.
4. **WYSIWYG:** echter Typst-Satz im Satzbild (Seitenumbruch, Silbentrennung) bzw. direkt im gesetzten Text tippen –
   oder beim Paar Quelltext/Satzbild bleiben?
5. **Baum über das ganze Dokument:** Revision als Stand des Werks über mehrere Abschnitte, mit Zusammenführungen –
   noch gewünscht?
6. **EPUB:** Kindle Previewer gibt es nur für Windows/macOS – genügt für Kindle eine Annäherung über Reader-CSS?
   EPUBCheck braucht Java – installieren? (PM-Vorgabe R20: Kindle als Annäherung „über EPUB“ samt Send-to-Kindle-Hinweis;
   EPUBCheck wird nur gesucht – Java bzw. `epubcheck` installiert der Nutzer selbst, dann Einstellungen → Daten →
   „EPUBCheck“. Offen: Java installieren? Hersteller-Reader auf Android (Samsung, Onyx …) – welche, es gibt kein
   erfundenes Profil dafür.)
13. **Rückenbreite:** Vorgabe 0,2 mm je Blatt (0,1 mm je Seite) beibehalten oder die Dicke des Papiers der Druckerei
    eintragen? (R20c, Einstellungen → Werk → Buchangaben)
7. **Bilder über Eliezer:** Hochskalieren/Neuerzeugen (`moss-image-sft`) gewünscht, und hat Eliezer dafür einen
   Endpunkt?
8. ~~**Teile:** vorhandene Teile-Texte („= Teil …“, leere Teile in Werk 3) in die Teil-Markierung überführen?~~
   **Entschieden (Nutzer 2026-11-07): „Automatisch umwandeln“** – umgesetzt im Nachgang zu Runde 14 als
   einmaliger Startschritt (`baruch-store/src/legacy_parts.rs`, `docs/Bedienung.md` „Ältere Teile umwandeln“).
9. **`#heading(...)` vor der ersten `=`:** künftig als Kapitel? (Kleinigkeiten, HANDOFF)
10. **Verteilung:** Signierung für macOS/Windows, Zielplattformen, Zeitpunkt (Plan „Offen (Nutzer)“); Git-Remote des
    Spiegels (Ziel) – heute gibt es keinen.
11. **Abgleich: Kommentare/Befunde nach MultiGPT** (Plan „RAG-Abgleich“): welche Beiträge dürfen hinaus (nur Befunde
    der Personas oder auch Chat-Beiträge des Menschen), und mit eigenem Schalter? (nach der Entscheidung eine eigene Aufgabe; in R19 nicht umgesetzt)
12. ~~**Neuer Teil über MCP übernimmt vorhandene Kapitel** (Gutachten Runde 14): `outline_add` mit `level: 0` vor einem
    Kapitel mitten in einem Teil hängt die folgenden Kapitel bis zum nächsten Teil sofort unter den neuen Teil. Gilt
    das als „Anlegen“ – oder soll es als Antrag laufen?~~
    **Entschieden (Nutzer 2026-11-07): „es braucht eine aktive Zustimmung (Popup)“** – umgesetzt am 2026-11-08:
    Übernähme ein neuer Teil vorhandene Kapitel, wird `outline_add` ein Antrag (Art `add`, Migration 0035,
    `Store::outline_add_by_assistant`); der Mensch stimmt im modalen Dialog (Widget `consent`, auf jeder Seite) bzw.
    ohne JavaScript über Hinweisleiste und Formular auf `/gliederung#antraege` zu oder lehnt ab; beides mit Herkunft
    im Spiegel. Ein Teil ohne Folgekapitel bleibt Anlegen (`docs/MCP.md`, `docs/Bedienung.md` „Zustimmung zu einem
    neuen Teil“, AGENTS.md „Ausnahme Gliederung“).
14. **Befund-Blöcke kompakter** (aus Runde 30, in Runde 23 nicht geändert): Jeder Listenpunkt einer Persona-Antwort
    wird ein eigener Befund-Block mit Statusknöpfen; bei langen Listen wird der Chat lang. Zusammenfassen (z. B.
    zugeklappt je Antwort) oder so lassen?
15. **`persona` über MCP ohne Token** (Runde 23): Ein Programm auf dem eigenen Rechner darf ohne Token mit dem Argument
    `persona` im Namen einer Persona schreiben (mit Token ist es verboten). Auch das sperren? Eine Zeile in
    `mcp/tools.rs::acting_persona`; bricht die Tests, die `persona` über Loopback nutzen (`docs/MCP.md`, „Bewusste
    Entscheidung“).
16. **Befunde beim Abgleich „vorspulen“** (Runde 23): Status und Verlauf eines schon bekannten Befunds aus der Datei
    übernehmen, wenn der Verlauf hier ein Präfix des Verlaufs der Datei ist (wie bei Fassungen)? Heute gilt „bleibt,
    mit Hinweis auf die Zahl der Einträge“.
17. **Veröffentlichung** (Wunsch 2026-11-24) – in Runde 24 nach den Vorgaben des Vertrags umgesetzt (Einspruch
    jederzeit): ~~Zielversion~~ 0.9.0, ~~Adresse des Repos~~ `github.com/Beerlesklopfer/baruch`, ~~Vorgabe der
    MultiGPT-Verbindung~~ leer, CI/Pakete/Reverse-Proxy gebaut. **Offen bleiben drei Entscheidungen** (Einzelheiten
    unter „Nach Runde 24 offen“ → „Schritte des Nutzers“): (a) **Historie** – so veröffentlichen, umschreiben oder
    frisches Repository; (b) **Archivschlüssel** – eigener für Baruch (empfohlen) oder der gemeinsame mit ForgeIEC;
    (c) **Impressum** der Webseite füllen oder bewusst so lassen. Nutzerfrage 10 (Signierung macOS/Windows) ist
    unberührt.
18. **`/mcp` hinter einem Reverse-Proxy** (Gutachten R24, niedrig, Entwurfsfrage): Schreibt der Proxy `Host` auf
    `127.0.0.1:<Port>` um (bei nginx die Vorgabe ohne `proxy_set_header Host`), hält Baruch die Anfrage für lokal und
    nimmt `/mcp` ohne Token an. Heute dokumentiert und mit Pflicht-Probe (`docs/Betrieb.md`). Geschlossen ausfallen
    hieße: Sobald `--allow-host` gesetzt ist, verlangt `/mcp` **immer** ein Token – dann brauchen auch Programme auf
    dem Rechner selbst eines. Der eigene Dienst ohne `--allow-host` wäre nicht betroffen. So lassen oder umstellen?
    (`guard::via_proxy`, `mcp.rs`; wenige Zeilen und Tests in `round24_betrieb.rs`.)

Vom PM gesetzte Vorgaben (Einspruch jederzeit): MCP `idea_search` darf mit `mode: "rag"` auch die
Bedeutungssuche nutzen (nur lesend wie `rag_search`, Ausfall als `isError`; Runde 19c, nicht im Backlog bestellt); @belege sucht beim Vorschlag nicht im Web (keine Quellenzeile im
Buchtext; Runde 13); Daten der Websuche und Anzeigezeiten in Ortszeit (Nutzer 2026-11-07; gespeichert und in RPC weiter UTC); Ausschalten des Hochladens lässt Dokumente bei
MultiGPT stehen, erst das Löschen des Werks räumt sie ab; Sprechtempo der Predigt 120 Wörter je Minute (je Werk änderbar);
Stichwortzettel als A6-Karten aus Überschriften und den ersten Sätzen je Abschnitt; Persona-Hervorhebung „übernehmen“
legt eine eigene Kopie an, das Original bleibt Vorschlag.

### Überholt (mit Beleg)

- MultiGPT-Prüfung nur erste Seite von `tools/list` (Runde 9): `connections/check.rs:178` folgt `nextCursor` (Runde 12).
- Lektor und Einfälle als Gedächtnis (Runde 8, STUB B2): `assist/context.rs:150` („Gedächtnis des Lektors“), Auftrag
  an den Lektor in `templates/idea.html:94` (Runde 10); offen bleibt nur die Wahl einer anderen Persona (erledigt R15c: Formular „Einfall verarbeiten“ mit Persona-Auswahl).
- Lektor-Wahl bei gleicher Kontextlänge (Versuch gemeinsames Modell): Regel „bei Gleichstand die erste“
  (`baruch-core/src/lib.rs:193`, Test `automatic_lector_takes_the_first_on_a_tie`), dazu fest @lektor.
- Echte Bibeldatenbank (Modelle der Personas): SWORD-Bibeln (`baruch-sword`, Migration 0030).
- Bilder bei der Erzeugung als virtuelle Dateien (Bilder: Erzeugung): `baruch-render/src/lib.rs` (Bilder als virtuelle
  Dateien, Vorschau seit Runde 8, PDF seit Runde 11); damit auch „Base64 in Typst prüfen“ gegenstandslos.
- Suche nach Name und Bildunterschrift: `asset.search` (`baruch-server/src/assets.rs:176`) sucht in Name,
  Bildunterschrift und Alternativtext (`baruch-store/src/assets.rs:838`, Runde 11).
- Export als PDF (Werke (Rest)): `GET /werke/{id}/pdf`, `src/pdf.rs` (Runde 11).
- Werkzeugleiste als eigenes Fragment (Menüband): `templates/fragments/edit_toolbar.html` gibt es.
- Teilweise: `/x.png` und `x.png` – das Anlegen lehnt eine Kollision in Normalform ab (`baruch-store/src/assets.rs:623`),
  Altbestand erledigt R21c (`asset.path_resolve`); MCP-Beiträge „eigene“ – mit Token heißt der Client wie der Token
  (`mcp/session.rs:7`, Runde 10), offen nur ohne Token (R23: `lokal-` gegen Token-Namen erledigt; `persona` ohne Token → Nutzerfrage 15).

## Bibeln in der Literatur und SWORD als Recherchewerkzeug (Wünsche 2026-11-07)

Wortlaut der Wünsche ist nicht überliefert; Inhalt nach Plan und Umsetzung in Runde 15 (R15b, R15c):
- **Bibeln in der Literatur:** Die SWORD-Bibeln des Werks erscheinen im Literatur-Popup und unter `/literatur/bibel`
  (Stelle aufschlagen, Treffergruppen „Bibel“ und „Kommentar“, Zitat als Vorschlag übernehmen). – erledigt R15c.
- **SWORD als Recherchewerkzeug:** Kommentare und Lexika aus SWORD-Modulen, Auswahl je Werk (Einstellungen → Werk,
  Migration 0037), RPC `commentary.*`/`lexicon.*`, MCP `commentary_lookup`/`lexicon_*`; Faktenblock aus Bibeltext,
  Kommentaren und Lexika für @belege und den Lektor (nummerierte Belege, Quellen im Beitrag). – erledigt R15a/R15b.
- Reste stehen unter „Nach Runde 15 offen“ (→ R17: Lexikon-Stichwörter, Fließtext-Verweise, Widget der Kommentare;
  Hervorheben an Bibeltexten → Nutzerfrage).

## Menüband für das Bearbeiten (Wunsch 2026-11-06, später)

Im Mittelpunkt steht jetzt der
Arbeitsbereich mit seinen drei Teilen (Gliederung, Editor, Assistenten). Später kommt ein Menüband (Ribbon) –
**nur für das Bearbeitungswerkzeug**, nicht für das App-Menü: Es ersetzt die Werkzeugleiste des Editors
(Satzbild/Quelltext, Absatzart, K, F, …) und wird auch im Preprint (Satz-Ansicht, `/werke/{id}/satz`)
verwendet. Die Werkzeugleiste darum schon jetzt als eigenes Fragment halten, das Editor und Preprint teilen
können. Das App-Menü der Kopfzeile (Runde 9) bleibt davon unberührt. (erledigt R18a; das eigene Fragment gibt es schon: `templates/fragments/edit_toolbar.html`)

## Umschlag und Bilder über MCP (Wunsch 2026-11-06)

**Umgesetzt in Runde 11** (außer Umschlag im EPUB, Stub, und Hochskalieren/Neuerzeugen über Eliezer). (Umschlag im EPUB erledigt R20a als Bilddatei, mit Titel erledigt R21c; Hochskalieren/Neuerzeugen → Nutzerfrage 7)

- **MCP `asset_add`** (heute nur `list_assets`/`read_asset`): `{work, file_name, content (base64|svg), name, path?,
  caption?, alt?, role?}` wie RPC `asset.add`; Herkunft im Spiegel. Neue Bilder anlegen dürfen Assistenten
  (wie `outline_add`); Ersetzen und Löschen nur als Antrag, den der Mensch bestätigt.
- **Rolle „Umschlag“** je Werk (`role: "cover"`): ein Bild als Umschlag markieren (Einstellungen → Werk und
  Bilderliste); die Werkvorlage setzt es vollflächig als erste Seite, Titel und Untertitel in einen wählbaren
  Bereich (Vorgabe oben; bei „Herr prüfe mich“ der dunkle obere Bereich). Auch im EPUB als Cover.
- **Druckauflösung prüfen:** Typst-Prüfung warnt, wenn ein Bild bei der gesetzten Größe unter 300 dpi liegt
  (Umschlag Werk 3: 832×1216 px reichen nur für etwa 7×10 cm). Hochskalieren oder neu erzeugen über Eliezer
  (`moss-image-sft`, Seed 1598) gehört zur Bilderzeugung (Abschnitt „Bilder: Erzeugung und Oberfläche“).
- Stand: Umschlag von Werk 3 liegt als `umschlag.png` (retuschiert) und `umschlag-original.png` im Werk
  (RPC `asset.add`, 2026-11-06); Dateien auch unter `~/Bilder/baruch/herr-pruefe-mich/`.

## Nach 0.9.0 offen: Ausnahmen in deny.toml (2026-11-24)

Vor dem erneuten Tag v0.9.0 meldete `cargo deny check advisories` in der CI elf Einträge. Gehoben: quick-xml 0.38 →
0.42 (eigene Nutzung in baruch-epub, baruch-exchange, baruch-server; RUSTSEC-2026-0194/-0195), zip 2.4 → 9.8
(RUSTSEC-2026-0367/-0368/-0369/-0429/-0545/-0548), url 2.5.8 → 2.5.9 (RUSTSEC-2026-0592). Als begründete
Ausnahme in `deny.toml` eingetragen, **offen nach 0.9.0**:

- **RUSTSEC-2026-0194, -0195 (quick-xml 0.38.4):** nur noch über typst-library → hayagriva 0.10.1 → citationberg
  0.7.0, das quick-xml ^0.38 verlangt. Erledigt, sobald citationberg/hayagriva/typst auf quick-xml ≥ 0.41 gehen.
- **RUSTSEC-2026-0459 (libsqlite3-sys 0.30.1, SQLite < 3.54):** sqlx 0.8.6 schreibt libsqlite3-sys ^0.30 fest; der
  Fix (≥ 0.39) braucht sqlx 0.9 (Abfragen mit zusammengesetztem SQL auf `AssertSqlSafe`/`SqlSafeStr`, rund 350
  Aufrufe; Rust ≥ 1.94; Postgres-Feature in baruch-server, Migrationen und Makros prüfen). Baruch öffnet nur
  die eigene Datenbank im eigenen Datenordner, darum im Betrieb nicht betroffen.
- **RUSTSEC-2026-0543 (chrono unbetreut):** kommt über arrow/datafusion/lance/lancedb; Ablösung erst, wenn diese
  wechseln (die direkte Nutzung in baruch-server allein abzulösen nähme chrono nicht aus dem Baum).

## Nach Runde 24 offen (PM-Abschluss 2026-11-24)

Aus den Berichten der drei Entwickler (`pakete`, `ci`, `webseite`) und der Integration. Die Berichte mit allen
Belegen sind über die Commits der Zweige lesbar: `git show dfbf20a:docs/runde24/pakete.md`,
`git show ff0db95:docs/runde24/ci.md`, `git show 285b58e:docs/runde24/webseite.md` (der Ordner `docs/runde24/` ist
nach dem Übertragen in die Hauptdokumente gelöscht). Die niedrigen Punkte aus früheren Runden, die auf Runde 24 zeigten (Satzbild, Typst-Abbruch, Kontingente,
Kosmetik …), waren nicht Umfang dieser Runde und bleiben offen – siehe „Zuordnung“ gleich unten.

**Zuordnung (Nachbesserung 2026-11-24, Gutachten R24):** Runde 24 war die letzte geplante Runde, und der Fahrplan
hatte ihr die niedrigen Reste aus Runde 23 zugeschlagen (`bibles::module_note` über `spawn_blocking`,
`with_image_formats`, Satzbild-Reste …). **Umgesetzt wurde davon in Runde 24 nichts** (`git diff --stat
3028194..689d609` berührt weder `crates/baruch-server/src/bibles.rs` noch `baruch-render`). Jeder Punkt, der an
seiner alten Stelle die Marke für Runde 24 trug (66 Stellen), trägt dort jetzt **„offen nach R24“**: nicht
umgesetzt, keiner Runde zugeordnet, Aufnahme nur auf Wunsch des Nutzers. Die sechs Marken mit Aufgabenbuchstaben
(R24b, R24c) tragen ihr Ergebnis: `protoc` in der CI, Lizenzprüfung (zweimal), kein TLS im Server (Doku),
erlaubte Rechnernamen – erledigt; Git-Historie – offen, beim Nutzer (unten). Probe: `grep -n "→ R24"
docs/Backlog.md` trifft nur noch diese Zeile; `grep -c "offen nach R24" docs/Backlog.md` zählt die offenen.
Schwerpunkte der offenen Reste (Fundstellen über die Probe): Satzbild (geschachtelte Begriffslisten, Markierung
von Treffern nach Offsets), Typst (kein harter Abbruch, Vorschau verdrängt den Zwischenspeicher, `cover_checks`
setzt doppelt, Zähl-Abgleich, Tests ohne Satzprobe), Werkvorlagen (`p.at(i)±k`, `slice()`, Kolumnentitel ohne
`document.title`), `with_image_formats` (rein textuell), `bibles::module_note` (blockiert den async-Faden),
MCP-Kontingente und Clients ohne Token, `fetch_allowed` ohne DNS-Auflösung, `scrub` (Hex, Base64), Rechte eines
vorhandenen Datenordners, Kosmetik (Token-Liste, Zitat-Folgezeile, Platzhalter der Zahlenfelder).

### Schritte des Nutzers (vor und bei der Veröffentlichung)

**Reihenfolge** (Nachbesserung 2026-11-24; die Punkte darunter beschreiben die Schritte im Einzelnen):

1. **Historie entscheiden** (Punkt „Historie“ unten) – vor dem ersten Push, danach lässt sich nichts mehr
   zurückholen. Heute sind 420+ Commits von `development` nicht gepusht (`git rev-list --count
   origin/main..development`).
2. **Schlüssel wählen** (eigener Baruch-Schlüssel oder der gemeinsame mit ForgeIEC, `docs/Verteilung.md`),
   Fingerabdruck an den drei Stellen eintragen, Impressum füllen, committen auf `development`.
3. **Erster Push** von `development`, dann **Fast-Forward nach `main`** und Push (löst `ci.yml` und `pages.yml`
   aus; nur der Nutzer).
4. **Repository öffentlich schalten** (Punkt „Sichtbarkeit“ unten) und die **einmalige Einrichtung** bei GitHub
   (Pages-Quelle, Umgebungen `github-pages` und `release`, Secrets in der Umgebung `release`, DNS).
5. **Generalprobe** aller drei Arbeitsabläufe per „Run workflow“; `ci.yml` muss grün sein.
6. **Tag** `v<Version>` setzen und schieben; im Lauf die Freigabe der Umgebung `release` erteilen; den
   **Entwurf** des Release prüfen und veröffentlichen, bevor der nächste Push auf `main` geht.
7. Danach im eigenen Dienst: Sicherung, neues Paket einspielen, MultiGPT-Adresse prüfen (Handprüfung in
   `HANDOFF.md`).

- (→ Nutzer, neu aus dem Gutachten R24) **Sichtbarkeit des Repositorys:** `Beerlesklopfer/baruch` ist bisher
  privat (`docs/Plan.md`: „privat …, später frei“). Webseite, README und Über-Seite verlinken aber auf Releases
  und Quelltext bei GitHub, das Quellcode-Angebot jedes Pakets (`QUELLTEXT.txt`, AGPL) nennt das Repo samt Tag,
  und die Download-Links der Seite zeigen auf Release-Dateien. Solange das Repo privat ist, führen alle diese
  Verweise für Fremde ins Leere, und das Quellcode-Angebot ist nicht erfüllt. Öffentlich schalten (Settings →
  General → „Change repository visibility“) – **nach** der Entscheidung zur Historie und **vor** dem ersten Tag.
  Annahme, ungeprüft: GitHub Pages und Schutzregeln für Umgebungen gibt es für private Repos nur in bezahlten
  Tarifen; mit einem öffentlichen Repo entfällt die Frage.
- (→ Nutzer, neu aus dem Gutachten R24) **Archivschlüssel:** Vorgesehen war der ForgeIEC-Archivschlüssel auch für
  Baruch. Das Gutachten rät zu einem eigenen Schlüssel (ein Einbruch in dieses Repo oder sein Secret träfe sonst
  auch `apt.forgeiec.io`). Die Abläufe sind an keinen Schlüssel gebunden; die Abwägung steht in
  `docs/Verteilung.md`, „Schlüssel für das APT-Repository“. Nachgebessert: Der Job `publish` läuft in der
  Umgebung `release` (Freigabe, nur Tags `v*` – die Regeln richtet der Nutzer ein), `scripts/ci-check-workflows.sh`
  lehnt `APT_GPG_*` außerhalb dieser Umgebung ab, und `scripts/ci-fingerprint.sh` lässt ein signiertes Release
  scheitern, wenn der veröffentlichte Fingerabdruck fehlt oder nicht zum Schlüssel passt.

- (→ Nutzer) **Einmalige Einrichtung** nach `docs/Verteilung.md`: DNS-CNAME `baruch` → `beerlesklopfer.github.io`,
  Pages-Quelle „GitHub Actions“, Regel für Tags `v*` in der Umgebung `github-pages` (Annahme, ungeprüft), Umgebung
  `release` mit Freigabe und Tags `v*`, darin die Secrets `APT_GPG_PRIVATE_KEY` und `APT_GPG_PASSPHRASE` (nicht
  als Repository-Secrets).
- (→ Nutzer) **Fingerabdruck des Archivschlüssels** eintragen: `params.aptFingerprint` in `site/hugo.toml` und der
  Platzhalter `FINGERABDRUCK-TRÄGT-DER-NUTZER-EIN` in `README.md` (einmal) und `docs/Verteilung.md` (zweimal).
  Probe: `scripts/ci-fingerprint.sh` (Exit 3 = noch offen, Exit 0 = alle drei Stellen gleich). Ohne den Eintrag
  scheitert ein signiertes Release im Job `publish` (gewollt), und README wie Webseite zeigen den Platzhalter.
- (→ Nutzer) **Generalprobe** aller drei Arbeitsabläufe per „Run workflow“ vor dem ersten Tag: Kein Arbeitsablauf ist
  je gelaufen. Darin erstmals der Bau für Windows und macOS (.msi, .dmg), `cargo deny check advisories`,
  `scripts/ci-apt-fetch.sh` gegen das echte GitHub, das Signieren mit dem echten Schlüssel, die Auslieferung über
  Pages.
- (→ Nutzer) **Entwurf veröffentlichen** vor dem nächsten Push auf `main`, sonst fällt `/apt/` auf den vorigen
  veröffentlichten Stand zurück (beim ersten Release: fehlt) und die Download-Links der Seite zeigen ins Leere.
- (→ Nutzer) **MultiGPT-Adresse**: Eine Instanz, die die Verbindung MultiGPT nie gespeichert hat und nur über die
  frühere Vorgabe lief, hat nach der Aktualisierung keine Adresse mehr und braucht sie einmal unter Einstellungen →
  Verbindungen. Das kann den eigenen Dienst betreffen (dort nicht nachgesehen).
- (→ Nutzer) **Historie** – Entscheidung vor dem ersten Push; der Push veröffentlicht alle Commits, nicht nur den
  letzten Stand. Im heutigen Stand ist alles Folgende bereinigt (Test
  `round24_abnahme::the_current_tree_names_no_private_machine_and_no_third_party`), **in den älteren Commits steht
  es weiter** (Gutachten R24, die Historie wurde nicht umgeschrieben):
  - die **Autoren-Adresse** der Commits: fast alle tragen eine private Adresse bei einem Freemail-Anbieter, zwei
    die Adresse, die auch die Pakete nennen (`git log --all --format='%an <%ae>' | sort | uniq -c`). Eine
    `.mailmap` ändert nur die Anzeige, nicht die gespeicherte Adresse;
  - die frühere **Intranet-Adresse** der MultiGPT-Vorgabe (zwei Treffer in einer Betriebstabelle der Doku), der
    **Rechnername** des Entwicklungsrechners (Titelzeile jeder früheren Fassung von `HANDOFF.md`) und lokale
    **Scratchpad-Pfade** mit dem Anmeldenamen;
  - **Name und Buchtitel eines Dritten**: Das Beispielwerk hieß bis zur Nachbesserung R24 nach dem echten Buch
    und nannte dessen Autor; `docs/Plan.md`, Backlog, Bedienung und mehrere Tests nannten Titel und Untertitel.
    Dazu der schon bekannte Punkt aus „Kleinigkeiten“: ein kurzer Überschriftsteil aus dem Buch in den drei
    Commits `a231aff`, `e4c5999`, `9f3034c` (alle in `development`, `git merge-base --is-ancestor`);
  - Geheimnisse, Schlüssel oder Tokens fand das Gutachten in keiner Revision (Suche nach `PRIVATE KEY`, `ghp_`,
    `github_pat_`, `sk-`, `AKIA`, `Bearer` und Zuweisungen an `password`/`token`/`api_key`).

  Drei Wege: (a) **Historie so veröffentlichen** – nur, wenn Adresse, Rechnername und die Nennung des Dritten
  (mit dessen Einverständnis) öffentlich sein dürfen; (b) **Historie umschreiben** (`git filter-repo` für Adresse
  und Textstellen) – aufwendig, alle Commit-Nummern in Backlog, Fahrplan und HANDOFF stimmen danach nicht mehr;
  (c) **frisches Repository** mit einem ersten Commit aus dem heutigen Stand (die volle Historie bleibt privat
  beim Nutzer) – am einfachsten, die Commit-Nummern in der Doku verweisen dann auf das private Archiv. Kein Agent
  hat hier etwas umgeschrieben oder gepusht. Bei (b) und (c) vorher die Autoren-Adresse für neue Commits setzen
  (`git config user.email`); auch der Commit dieser Nachbesserung trägt noch die bisherige.
- (→ Nutzer) **Beispielwerk** (Nachbesserung R24): `baruch_core::sample::work()` heißt jetzt „Beispielwerk: Leise
  Abhängigkeit (Auszug)“ von „Erika Mustermann“; das Kapitel 2 heißt „Das heimliche Muster“. Vorher trug es Titel
  und Autor eines echten Buchs und steckte so in jedem Paket. Vorhandene Datenbanken ändert das nicht (gesät wird
  nur in eine leere); im eigenen Dienst behält ein noch vorhandenes Beispielwerk seinen alten Titel, bis der
  Nutzer es umbenennt oder löscht. Thema und Gliederung des Beispiels (Abhängigkeit in Beziehungen, Seelsorge)
  sind geblieben – sie sind frei erfunden formuliert, lehnen sich aber an das Thema des echten Buchs an; wer auch
  das nicht will, ersetzt das Beispiel ganz (viele Tests hängen an seinen Texten). Das Beispielwort im Auftrag
  des Hörers (`assist/prompts.rs`) ist jetzt „Perikope“.
- (→ Nutzer) **Impressum der Webseite** (`site/content/impressum.md`) ist ein sichtbar markierter Platzhalter ohne
  Anschrift (`platzhalter: true`); seit der Nachbesserung warnt jeder Seitenbau davor (`scripts/ci-site.sh`),
  liefert aber aus. Vor der Veröffentlichung füllen oder die Seite bewusst so lassen (Entscheidung des Betreibers;
  ob eine Anbieterkennzeichnung nötig ist, ist eine Rechtsfrage, die kein Agent beantwortet).
- (→ Nutzerfrage 10) **Signierung** für macOS und Windows bleibt abgeschaltet (`MACOS_SIGN`, `WINDOWS_SIGN`).

### pakete

- (offen, erster CI-Lauf) `cargo check -p baruch-server` für Windows und beide Apple-Ziele scheitert lokal an
  `zstd-sys` (kein Kreuzcompiler). .dmg und .msi sind nie gebaut worden (unsicher: `candle`/`light` mit ICE-Prüfungen
  und HKCU-Schlüsselpfad bei `perMachine`, `sips`/`iconutil`/`hdiutil`, der Starter unter macOS, `baruch-start.cmd`).
- (offen) Kein Windows- oder macOS-Zweig ist je gelaufen: Schlüsselbund dort, Programmsuche mit `.exe/.cmd/.bat` und
  Abbruch über `taskkill` in `baruch-epub/src/check/tools.rs` sind nur übersetzt.
- (niedrig) Der Zweig der Über-Seite mit gesetztem `APPIMAGE` hat keinen automatischen Test (nur Handprobe am Abbild).
- (niedrig) AppImage: Start ohne FUSE, andere Distributionen, `desktop-file-validate`/AppStream ungeprüft; `lintian`
  über das .deb lief nicht (nicht vorhanden).
- (Hinweis) Größe: `baruch-server` 340 MB ausgepackt, .deb 75 MB, AppImage 116 MB.
- (niedrig) Unter Windows bleibt der freie Platz vor einer Sicherung unbekannt (`settings/data.rs` liefert `None`).

### ci

- (offen) `actionlint` und `shellcheck` über Abläufe und Skripte (lokal nicht vorhanden; Ersatz PyYAML,
  `scripts/ci-check-workflows.sh`, `sh -n`, Selbsttest).
- (offen) Tests und clippy laufen in einem CI-Job: Plattenplatz und Laufzeit beobachten (Grenze 240 Minuten), bei
  Bedarf trennen. Node, `protoc`, `apt-utils`, `gnupg`, `python3-yaml`, `gh` sind nicht auf Versionen festgelegt.
- (niedrig) `upload-pages-artifact` v4.0.0 und `deploy-pages` v4.0.5 gewählt; es gibt v5, deren Anforderungen nicht
  nachgelesen sind.
- (niedrig) Ein zweiter Lauf für dasselbe Tag scheitert an `gh release create`; übernommene Pakete werden nach
  `SHA256SUMS` desselben Release geprüft, nicht nach ihrer Signatur.
- (Hinweis) Neuer öffentlicher Typalias `baruch_server::pdf_import::typst_out::Assignment` (clippy
  `type_complexity`).

### webseite

- (offen) Die nginx- und Caddy-Beispiele in `docs/Betrieb.md` liefen an keinem echten Proxy; das Zurückspielen einer
  Sicherung ist dort als Annahme markiert.
- (Grenze, Hinweis an den Nutzer) Schreibt der Proxy `Host` auf `127.0.0.1:<Port>` um, kann Baruch die Anfrage nicht
  von einer lokalen unterscheiden und `/mcp` ginge ohne Token; `docs/Betrieb.md` verlangt darum eine Probe nach dem
  Einrichten. `X-Forwarded-*` wertet Baruch nicht aus.
- (Entscheidung, dokumentiert) Sind Namen erlaubt, gilt auch eine Anfrage ohne `Host`-Kopfzeile nicht als lokal;
  `AllowedHosts::parse` lehnt zusätzlich `localhost` und Namen mit Zahl als letztem Label ab.
- (offen) `PGSSLROOTCERT`: siehe „Nach Runde 19 offen“ (nur Quelltext gelesen; Prüfung gegen PostgreSQL mit TLS fehlt).
- (niedrig) `params.version` in `site/hugo.toml` ist eine zweite Stelle für die Versionsnummer (Test und
  `scripts/ci-site.sh` melden eine Abweichung). Hugo vor 0.158 kennt `locale` vermutlich nicht (ungeprüft).
- (niedrig) Die Angaben der Seite Installation zu macOS und Windows stammen aus `docs/Verteilung.md` und sind wie
  dort ungeprüft.

### Aus der Integration

- (erledigt, Integration) Zwei Tests an das Vertragsverhalten angepasst: festgeschriebene Prüfsumme in
  `crates/baruch-rag/tests/port.rs` (das `#[allow]` mit Begründung in `baruch-rag/src/lib.rs`),
  `round12_lancedb.rs` (MultiGPT ohne Adresse ist „aus“ statt „öffnet mit der Vorgabe-Adresse“). Neue Abnahme
  `tests/round24_abnahme.rs` (drei Tests). `cargo fmt --all` über die neuen Dateien der Zweige.
- (erledigt, Integration) Sechs Bildschirmfotos der Webseite (`site/static/bilder/`, erfundenes Werk) mit
  `site/tools/bildschirmfotos.py --fotos`; im dunklen Thema hatten Codeblöcke mit langen Zeilen einen hellen
  Rollbalken-Streifen (`scrollbar-color` im Stil ergänzt); Alt-Text des Bilds „Fassungen“ an das Foto angepasst.
- (offen, CI) `typst compile` des exportierten Werks mit der Typst-Kommandozeile lief auch beim Integrator nicht
  (die Rechteprüfung der Sitzung lehnte den Aufruf ab, nicht umgangen); die geprüfte Kommandozeile 0.15.1 und
  `scripts/ci-typst.sh` liegen bereit, der Job `test` in `ci.yml` zeigt es.
- (Abnahmebefehl berichtigt) `git grep -n "0\.0\.1" -- Cargo.toml README.md packaging scripts .github site` trifft
  auch `127.0.0.1`; gemeint war die alte Version. `git grep -nE '(^|[^0-9.])0\.0\.1' -- …` ist leer. Außerhalb
  des Suchbereichs nennt `CHANGELOG.md` „Stand 0.0.1“ einmal als Geschichte; zwei Beispieldateien des
  Austauschformats tragen `programm="baruch 0.0.1"` (erfundene Altdaten, bewusst unverändert).
- (Hinweis) `grep -rn "STUB:" crates` trifft vier ältere Tests, die gerade zusichern, dass kein Stub-Vermerk mehr
  da ist; unter `crates/*/src` gibt es keinen Vermerk.
- (niedrig) Bildschirmfotos der Webseite zeigen die Seite Assistenten mit den mitgelieferten Personas und Modellnamen
  der Vorgabe; `fassungen.png` zeigt das Akkordeon „Fassungen“ zugeklappt (Umschalter 1/2 sichtbar).
- (niedrig) Sichtprüfung der Webseite nur in Firefox (headless); kein anderer Browser, kein echtes Telefon.
- (erledigt, PM-Abschluss) `docs/Fahrplan.md` für Runde 24 nachgeführt; das Gutachten ist erfolgt.

### Aus dem PM-Abschluss (2026-11-24)

Gutachten R24: 16 Befunde (8 mittel, 8 niedrig). Die acht mittleren hat die Nachbesserung `1943102` bearbeitet (oben);
von den acht niedrigen sind sechs im PM-Abschluss behoben, zwei bleiben (Nutzerfrage 18, Lieferkette).

- (erledigt, PM-Abschluss) Test für `baruch-server --version` und `-V`
  (`round24_betrieb::the_program_prints_its_version_and_starts_nothing`: Exit 0, genau eine Zeile, kein Datenordner).
- (erledigt, PM-Abschluss) Über-Seite: Der Satz zur Aktualisierung nannte APT für alle Pakete; jetzt „von Hand mit
  einem neuen Paket“, APT nur für Debian/Ubuntu (`templates/about.html`, `docs/Bedienung.md`, `docs/Verteilung.md`).
- (erledigt, PM-Abschluss) `ci.yml`, Job `test`: `cargo fmt --all -- --check` als erster Schritt (rustfmt in der
  festen Rust-Version), wie in der lokalen Abnahme.
- (erledigt, PM-Abschluss) `docs/Verteilung.md`, Überblick: Das AppImage ist lokal gebaut und über FUSE gestartet
  (die Tabelle sagte noch „Abbild nur in der CI“).
- (erledigt, PM-Abschluss) Fahrplan, Übersichtstabelle des Rundenplans (R23, R24), Nutzerfrage 17, `docs/Plan.md`
  „Offen (Nutzer)“.
- (→ Nutzerfrage 18) `/mcp` fällt hinter einem Proxy, der `Host` umschreibt, offen aus (dokumentiert, Pflicht-Probe).
- (niedrig, offen) Lieferkette in `release.yml`: `brew install protobuf` und `choco install protoc` ohne feste
  Version, `cargo install` von cargo-deb und cargo-deny ohne Prüfsumme; `security import … -A` im Schlüsselbund des
  macOS-Runners (Signieren ist abgeschaltet). Vor dem Einschalten der Signierung nachziehen.
- (niedrig, offen) `settings.get {page: "mcp"}` liefert fest `loopback_without_token: true`, auch unter einem
  erlaubten Rechnernamen (nur in `docs/RPC.md` erklärt).
- (Hinweis) .deb und AppImage unter `target/` stammen aus der Integration (`689d609`) und sind nach der Umbenennung
  des Beispielwerks und dem geänderten Satz der Über-Seite **nicht neu gebaut**; vor einer Weitergabe neu bauen
  (`scripts/build-deb.sh`, `packaging/appimage/build-appimage.sh`) – die CI baut ohnehin aus dem Tag.
- (Hinweis) Aufgeräumt: Worktrees der Runde samt `target/` entfernt; die Zweige `team/pakete`, `team/ci`,
  `team/webseite` bleiben (alle in `development` enthalten). Die Bilder der Sichtprüfung liegen unter `.claude/sicht/r24/` (nicht im Git).

## Nach Runde 23 offen (Abschluss 2026-11-24)

Aus den Berichten der drei Entwickler (`web`, `mcp-namen`, `geheimnisse`), dem Codereview vom 2026-11-24, der
Integration, dem Gutachten, der Nachbesserung und dem PM-Abschluss (unten). Die vollständigen
Tabellen „Punkte“ (jeder Punkt der Runde mit Ergebnis und Beleg) und „Codereview“ stehen in den Berichten; dauerhaft
lesbar über die Commits (die Zweignamen `team/…` fallen beim Aufräumen weg):
`git show c96476e:docs/runde23/web.md`, `git show da6807a:docs/runde23/mcp-namen.md`,
`git show bc298bd:docs/runde23/geheimnisse.md` (der Ordner `docs/runde23/` ist nach dem Übertragen in die
Hauptdokumente gelöscht).

**Zuordnung (Nachbesserung 2026-11-24):** Jeder Punkt, der an seiner alten Stelle in diesem Dokument eine Marke für
Runde 23 trug (110 Zeilen), trägt dort jetzt sein Ergebnis nach den Tabellen „Punkte“: „erledigt R23“, „teilweise R23,
Rest offen“, Marke für Runde 24 (bleibt offen; seit der Nachbesserung R24 „offen nach R24“), „überholt“, „→ Nutzerfrage n“ bzw. „→ Nutzer, Handprüfung“. Die offenen stehen
zusätzlich unten in dieser Liste. Probe: `grep -n "→ R23" docs/Backlog.md` trifft nur noch diesen Abschnitt.

### Aus Gutachten und Nachbesserung (2026-11-24)

- (erledigt, Nachbesserung) Zwei Tokens aus der Zeit vor Runde 23, die erst die neue Handle-Regel zusammenlegte („Büro“
  bis R22 `@mcp:b-ro`, „buero“ `@mcp:buero`): der Token mit Umlaut behält seinen Alt-Handle
  (`mcp::session::keeps_legacy`), die Seite „MCP-Zugang“ sagt es; Tests `mcp/session.rs` und
  `round23_mcp_namen::two_old_tokens_that_only_the_new_rule_would_merge_stay_apart`.
- (erledigt, Nachbesserung) Kontingente ohne Token nicht mehr über neue Namen umgehbar: Modellaufträge aller Clients
  ohne Token zusammen höchstens vier (`quota::MAX_WITHOUT_TOKEN`), `add_post` (300 in 24 h) und offene Vorschläge (100)
  zählen für alle Clients ohne Token zusammen (`mcp/tools.rs::Counted`); Tests
  `round23_mcp_namen::clients_without_token_share_the_model_slots_whatever_they_call_themselves` und
  `…::add_post_and_propose_revision_have_size_limits_and_quotas`.
- (offen nach R24, niedrig) Mit `persona` ohne Token zählt für `add_post`/`propose_revision` der Bestand der genannten Persona
  (begrenzt durch die Zahl der Personas); Registervorschläge und Gliederungsanträge zählen weiter je Herkunft (dort
  begrenzen 2000 je Werk bzw. die Zustimmung des Menschen). Hängt an Nutzerfrage 15.
- (erledigt, Nachbesserung) Überholte Stellen der Doku: `?ok=sicherung` ohne `ziel` (RPC.md), `MCP_BODY_LIMIT`
  (Architektur.md), `defined_functions` mit Blockkommentaren, Klammertiefe und `#import` (RPC.md, Architektur.md),
  Vorwärtsverweise auf die abgeschlossene Runde (Architektur.md, WYSIWYG.md); README „Stand und Grenzen“.
- (erledigt, Nachbesserung) Handprüfung für den Nutzer zu Runde 23 in `HANDOFF.md` (Teil A Wegwerf-Instanz, Teil B Werk 3).

### Vor einer Veröffentlichung (Entscheidung des Nutzers, nicht Teil von Runde 23)

Der Wunsch vom 2026-11-24 heißt „heute abschließen und veröffentlichen“ (Webseite `baruch.bernau.family`). Runde 23 hat
bewusst nichts aus R24 vorgezogen; offen ist, was davon vor dem ersten Push gelten soll:

- (erledigt R24, S1: Version 0.9.0, `repository` gesetzt) **Zielversion** (damals 0.0.1; Versionsnummern legt nur der Nutzer fest) und **Adresse des Repos**
  (`repository` fehlt in `Cargo.toml`; `homepage` ist seit der Nachbesserung `https://baruch.bernau.family`).
- (→ Nutzer; in R24 nicht bearbeitet, Befund des Gutachtens R24 und Wege in „Nach Runde 24 offen“, Schritt „Historie“) **Git-Historie prüfen** vor dem ersten Push. Schon geprüft im Gutachten: `docs/buch.typ` war nie in
  der Historie (`git log --all -- docs/buch.typ` leer), keine offenen `// STUB:`. Nicht geprüft: die ganze Historie auf
  Namen, Adressen und Texte aus dem echten Buch in Commit-Meldungen und älteren Ständen.
- (erledigt R24 `webseite`: Vorgabe leer, Platzhalter `https://multigpt.example/mcp/`) **Vorgabe der MultiGPT-Verbindung** einer neuen Instanz war die Intranet-Adresse des Nutzers
  (`baruch-store/src/connections.rs`, `docs/Bedienung.md`): für fremde Nutzer leer lassen oder ein Beispiel setzen?
- (erledigt R24 `ci`: `ci.yml`, `release.yml`, `pages.yml`, `cargo deny`; noch nie gelaufen) **CI**: Es lag kein Arbeitsablauf im Repo (kein `.github/`); der Satz in `AGENTS.md` „Fast-Forward nach
  `main` löst die CI aus“ gilt erst, wenn es einen gibt. Dazu Lizenzprüfung der Abhängigkeiten.
- (teilweise R24 `pakete`: .deb und AppImage gebaut und gestartet; .dmg/.msi nur CI, nie gelaufen; Signierung offen) **Pakete** (.deb/AppImage, .dmg, .msi) und Signierung (Nutzerfrage 10) – ohne sie ist die Veröffentlichung
  eine Quelltext-Veröffentlichung (Bauen nach README „Schnellstart“).
- (erledigt R24 `webseite`: `--allow-host`, `docs/Betrieb.md`; an keinem echten Proxy geprüft) **Betrieb hinter Reverse-Proxy/TLS** (erlaubte Rechnernamen, Doku) – nur nötig, wenn die Webseite Baruch
  selbst zeigen soll und nicht nur beschreibt.
- (erledigt, PM-Abschluss) `docs/Fahrplan.md` für Runde 23 nachgeführt (Beginn/Ende aus `git log`, Gantt geprüft).

### Aus dem PM-Abschluss (2026-11-24)

Das Gutachten hatte 14 Befunde (7 mittel, 7 niedrig). Sechs mittlere behob die Nachbesserung (oben), der siebte ist die
Entscheidung des Nutzers („Vor einer Veröffentlichung“). Von den niedrigen:

- (erledigt, PM) Zusicherung ohne Begründung in `baruch-epub/tests/round23.rs` (`figure_label` mit Label) hat jetzt eine.
- (erledigt, PM) `fetch_allowed` lehnt auch `198.18.0.0/15` (Messnetze) und das ganze `64:ff9b:1::/48` (NAT64 für den
  eigenen Gebrauch) ab; vier neue Zeilen im Tabellentest, `docs/Architektur.md`.
- (erledigt, PM) Die Werkzeugbeschreibungen von `ask_persona` und `chat` in `tools/list` nennen die gemeinsame Grenze
  „alle Clients ohne Token zusammen höchstens vier“ (stand nur in `docs/MCP.md`).
- (erledigt, PM) Fahrplan nachgeführt; das fremde Bauverzeichnis `.claude/sicherheit-target/` aus dem Gutachten war
  beim Abschluss schon entfernt (`git status` sauber).
- (offen nach R24, niedrig) `ribbon::code_lines` zählt Klammern ohne Unterschied zwischen Markup und Code: Eine nicht
  geschlossene Klammer im Fließtext der Werkvorlage (oder eine maskierte `\[`) oberhalb einer `#let szene(…)`-Definition
  lässt die Knöpfe Szene/Arbeit/Bibelstelle still wegfallen (bis R22 zählte jede `#let` am Zeilenanfang). Sauber geht
  das nur mit dem Typst-Parser statt der Zeilenregel; kein Test mit unausgeglichenen Klammern.
- (offen nach R24, niedrig) `fetch_allowed` ohne DNS-Auflösung (Rebinding, Weiterleitung auf private Adressen): Grenze dessen,
  was Baruch leisten kann, weil MultiGPT die Seite abruft; ob MultiGPT filtert, prüft der Nutzer.
- (offen nach R24, niedrig) `/einstellungen/daten?ok=sicherung` zeigt „Sicherung angelegt“ bei jedem Aufruf (auch über einen
  Link) – mit dem Namen der neuesten vorhandenen Sicherung oder allgemein; kein beliebiger Name mehr, aber auch kein
  Beleg, dass gerade eine Sicherung entstand (`settings/data.rs`). Abhilfe wäre ein einmaliger Vermerk am Server.
- (offen nach R24, niedrig, Kosmetik) Token-Liste: „(früher `@mcp:…`)“ bricht in der schmalen Spalte mitten im Namen um.
- (bleibt, dokumentiert) Rechte eines vorhandenen Datenordners: siehe „Aus der Integration“.
- (aufgeräumt) Worktrees `.claude/worktrees/wf_20e1b3fd-3f9-3/4/5` samt `target/` entfernt; die Zweige `team/web`,
  `team/mcp-namen`, `team/geheimnisse` bleiben (gemergt).

**Codereview 2026-11-24:** Alle mittleren Befunde sind behoben (store-1, web-1, ki-1, ki-2 samt eigener Größengrenze
für `/mcp`, satz-1, quellen-1 bis -3), dazu die niedrigen store-2, store-3, web-2 bis web-5, ki-3, quellen-4, quellen-5
(neuer Datenordner 0700) und quellen-6. Offen bleiben niedrig: store-5, store-6 (unklar), satz-2, ki-5 (unten).

### Aus der Integration

- (→ Nutzerfrage 15) **„MCP ohne Token kann keine Beiträge unter Persona-Namen anlegen“** gilt nur eingeschränkt: Mit
  Token ist `persona` verboten, und ein Client ohne Token, der sich wie eine Persona oder ein Token nennt, schreibt
  als `@mcp:<name>` bzw. `@mcp:lokal-<handle>` (Abnahme `round23_abnahme.rs`). Das Argument `persona` vom eigenen
  Rechner ohne Token bleibt nach Vertrag §4 erlaubt (Loopback = der Mensch selbst; `docs/MCP.md`, „Bewusste
  Entscheidung“). Soll auch das gesperrt werden, ist es eine Zeile in `mcp/tools.rs::acting_persona` und bricht die
  Tests, die `persona` über Loopback nutzen.
- (offen nach R24, niedrig) `guard::cross_site_subresource` weicht in einem Punkt vom Wortlaut des Vertrags ab: Fehlen
  `Sec-Fetch-Mode` **und** `Sec-Fetch-Dest`, gilt die Anfrage nicht als Unterressource (Browser schicken die drei
  Kopfzeilen immer zusammen; ältere Tests bilden so den Link von außen nach). Rückweg zur wörtlichen Regel: Zweig
  `(None, None)` auf `true`, dazu `round12_search_ui`/`round13_quellen` um die beiden Kopfzeilen ergänzen.
- (offen nach R24, niedrig) Die Rechte eines **vorhandenen** Datenordners ändert Baruch nicht (nur neue bekommen 0700);
  `baruch.sqlite*` und `works/` entstehen weiter mit der umask. Der Dienst des Nutzers liegt unter dem geschützten Home.
- (offen nach R24, niedrig) Die Folgezeile des Knopfs „Zitat“ ist „– “; bleibt sie ungefüllt, steht ein einzelner
  Gedankenstrich im Text (`ribbon::QUOTE_SNIPPET`; Entscheidung web, in der Sichtprüfung bestätigt oder zu ändern).
- (offen nach R24, niedrig) R30-Widgets im Node-Harness: geprüft ist der Dialog „Verweis“; Suchleiste, Seitenleiste und
  Strg+Klick bleiben ohne Verhaltenstest. Ohne eigenen Test: store-3, web-5, quellen-6, `work.sheet_set` unter der
  Schreibsperre, Einhängen der Pfeiltasten im Menüband, ↻ Wiederholen eines Relay-Beitrags im Faden ohne Abschnitt.
- (keine Aufgabe) `baruch_exchange::sample` trägt Kapitel mit `==` unter Teilen; jedes Einlesen des Beispiels meldet
  dafür seit Runde 23 Hinweise in `deviations`.

### web

- (offen nach R24) `bibles::module_note` läuft weiter auf dem async-Faden (bis 2 s beim ersten Aufruf je Modul); „ein Lauf je
  Modul“ ist umgesetzt, `spawn_blocking` verlangt den Umbau von vier synchronen Aufrufstellen (`bibles.rs:choice_note`,
  `scripture.rs` dreimal).
- (offen nach R24) `with_image_formats` ergänzt `format:` weiter rein textuell (auch in Kommentaren/Raw), nur bei
  Zeichenketten-Literalen; Export nach Typwechsel. Nicht angefasst.
- (offen nach R24) Markierung eines Treffers im Satzbild nach Quelltext-Offsets; Suchen in der Markierung weitergeben;
  Trefferumfeld an Wortgrenzen; Chat-Höhe. Nicht angefasst (brauchen Sichtprüfung im Browser).
- (offen nach R24) Vorschau der Satz-Einstellungen verdrängt weiter einen Platz im Satz-Zwischenspeicher und ist nicht
  abbrechbar. Nicht angefasst.
- (offen nach R24) Block „Satz“: Platzhalter der Zahlenfelder, `aria-describedby` für Konflikt-Hinweise. Nicht angefasst.
- (offen nach R24) `cover_checks` setzt den Umschlag noch einmal nur für die Maße.
- (offen nach R24) Seitenzweige ohne Umstellung „Nummer an/aus“ und ohne Register-Projektion; Bildpfad ungleich
  `bilder/<Name>` im Spiegel. Kein kleiner Eingriff (Spiegel-Format), dokumentiert wie bisher.
- (offen nach R24, Codereview niedrig) store-5 (Löschen eines Gliederungspunkts protokolliert Vorschläge und Beiträge nicht),
  satz-2 (EPUB streicht ein echtes `;` hinter `_x_`/`*x*`), ki-5 (Modulangaben ungerahmt im Faktenblock der
  Stellenprüfung), store-6 (unklar: Wort-Diff in der Transaktion).
- (→ Nutzer, Sichtprüfung) Zitat im Satzbild (neuer Block ohne Auswahl), Sperre des Editors während des Speicherns,
  Namensschilder am Rücken, Farben der Registermarke in beiden Themen, Spalten der EPUB-Dateiliste, Pfeiltasten im
  Menüband – im Browser ansehen (Entwickler ohne Firefox). Ablauf in `HANDOFF.md` (Runde 23, „Handprüfung durch den
  Nutzer“).
- (offen nach R24) Geschachtelte Begriffslisten im Satzbild bleiben eine Insel; gemischte Listenzeichen und springende Nummern
  ebenso (das Blockmodell `Term {term, desc}` hat keine Tiefe; docs/WYSIWYG.md).
- (offen nach R24) Vorlagenfunktionen mit `p.at(i)±k`, `slice()` oder Inhalt als Ergebnis: Nummern in Baum und Gliederung können
  vom Satz abweichen (Grenze in docs/Architektur.md, dort auch der Vorschlag „Nummern aus dem Satz übernehmen“).
- (offen nach R24, bei Bedarf) Regelblock „// Baruch: Satz“: Register, Glossar und „Querverweis mit Seite“ kommen beim
  Re-Import nicht als Einstellung zurück.
- (offen nach R24) Kolumnentitel „Werktitel“ bleibt ohne `#set document(title: …)` leer (Hinweis `NO_DOCUMENT_TITLE` vorhanden).
- (offen nach R24) Abgleich der Satz-Einstellungen überschreibt nie; ein Knopf „übernehmen“ fehlt.
- (offen nach R24) `outline_mentions` kennt Ordnungszahlen nur bis „zwölfte“ und römische Zahlen bis XXXIX.
- (offen nach R24) Zeilenangaben in Satzmeldungen verschieben sich bei Teilen und Abschnitten ohne Nummer hinter der Ankerzeile.
- (offen nach R24) Kapitelzuordnung einer geteilten Seite über `page_lines` zählt nur Text (nicht Bild oder Linie).
- (offen nach R24) Die Tests prüfen weiter nicht durchgehend, ob Typst die erzeugte Ausgabe setzt.
- (offen nach R24) Zähl-Abgleich Typst ↔ Baruch (Wörter/Zeichen) fehlt.
- (offen nach R24, braucht einen Browser) `the_topbar_stays_on_one_line_at_1024_pixels` prüft nur CSS-Regeln und misst keine
  Breite der Kopfzeile.
- (offen nach R24, Fachentscheidung) Wasserzeichen überdeckt ein `foreground` der Vorlage in Ausgaben mit Entwurf;
  EPUB-Titelseite ohne Wasserzeichen. Dokumentiert, keine Änderung.
- (offen nach R24) Typst lässt sich nicht hart abbrechen: Auswertung und Umbruch laufen zu Ende und halten so lange die Sperre
  (nicht wieder versucht, siehe SKILLS.md).
- (keine Aufgabe, bei Bedarf) Die Saat einer neuen Datenbank legt die Teile des Beispielwerks weiter mit leerem Text an.
- (→ Nutzerfrage 14) Befund-Blöcke kompakter: jeder Listenpunkt einer Persona-Antwort ist ein eigener Block.

### mcp-namen

- (offen nach R24, niedrig) Clients ohne Token sind untereinander nicht unterscheidbar; `persona` ohne Token bleibt erlaubt (bewusste Entscheidung, siehe MCP).
- (erledigt, Nachbesserung R23) ~~Das Tageskontingent von `add_post` und die Grenze offener Vorschläge zählen je Autor; ein Client ohne Token umgeht sie mit einem neuen Namen (anders als bei Bildern).~~ Alle Clients ohne Token zählen zusammen (siehe „Aus Gutachten und Nachbesserung“).
- (offen nach R24, niedrig) Das Bildkontingent eines Tokens zählt nur den neuen Handle; Bilder, die derselbe Token in den 24 Stunden vor dem Einspielen unter dem Alt-Handle schrieb, zählen nicht mit.
- (offen nach R24, niedrig) Angehängte Befunde (`similar_to`) bleiben beim Verschieben eines Beitrags an ihrem Hauptbefund hängen, auch wenn der in einem anderen Abschnitt steht.
- (offen nach R24, niedrig) `all_done` in `persona.chat` sagt nicht, welche Persona es meldete (die Beiträge zeigen es).
- (→ Nutzer, Handprüfung) Lektor-Regel 6 „bei einer Prüfung“: Ton im Chat am echten Modell prüfen.

### geheimnisse

- (→ Nutzer, unverändert) Hochladen nach MultiGPT höchstens 30 s (`SESSION_CEILING`): nicht geändert (nur nach
  Handprüfung).
- (→ Nutzerfrage 16) Befunde beim Abgleich „vorspulen“ wie Fassungen (Status und Verlauf übernehmen, wenn der Verlauf
  hier ein Präfix des Verlaufs der Datei ist): bewusst nicht gebaut; Regel dieser Runde ist „bleibt, mit Hinweis“.
- (offen nach R24, niedrig) Lage „lokal/extern“ bei Intranet-Namen: weiter ohne DNS (siehe Punkte).
- (offen nach R24, niedrig) `scrub` erkennt Hex, JSON-Escapes, doppelte Kodierung und umbrochenes Base64 nicht.
- (offen nach R24, niedrig) `fetch_allowed` ohne DNS: öffentliche Namen auf private Adressen (auch der eigene
  Intranet-Name) gehen an MultiGPT.

## Nach Runde 22 offen (PM-Abschluss 2026-11-10)

Aus den Berichten der Entwickler, der Integration, dem Gutachten (15 Befunde: 5 mittel, 10 niedrig) und der
Nachbesserung; vom PM zugeordnet. Die fünf mittleren behob die Nachbesserung `be1584e`, der PM die zehn niedrigen bis
auf die unten genannten (Seitenzweig-Bildpfade ab der Wurzel, Hinweis „veraltet“ an Vorschlägen auf älterem Stand,
„Erledigt: Befund n“ nur zu Befunden, die im Auftrag standen, `post.repeat` überspringt nur offene Rückfragen
„erledigt“, Autor einer `<fortschreibung>` 1–200 Zeichen beim Einlesen, Archivseite nennt die Werkart, Begründungen in
`round22_fassungen.rs`/`round22_seitenzweige.rs`, Architektur `<kN>`, MCP-Tabelle `revision_amends`, Fahrplan). Nach
Runde 22 **Pause** – keine neue Runde vorbereitet.

- (erledigt R23, Nachbesserung R22) Ändern über den Baum (`revision.update`) auf einer fortgeschriebenen Fassung legt keinen
  Schritt im Verlauf an; der Verlauf hat dann eine Lücke (das Lesen im Austausch duldet sie bewusst, `fast_forward`
  spult dann nicht vor). Schritt „bearbeitet“ im Verlauf anlegen oder dokumentiert lassen.
- (erledigt R23, Gutachten R22) Beim Kürzen des Kontexts gestrichene eigene Befunde zählen weiter als „im Auftrag“
  (`Prepared::own_findings`), falls die Kürzung die Liste nicht nachführt – prüfen.

- (→ Nutzer, Handprüfung) Nach dem Einspielen **Migration 0050** (Umbau `uuid_tombstone`, neue Spalten/Tabellen) erst
  beim nächsten Neustart des Dienstes – vorher Sicherung von `~/.local/share/baruch`. Schritte in `HANDOFF.md`
  (Runde 22).
- (→ Nutzerfrage) Fortschreiben über die übrigen Schreibwege (§2.7): Bild einfügen, Registermarken, Verweis umwandeln,
  Ersetzen und Gliederungsaktionen legen weiter immer neue Fassungen an (`save_human`, `save_human_on`, `outline_save`).
- (→ Nutzerfrage) Option „Sitzung“ statt Minuten für die Zeitgrenze.
- (→ Nutzerfrage) `change_percent` nach der Vertragsformel zählt Leerräume als gleiche Teile (ein völlig neuer Satz
  ≈ 50 %); Vorgabe 20 % entspricht grob einem Drittel bis der Hälfte geänderter Wörter. So lassen oder auf reine
  Wortteile umstellen.
- (erledigt R23) Rückfrageseite vor Teilen/Zusammenführen (`outline_confirm.html`) reicht `mode` nicht weiter: die neue
  Fassung entsteht richtig, die Rückmeldung nennt aber keinen Grund `heading_changed` (`based_on_amend` reicht sie seit
  der Nachbesserung weiter).
- (offen nach R24) Seitenzweig-Inhalt ohne Umstellung „Nummer an/aus“ und ohne Register-Projektion (dokumentiert).
- (erledigt R23) `persona.chat` und MCP `ask_persona` geben `done_proposals`/`done_notes`/`all_done` nicht eigens aus.
- (→ Nutzerfrage) Archiv: Bilder und Werkart (Predigt) kommen beim Wiederherstellen nicht mit; ältere Fassungen,
  Beiträge und Befunde nur in der Git-Historie des Archivordners.
- (offen nach R24) Spiegel setzen: Rasterbilder liegen im Spiegel als `bilder/<name>.b64`; ein Bildpfad ungleich
  `bilder/<Name>` fehlt im Spiegel (dokumentierte Abweichung).
- (offen nach R24) Wasserzeichen: Eine Vorlage, die selbst `foreground` setzt, wird in Ausgaben mit Entwurf überdeckt; ein EPUB
  ohne Titelei bekommt mit Entwurf eine Titelseite (Fachentscheidung, dokumentiert).
- (erledigt R23) JS-Wege des Editors (Knopf-Weitergabe über `requestSubmit(submitter)`, Hinweis nach dem Speichern) ohne
  eigenen Browser-Verhaltenstest (Sichtprüfung R22 mit und ohne JavaScript ohne Befund).
- (erledigt R23, Befund Nutzer 2026-11-10 am echten Buch) **Zitat-Werkzeug:** Der Knopf „Zitat“ (`ribbon.rs`) fügt
  `#quote(block: true)[]` mit nur einem Zeilenumbruch danach ein. Schreibt der Autor die Stellenangabe in die nächste
  Zeile, klebt sie am Block; der Editor (`baruch-wysiwyg::parse::paragraph`) erkennt einen Block nur, wenn er den ganzen
  Absatz füllt, und zeigt ihn sonst als Code-Insel („Zitate werden nicht gerendert“, Werk 3, Ps 139). Einfügen mit
  Leerzeile davor und danach und mit Platz für die Angabe (`attribution: []` oder „– Stelle“ im Rumpf, Cursor in den
  Rumpf); zusätzlich eine Zeile direkt nach `]` als eigenen Absatz lesen wie Typst, statt den Block zur Insel zu machen.
- (keine Aufgabe, Integrator) Austausch: Stand 0 (`beruht-stand`, `stand` an Befunden) wird nicht geschrieben – sonst
  wären Dateien ohne Fortschreibung nach den älteren Schemata ungültig gewesen (fünf ältere Tests fielen beim
  Zusammenführen).

## Nach Runde 21 offen (PM-Abschluss 2026-11-09)

Aus den Berichten der Entwickler, der Integration, dem Gutachten und der Nachbesserung; vom PM zugeordnet.

- (→ Nutzer, Handprüfung) Sieben Schritte auf einer Wegwerf-Instanz in `HANDOFF.md` (Runde 21); Migration **0108**
  der Bibliothek erst beim nächsten Neustart des Dienstes – vorher Sicherung von `~/.local/share/baruch`.
- (offen nach R24, Gutachten R21, niedrig) `baruch_render::with_image_formats` ergänzt `format:` rein textuell, auch in
  Kommentaren, Raw-Blöcken und Zeichenketten; `export --out` schreibt nach einem Typwechsel PNG-Bytes unter `.svg`
  mit einem Typst-Text, der außerhalb von Baruch nicht setzbar ist (Export umbenennen oder `format:` mitschreiben).
- (erledigt R23, Gutachten R21, niedrig) MCP-Kontingent für Bilder gilt je selbst gewähltem Namen
  (`clientInfo.name`, `persona`): ein Client ohne Token vom eigenen Rechner bekommt mit neuem Namen ein neues
  Kontingent (in `docs/MCP.md` vermerkt). Zusammen mit den eindeutigen Handles lösen.
- (erledigt R23, Gutachten R21, niedrig) `markdownTitle` in `web/src/works.ts` ohne Test und ohne Gleichlauf mit
  `markdown_import::convert` (Titelvorschlag kann vom vergebenen Titel abweichen).
- (erledigt R23, Sichtprüfung R21) Liste „Dateien und Kapitelgrenzen“ in der EPUB-Vorschau verteilt sich ungleichmäßig auf
  Spalten (älteres Layout, `08_epub_vorschau_cover.png`); Bildschirmfotos der Sichtprüfung R21 nur im hellen Thema und
  „mit JS“/„ohne JS“ byte-gleich – nächste Sichtprüfung beide Themen und den Dialog „Importieren…“ mit JavaScript.
- (keine Aufgabe, Nachbesserung R21) Eine EPUB-Fußnote ohne Rücksprung-Link, die wirklich mit ihrer eigenen Nummer
  beginnt („1 Kor …“ als Fußnote 1), verliert die Zahl; die Warnung `import-epub-footnote-number` sagt es.
- (keine Aufgabe, Nachbesserung R21) Lokale PDF-Lesevorgänge warten zu zweit in einer Schlange; Bilder heißen im EPUB
  nach einem Typwechsel z. B. `images/haus.svg.png` (nur gegen die eigene Prüfung geprüft, nicht gegen EPUBCheck).
- (→ Nutzer, Handprüfung) MultiGPT-Annahmen von R21a ungeprüft: `upload_document {collection, content_base64,
  filename}`, `run_status`, `document_info` mit Inhaltsverzeichnis, `read_document` mit `page_from`/`page_to`/
  `truncated` (Architektur „MultiGPT“). Lokaler PDF-Weg mit echten PDFs prüfen (Ebene 3 nur über Nummern oder größere
  Schrift; Hervorhebungen, Fußnoten und Bilder aus PDF kommen nicht mit – Warnung `import-pdf-plain`).
- (R23 nicht geändert → Nutzer, Handprüfung, nur falls die Handprüfung es zeigt) Hochladen nach MultiGPT in einem Aufruf mit höchstens 30 s (`SESSION_CEILING`): sehr große
  PDFs über langsames Netz scheitern, die Meldung bietet „lokal lesen“ an.
- (offen nach R24) Geschachtelte Begriffslisten im Satzbild bleiben eine Insel (docs/WYSIWYG.md).
- (offen nach R24) Vorlagenfunktionen mit `p.at(i)±k` oder `slice()`: Nummern in Baum und Gliederung können abweichen.
- (erledigt R22, mit dem Integrator) Titel mit Code an einem gleichlautenden nummerierten Knoten: Show-Regel über `<kN>`.
- (offen nach R24, bei Bedarf) Regelblock „// Baruch: Satz“: Register, Glossar, „Querverweis mit Seite“ kommen beim Re-Import
  nicht mit; Bundsteg kommt als Teil des Innenrands zurück (Satz gleich).
- (erledigt R23) Arbeitsseite mit `?lq=`: Kasten „Bilder des Werks“ steht nicht im ausgelieferten HTML
  (erst über `literature.render` im Popup; ohne JavaScript über `/literatur`). Eine Include-Zeile in
  `fragments/library.html` würde genügen.
- (offen nach R24, mit `with_image_formats` oben) `format:` wird nur in `image()`-Aufrufen mit Zeichenketten-Literal ergänzt; berechnete Pfade der Werkvorlage
  mit Typwechsel scheitern weiter in Typst.
- (dokumentiert, kein Fehler) `pdfinfo` meldet bei jedem Typst-PDF die Suspects-Zeile (krilla schreibt `/Suspects
  false`, poppler 25.03 meldet jeden Wert außer `true`).
- (→ Nutzer, Handprüfung) EPUB-Cover rastert jetzt immer (auch ohne Umschlagbild); Sichtprüfung am echten Werk.

## Nach Runde 31 offen (PM-Abschluss 2026-11-09)

- (→ Nutzer) Handprüfung mit Werk 3 nach dem Einspielen (**Migration 0049 – vorher Sicherung** von
  `~/.local/share/baruch`): neun Schritte mit erwarteten Ergebnissen in `HANDOFF.md` (Runde 31, „Handprüfung durch den
  Nutzer mit Werk 3“), darin Kontextmenü, Dialoge und Popups von Hand (bisher nur headless Firefox mit synthetischem
  Rechtsklick).
- (erledigt R21b: nur ein erzeugtes Buchende wird geschnitten, Bericht `import-back-matter-cut`/`-kept`; Gutachten R31, niedrig) `strip_back_matter` schneidet beim Import still alles ab der ersten
  Zeile `// Baruch: Register und Glossar` ab, auch wenn dahinter kein erzeugtes Buchende steht (Kommentar von Hand im
  Buchtext); der Importbericht nennt den Schnitt nicht. Lösung: nur schneiden, wenn der Block wie erzeugt aussieht
  (Hilfsfunktionen/Labels), und den Schnitt im Bericht melden.
- (erledigt R21b: Vermerk `/*baruch-erste-erwaehnung*/` im Export, beim Import entfernt; Gutachten R31, niedrig) Import eines Exports mit „erster Erwähnung“ ohne Glossarseite: `#emph[…]` bleibt als
  Hervorhebung im Text (Rundreise Export → Import ändert den Text; in `docs/Bedienung.md` vermerkt, ungetestet). Lösung
  z. B. ein unsichtbarer Vermerk der Projektion im Export.
- (erledigt R23, Gutachten R31, niedrig) Austausch u7: `exchange_apply.rs` (`insert_term`, `insert_register`)
  zählt die Begriffe nicht gegen `MAX_GLOSSARY_TERMS` und prüft die Länge eines Registertitels nicht
  (`MAX_REGISTER_TITLE_CHARS`); `check_u7` auch nicht. Danach lehnt `glossary.add` jedes weitere Anlegen ab. In
  `docs/Austauschformat.md` als „noch nicht geprüft“ berichtigt.
- (erledigt R23, Gutachten R31, niedrig) `index_propose` ohne Obergrenze offener Registervorschläge je Werk bzw.
  Client (jeder ein Spiegel-Commit); Glossarvorschläge sind über `MAX_GLOSSARY_TERMS` gedeckelt.
- (erledigt R23, Integration R31) MCP `index_propose` meldet `node` statt `key` als `isError`, die älteren schreibenden
  Werkzeuge als -32602 – angleichen.
- (erledigt R23, Gutachten R31, niedrig) Registermarke im Satzbild mit festen Farben (`wysiwyg.css` `#8a6d1f`,
  `#2f6f8f`, `#b45309`) statt Themen-Tokens; Hell/Dunkel für Kontextmenü, Dialoge, Glossar- und Registerseite nicht im
  Bild geprüft (alle zehn R31-Bilder hell).
- (erledigt R23, Integration R31) Typst-Meldungen aus dem Buchende ordnet `ExportMap::section_at_line` dem letzten Abschnitt zu.
- (erledigt R23, Nachbesserung R31) `sameMarkBehind` im Satzbild (`wysiwyg.ts`) ohne eigenen Test (braucht DOM); zusammen mit
  dem Browser-Verhaltenstest der neuen Widgets.
- (teilweise R24 `ci`: Schritt in `ci.yml` über `scripts/ci-typst.sh`; lokal siehe „Nach Runde 24 offen“) `typst compile` des Exports nur mit dem eingebetteten Compiler geprüft, die Kommandozeile fehlt auf dem
  Rechner; in der CI mit installiertem `typst` laufen lassen.
- (keine Aufgabe) Ohne JavaScript lässt sich eine Marke nur entfernen und neu setzen (Vertrag: nur `unmark` als
  Formular). Stichwörter verschiedener Groß-/Kleinschreibung bleiben getrennt (PM-Entscheidung, siehe R31).
- (erledigt PM R31) Rustdoc von `back_matter` stand vor `strip_back_matter`; `docs/MCP.md` „Integration“ war Platzhalter.

## Nach Runde 20 offen (Nachbesserung 2026-11-09)

- (→ Nutzer, Buchtext – kein Entwicklungsauftrag) **Wunsch 2026-11-09 am echten Buch: Einleitung „Der blinde
  Fleck“.** „Das Buch braucht als Einleitung die Erklärung des blinden Flecks. Wir fangen bei der Biologie des Auges
  an und gehen über die Analogie des Sehens weiter zur Seele des Menschen.“ Bogen der Einleitung: (1) Biologie des
  Auges – wo der Sehnerv die Netzhaut verlässt, sieht das Auge nichts, und das Gehirn füllt die Lücke unbemerkt;
  (2) Analogie des Sehens – was wir nicht sehen, merken wir nicht einmal als Lücke; (3) die Seele des Menschen – der
  blinde Fleck im Blick auf sich selbst. Weg in Baruch nach den Regeln (AGENTS.md): Den Gliederungspunkt
  „Einleitung“ darf ein Assistent anlegen (MCP `outline_add`, Herkunft im Spiegel); den Text schreibt der Mensch
  oder eine Persona als **Vorschlag** (`ask_persona` mit `propose`, `propose_revision`), übernehmen nur der Mensch.
  Kein Agent der Entwicklerrunden schreibt am echten Werk. Gehört nicht zu R22 (dort nur „Kleine Änderungen ohne neue
  Fassung“ und „Personas fragen, ob eine Kritik erledigt ist“ aus derselben Anfrage).
- (erledigt R21c: `cover::front_cover_source`, `baruch_render::render_png`; Gutachten R20) **EPUB-Cover mit Titel aus der Umschlag-Anordnung.** Heute ist das EPUB-Cover die
  rohe Umschlag-Bilddatei (`epub.rs` `EpubImage` aus `asset_data`); Titel, Untertitel und Autor der R27-Blöcke fehlen,
  obwohl „Umschlag und Bilder über MCP“ „Auch im EPUB als Cover“ mit Titel verlangt. Bei Werk 3 trägt das Bild keinen
  Titel, die Bibliothek des Readers zeigt nur das Bild. Lösung braucht ein Rasterbild der Vorderseite aus dem
  Umschlag-Satz (z. B. `typst-render`, neue Abhängigkeit, PNG/JPEG in Cover-Größe 1600×2560) statt der Rohdatei; SVG
  scheidet für Kindle aus. Bis dahin in `docs/Bedienung.md` („Umschlag und Titelei“) als „noch nicht“ vermerkt.
- (→ Nutzer) Handprüfung mit Werk 3 nach dem Einspielen: nummerierte Schritte mit erwarteten Ergebnissen in
  `HANDOFF.md` (Runde 20, „Handprüfung durch den Nutzer mit Werk 3“).
- (erledigt Nachbesserung R20) Anmerkungen im Endnoten-Modus auf oberster Ebene des Inhaltsverzeichnisses; Label in
  einer Fußnote zeigt im Endnoten-Modus nach `notes.xhtml`; `READER_PUBLISHER_FONT` nur bei eingestellter Schrift;
  Prüfseite lehnt fremde Seiten mit 403 ab. (erledigt PM) `epub_export`/`epub_check` in der Werkzeugtabelle von
  `docs/MCP.md`; Themenfarben `--warning`/`--success`/`--line` in `epub_check.css`/`epub.css` (Dunkelmodus);
  Begründung an der letzten Zusicherung von `round20_abnahme.rs`.
- (erledigt R23, Gutachten R20, niedrig) Fußnote in einer Fußnote: die innere Note landet vor der äußeren in der Liste
  (`xhtml.rs` `footnote()`), `start` und sichtbare Nummern verrutschen.
- (erledigt R23, Gutachten R20, niedrig) `@label` auf eine Abbildung zeigt den rohen Labelnamen statt „Abbildung n“ wie Typst.
- (erledigt R23, Gutachten R20, niedrig) Temp-Ordner für EPUBCheck `/tmp/baruch-epubcheck-<pid>-<n>` vorhersagbar und mit
  Standardrechten; auf Mehrbenutzer-Rechnern lesbar bzw. vorab anlegbar. Lösung: Zufallsname, `DirBuilder::mode(0o700)`.
- (erledigt R23, Gutachten R20, niedrig) Widget `epub_preview.ts` schickt bei jedem `change` ab; Pfeiltasten in einer
  geschlossenen Auswahlliste laden in Firefox die ganze Vorschau neu (WCAG 3.2.2). Auswahllisten erst bei Bestätigung.
- (erledigt R23, Integration R20, niedrig) `check_mimetype` prüft den Inhalt eines komprimierten mimetype nicht.
- (kosmetisch) In E-Ink-Rahmen bleibt rechts ein schmaler hellerer Streifen (vermutlich die Scrollleiste des iframes).
- (Sichtprüfung R20, offen für den Nutzer) Vorschau-Rahmen: Firefox kürzt den Referer aus dem `sandbox`-Rahmen auf
  `http://host/`; `from_own_frame` nimmt den gekürzten Referer desselben Hosts an. Andere Browser ungeprüft.

## Nach Runde 19 offen (PM-Abschluss 2026-11-09)

- (→ Nutzer) Handprüfung nach dem Einspielen (keine Migration, trotzdem **vorher Sicherung** von
  `~/.local/share/baruch`): RAG-Status bei kurz fehlendem Eliezer, „Index neu aufbauen“ am echten Werk (Dauer der
  Einbettung festhalten), Präfixe für nomic-embed-text (ein geändertes Dokument-Präfix legt LanceDB bis zum Neuaufbau
  still), eigenes pgvector mit TLS-Hinweis, Prüfmodul mit Trennresten, MultiGPT-Annahmen, Gruppen benennen mit dem
  echten Lektor. Schritte in `HANDOFF.md` (Runde 19).
- (→ Nutzerfrage 1) Ältere Fassungen und Befunde/Beiträge im RAG-Index; (→ Nutzerfrage 11) Kommentare/Befunde nach
  MultiGPT.
- (erledigt R21a: Ersatz `rank_score` aus dem Rang, nie „schwach“; Gutachten R19, niedrig) MultiGPT ohne `score`/`distance`: der Ersatz-Score aus dem Rang (1/Rang) macht jeden
  Treffer ab Rang 3 „schwach“, Rang 1 und 2 nie (`connections/multigpt.rs`, `meta.score == "aus Rang"`). Abhilfe:
  Treffer „aus Rang“ nicht nach `WEAK_SCORE` einstufen (oder ohne Angabe „Ähnlichkeit unbekannt“).
- (erledigt R23; Gutachten R19, niedrig) Leerraum-Vergleich (`baruch-scripture/src/wording.rs` `matches_without_spacing`)
  gilt für jedes Modul und lässt innere Wortgrenzen frei: „die Bein“ gegen „Diebe in“ ergibt „stimmt (Leerraum im
  Modul)“. Abhilfe: nur bei auffälligem Modul (`fragment_survey`) oder nur Grenzen tolerieren, an denen das Modul ein
  Wortfragment hat; Test mit solchen Paaren.
- (erledigt R23; Gutachten R19, niedrig) „Joh 3,16 Mal 3“ (Maleachi nur mit Kapitel): „Mal“ gilt als Zahlwort, Joh 3,16
  entfällt still (`baruch-scripture/src/lib.rs` `count_word_follows`). Mehrdeutig („16 Mal“) – Regel festlegen und testen.
- (erledigt R23; Gutachten R19, niedrig) Gedankenkarte: `idea.map_group_accept` übernimmt einen gespeicherten Vorschlag auch,
  wenn die Gruppe inzwischen andere Einfälle hat (Anzeige blendet ihn aus, `of_map`); scheitert ein `idea.update`
  mittendrin, bleiben die schon geänderten Einfälle geändert. Abhilfe: veralteten Vorschlag ablehnen, Grenzen vorab.
- (teilweise R23, Rest offen nach R24; Gutachten R19, niedrig) `bibles::module_note` liest die Stichprobe (bis 4000 Verse/2 s) blockierend im
  async-Pfad und ohne Single-Flight; `spawn_blocking` und ein Lauf je Modul.
- (erledigt R23) Test `round19_index::reindex_answers_at_once_while_an_embedding_hangs_and_rebuilds_after_release` kann
  grün sein, ohne dass etwas unterbrochen wird (Freigabe vor der nächsten 50-ms-Abfrage); Wartezeit vor der Freigabe
  wie im Nachbesserungstest.
- (teilweise R24 `webseite`; die Annahme war falsch) `PGSSLROOTCERT`: sqlx 0.8.6 liest die Variable, prüft das
  Zertifikat aber nur bei `verify-full` (bzw. `verify-ca`); bei `require` nimmt es jedes Zertifikat an (Quelltext
  gelesen, nicht gegen einen Server geprüft). Dokumentiert in `docs/Betrieb.md`. Offen: Prüfung gegen PostgreSQL mit
  TLS; der TLS-Hinweis der Seite Verbindungen kennt die Umgebungsvariablen nicht.
- (erledigt R24 `ci`: grün, 34 Dateien mechanisch, fünf `#[allow]` mit Begründung) `cargo clippy -- -D warnings` scheiterte an älteren Warnungen (u. a. `baruch-rag/src/lib.rs` `!(score >=
  WEAK_SCORE)`, gewollt wegen NaN – `allow` mit Begründung), baruch-typst, baruch-store, baruch-wysiwyg; vor der CI
  aufräumen.
- (keine Aufgabe) Die Verbindungsprüfung MultiGPT sendet kein `DELETE`, wenn das gemeinsame Zeitlimit (10 s) sie
  abbricht; die Sitzung läuft beim Server aus.
- (keine Aufgabe) `ws_command.ts` (in SKILLS als eingefroren geführt) wurde in R19b bewusst um `scripture`/`cite`
  erweitert; reine Bausteine `scriptureFunction`, `scriptureSource`, `citeKey`.
- (keine Aufgabe) Annahmen über das echte MultiGPT (Rechte-Wörter in `isError`, `upload_document` ohne ID, stilles
  Kappen von `limit` nur über `maximum` erkennbar, Skala der Ähnlichkeit) stehen in `docs/Architektur.md`; prüft der
  Nutzer (Nutzerfrage 3).

## Nach Runde 29 offen (Integration 2026-11-09)

- (→ Nutzer) Handprüfung nach dem Einspielen (**vorher Sicherung** von `~/.local/share/baruch`): beim ersten Start läuft
  Migration 0048 (neue Tabelle `work_typeset`, Umbau von `uuid_tombstone` wie 0046, verlustfrei getestet in
  `migration_0048.rs`). Werk 3: Angebot zur Show-Regel aus Vorlagenfassung 2 annehmen, Kapitel stehen danach weiter
  auf rechten Seiten. Schritte in `HANDOFF.md` (Runde 29).
- (erledigt R21b: `typeset::settings_from_rules`, nur wenn exakt zurücklesbar) Ein Export mit Regelblock „// Baruch: Satz“ beim Einlesen wieder in Satz-Einstellungen zurückverwandeln
  (heute landet der Block als Teil der Werkvorlage im neuen Werk).
- (erledigt R20a) EPUB nutzt `baruch_typst::typeset::epub_style` (CSS, Kapitel-Dateien, Sprache) – heute nur Datenmodell.
- ~~(überholt, war R23a) Leerseiten vor Teil-Seiten erkennt der Regelblock nur mit eigener Teil-Einstellung (`part_page`).~~
  **Erledigt in der Nachbesserung `066ab17`** (`part_page_rule_for` setzt die Marke auch mit der Standard-Teilregel;
  Messtest `the_blank_page_before_a_second_part_stays_empty`).
- (offen nach R24) Vorschau der Satz-Einstellungen (`work.typeset_preview`, Formular „Vorschau“) setzt über `typeset::compile`:
  nicht über `preview.cancel` abbrechbar, hält bis zum Ende die einzige Satz-Sperre; jeder Vorschaustand verdrängt einen
  der 8 Plätze im Satz-Zwischenspeicher (Buchsatz wird dann neu gerechnet). Eigener Zwischenspeicher oder `compile_for`.
- (offen nach R24) Block „Satz“: Platzhalter der Zahlenfelder (6em) abgeschnitten; Konflikt-Hinweise und Fehler beim Feld nicht
  per `aria-describedby` verbunden (die Erklärungen der Satzbegriffe sind es seit `066ab17`).
- (erledigt R23) `work.typeset_adopt_rule` verlangt über RPC keine Bestätigung (wie `work.typeset_reset`; RPC ist der Weg des
  Menschen, das Formular verlangt das Häkchen). Falls ein Widget die Methode nutzt: Bestätigung dort.
- Verhaltensänderung (Nachbesserung `066ab17`, bewusst): Mit gesetztem Kapitelanfang bricht der Export auch vor Teilen
  um, wie die übernehmbare Vorlagenregel; Werke ohne Satz-Einstellungen bleiben byte-gleich.
- (offen nach R24) Kolumnentitel „Werktitel“ liest `document.title`; ohne `#set document(title: …)` bleibt er leer (Hinweis
  `NO_DOCUMENT_TITLE` in `work.typeset_get` und im Block „Satz“).
- (offen nach R24) Abgleich der Satz-Einstellungen überschreibt nie: Ändern und Zurücksetzen auf einer Instanz erreichen die
  andere nur als Hinweis bzw. „hier gelöscht“ (Regel in `docs/Austauschformat.md`); ein „übernehmen“-Knopf je Hinweis
  fehlt.
- (erledigt R23) Ältere Server-Tests prüfen ihre Ausgaben weiter gegen das u5-XSD (round26_export, round26_roundtrip,
  round27_daten, round28_befunde, round30_varianten) – gültig, solange sie kein `<satz>` ausgeben.

## Nach Runde 30 offen (Integration 2026-11-09)

- (→ Nutzer) Handprüfung nach dem Einspielen (**vorher Sicherung**): beim ersten Start läuft Migration 0047 (nur
  angehängte Spalten und die Tabelle `replace_batch`), und durch die Sprungmarken `<kN>` ändert sich jeder Spiegel
  einmal (ein Commit je Werk durch `sync_all`). Schritte in `HANDOFF.md` (Runde 30).
- (teilweise R23, Rest offen nach R24) Die DOM-Teile der neuen Widgets (Suchleiste, Seitenleiste, Markierung im Satzbild, Strg+Klick, Dialog
  „Verweis“, Live-Anzeige) sind nur über reine Funktionen unter Node und die headless Sichtprüfung des Integrators
  geprüft; ein Verhaltenstest im Browser fehlt.
- (offen nach R24) Die Markierung eines Treffers im Satzbild sucht den Treffertext und bildet die Quelltext-Offsets nicht genau
  ab (bei mehrfach vorkommendem Text kann die falsche Stelle markiert werden).
- ~~(→ R29c) Verweis mit Seitenzahl als Werk-Einstellung; heute merkt sich der Browser die Wahl (localStorage).~~
  **Erledigt in Runde 29c** (Einstellung `xref_with_page`, `xref.ts` liest `work.typeset_get`; localStorage nur ungesetzt).
- (erledigt R20a) EPUB: die Sprungmarken `<kN>` der Export-Quelle als interne Links abbilden.
- (→ Nutzerfrage) Vorwärts-Sprung per Taste (heute nur Knopf; Alt+→ bleibt „nächste Fassung“, siehe
  `docs/Bedienung.md` „Blättern durch die Fassungen“).
- (offen nach R24) `outline_mentions` kennt Ordnungszahlen bis „zwölfte“ und römische Zahlen bis XXXIX.
- (offen nach R24) Bei Teilen und Abschnitten ohne Nummer verschieben sich Zeilenangaben in Satzmeldungen hinter der
  Überschrift um eine Zeile (Ankerzeile).
- (erledigt R23) Varianten: Eine Fassung, die beim Abgleich (Austausch) eine abgeleitete Kennung bekommt, verliert ihr
  Varianten-Kennzeichen.
- (R23 nicht geändert → Nutzerfrage 14) Jeder Listenpunkt einer Persona-Antwort wird ein eigener Befund-Block mit Statusknöpfen; bei langen Listen
  von Hinweisen wird der Beitrag lang. Prüfen, ob Hinweise kompakter gesetzt werden (Nutzer beim Handtest fragen).
- (offen nach R24) Sichtprüfung R30: Der Chat behält seine feste Höhe aus Runde 28, unter den Beiträgen bleibt im Akkordeon
  Platz frei; im Trefferumfeld der Werk-Suche wird der Text hinter einem Treffer mitten im Wort abgeschnitten.
- (erledigt R23) MCP-Relay aus Runde 15 (`relay_section` in `mcp/tools.rs`) und Relay bei direkter Anrede (Runde 30c)
  laufen nebeneinander (seit der Nachbesserung in einer Stufe und beide gerahmt); zusammenlegen.
- (offen nach R24) Suchen in der Markierung: Die Seite `/suche` bietet „Markierung im Editor“ nur an, wenn die Adresse
  `start`/`end` trägt; der Editor selbst sucht in der Markierung über „⌶“ (Alt+L), gibt die Markierung aber nicht an
  den Link „im Werk …“ weiter. Die Offsets einer Markierung beziehen sich auf den Editortext; auf `/suche` (ohne
  ungespeicherten Text) gelten sie gegen die gewählte Fassung.
- Gutachten R30 (15 Befunde: 0 hoch, 6 mittel, 9 niedrig): Nachbesserung `88df145` behob die fünf mittleren im Code
  (Abgleich mit Querverweisen, Lösung nach Leerzeile, MCP-Relay zweistufig/ungerahmt, Ersetzen-Vorschau ohne
  Speichergrenze, Bereich Markierung/Fassung); der sechste mittlere war der PM-Merkposten Fahrplan. Im PM-Abschluss
  behoben: doppelte `picks` in `solution.adopt` (werden einmal gezählt), Etiketten der Varianten rücken beim Ausfall
  einer Variante nicht mehr nach, Querverweis-Farbe im dunklen Thema (`--primary` statt des nirgends gesetzten
  `--accent`), Werkzeugtabelle `text_search` in `docs/MCP.md` (`kinds`, erlaubte Bereiche), Alt+→ oben, Fälle aus
  Werk 3 in der Handprüfung (`HANDOFF.md`). Offen:
- (erledigt R23) ↻ Wiederholen eines Relay-Beitrags schickt den Auftragstext ohne den Relay-Rahmen (die Antwort der
  anredenden Persona fehlt im Prompt, `posts::repeat` baut `OrderFrame::default()`); Rahmen in `PostMeta.request`
  mitspeichern oder Relay-Beiträge ohne ↻.
- (erledigt R23) ↻ Wiederholen setzt immer `web: true` und `asker: None`: stammt der Auftrag von einem MCP-Client, sucht
  @belege nun im Netz mit dem Text des Clients, und die Herkunft geht verloren; Herkunft und `web` aus dem
  gespeicherten Auftrag übernehmen.
- (erledigt R23) Teilvorschlag bei Varianten langer Abschnitte: `variants::splice` normalisiert den ganzen Abschnitt
  (mehrfache Leerzeilen, Leerraum-Zeilen, Zeilenende); nur die betroffenen Absätze ersetzen, den Rest byte-gleich
  lassen.

## Nach Runde 28 offen (Integration 2026-11-09)

- (→ Nutzer) Handprüfung mit Werk 3 nach dem Einspielen (beim ersten Start läuft Migration 0046 und zerlegt
  `findings::backfill` einmal alle vorhandenen Persona-Beiträge, **vorher Sicherung**): Schritte in `HANDOFF.md`
  (Runde 28) – Befunde aus den RFC-Beiträgen 41–49, Status setzen, Matrix, Chat kopieren/rollen/bearbeiten im echten
  Browser (die Entwickler durften keinen Browser nutzen; der Integrator prüfte nur headless mit Bildschirmfotos).
- (→ Nutzerfrage 1) Ähnlichkeit nur über Inhaltswörter (Dice ab 0,8, seit der Nachbesserung R28 ohne Füllwörter): Umformulierungen mit anderen Wörtern werden nicht angehängt;
  Einbettungen und Befunde im RAG-Index bleiben offen bis Nutzerfrage 1 (in R19a nicht umgesetzt).
- (→ Nutzerfrage, Vorgabe: Vorschlag für den gezeigten Abschnitt) Neuer Abschnitt per Schreibauftrag aus dem Chat
  („Erstelle ein Vorwort“); heute wird daraus ein Vorschlag für den gezeigten Abschnitt, neue Abschnitte nur über die
  Gliederung bzw. `outline_add`.
- (erledigt R23) `finding_list` fragt je Befund den offenen Vorschlag einzeln ab (bis zu 2000 Abfragen je Arbeitsseite);
  eine Abfrage mit Fensterfunktion genügt.
- (erledigt R23) Abgleichen (Austausch u4): Verlaufseinträge zu einem schon bekannten Befund kommen nicht dazu, ein
  abweichender Status gibt nur einen Hinweis; Regel festlegen und dokumentieren.
- (erledigt R23) `findings.ts` hat keinen Verhaltenstest im Node-Harness (nur Auslieferung und Haken).
- (erledigt, Nachbesserung R28) Ohne JavaScript verdrängte das Abzeichen „k offen“ in der linken Gliederung den Titel
  („2.. 4 offen“); jetzt stehen Pfeile und „Bearbeiten“ ohne JavaScript in eigener Zeile, der Titel behält 8em und
  Abzeichen rutschen darunter. Sichtprüfung im Browser steht noch aus (Entwickler ohne Firefox).
- (keine Aufgabe) Schreibauftrag-Erkennung kennt nur feste Imperative am Anfang; für Fragen wie „Kannst du das
  schreiben?“ gibt es das Kästchen „als Auftrag“.
- **Gutachten R28 (19 Befunde: 0 hoch, 5 mittel, 14 niedrig).** Nachbesserung `1796940` behob die fünf wichtigsten
  (Statusvorschlag nur für Befunde des eigenen Abschnitts, Ähnlichkeit ohne Füllwörter mit Schwelle 0,8, Kappung bei
  2000 behält die neuesten und meldet `truncated`, Lektor-Ausnahme im Schreibauftrag, Gliederung ohne JavaScript); der
  PM-Abschluss behob Backlog/Fahrplan-Widersprüche, die Angaben „0107 frei“, den Satz zu neuen Abschnitten in
  Bedienung.md, die S1-Ausweichzweige in `round28_chat.rs`, vier Zusicherungen ohne Begründung in
  `round28_uebersicht.rs` und die ungerahmte Persona im Kopf einer Entscheidungszeile (jetzt entschärft). Offen:
- (erledigt R23) `post.move`/MCP `post_move` lässt die Befunde des Beitrags am alten Abschnitt (`finding.node_id`); sie
  zählen dort weiter, „übernommen“ prüft die Fassung des alten Abschnitts. Beim Verschieben mitziehen (ein Commit) oder
  dokumentieren.
- (erledigt R23, PM-Entscheidung) Sammelbefunde des Lektors (`summary_of` nicht leer) zählen heute selbst als offen, obwohl
  ihre Ursprünge schon zählen; nach einem RFC steigt „k offen“ doppelt. Entscheidung: Sammelbefunde zählen nicht in „k
  offen“ und werden „veraltet“, sobald alle Ursprünge entschieden sind.
- (erledigt R23) Lektor-Vorgabe: Regel 6 („Beginne mit „Was trägt:““) und Regel 8 („Im Chat zuerst auf die Nachricht
  eingehen“) widersprechen sich im Chat; Regel 6 auf Prüfung beschränken (Längentests `round15_*` beachten). Bei der
  Handprüfung mit Werk 3 auf den Ton achten.
- (erledigt R23) Matrix für Bildschirmleser: Zellen ohne Link (Teilsummen, „Gesamt“, „ohne Abschnitt“) tragen die Worte nur
  in `title`/`aria-label` eines `<span>`/`<td>`; sichtbar versteckten Text mitgeben.
- (erledigt R23) Liste „Offen, älter als n Tage“ (limit 200) zeigt über der Grenze die neuesten der alten Befunde ohne
  Hinweis auf die Kappung.
- (erledigt R23) „Zitieren“ setzt fremden Text an den Anfang der eigenen Nachricht; `@alle` oder `@handle` im Zitat rufen
  Personas auf. Zeilen mit „> “ von RFC- und Erwähnungserkennung ausnehmen.
- (erledigt R23) Schreibauftrag: `[B<id>]` aus dem Modelltext des Vorschlags erzeugen weiter Statusvorschläge (seit der
  Nachbesserung nur noch für Befunde des eigenen Abschnitts; der Mensch bestätigt). Erwägen, nur Verweise aus der
  Nachricht des Menschen zu nehmen.
- (erledigt R23) MCP `post_edit` erkennt Eigentum am selbst angegebenen `clientInfo.name` (wie `post_delete`); ein Client
  mit fremdem Namen kann Beiträge eines anderen MCP-Clients bearbeiten (Verlauf bleibt). Mit eindeutigen Handles lösen.
- (→ Nutzer) Kopieren im Chat: belegt ist nur die Ursache fürs Scrollen (`overflow: hidden` ohne Höhe); fürs Kopieren
  fand sich kein blockierender Handler, `user-select: text` ist Absicherung. Erst nach der Handprüfung (HANDOFF Runde
  28, Schritt 5) als erledigt werten.

## Nach Runde 27 offen (Integration 2026-11-09)

- (→ Nutzer) Handprüfung mit Werk 3 nach dem Einspielen (beim Einspielen läuft Migration 0045, **vorher
  Sicherung**): neun nummerierte Schritte mit erwarteten Ergebnissen in `HANDOFF.md` (Runde 27, „Handprüfung durch
  den Nutzer mit Werk 3“) – 1. Ausgangslage und Umschlag-PDF aufheben, 2. Titel ins dunkle obere Drittel, breiter,
  hell, 3. Autor nach unten, Klappentext verschieben und schmaler, 4. gleiche Lage in Umschlag- und Buch-PDF,
  5. Warnungen zu Beschnitt, Sicherheitsabstand, Kontrast und fehlender Schrift, 6. Buchangaben mit Anschriften,
  7. Impressum-Vorlage übernehmen und eigene `impressum(buch)` im Satz prüfen, 8. Gegenprobe ohne JavaScript,
  9. Anordnung zurücksetzen.
- (→ Nutzerfrage, Vorgabe aus R27: später) Eigene Schriftdateien hochladen (mit Lizenzhinweis); bis dahin nur Schriften,
  die Baruch eingebettet hat oder das System kennt (`cover.fonts`), eine fehlende fällt mit Hinweis zurück.
- (erledigt R20a) Umschlag als `cover-image` im EPUB (Stub `EPUB_COVER` entfernt).
- (erledigt R23) Ungültige Blöcke melden sich im Store als `StoreError::Asset(String)` (RPC -32602 mit Art und Feld); eine
  eigene Fehlerart `StoreError::CoverBlock` wäre sauberer.
- (keine Aufgabe, dokumentiert) Der Verlauf hinter einem Block ist symmetrisch (0 → 35 % → 0), nicht richtungsabhängig
  wie der alte Titelblock; die Kontrastprüfung zählt ihn pauschal als 30 % Abdunklung. SVG-Umschläge werden nicht
  auf Kontrast gemessen.
- (offen nach R24) `cover_checks` setzt den Umschlag noch einmal nur für die Maße (Satz aus dem Zwischenspeicher, Extras neu
  gelesen); ließe sich mit `layout_view` teilen.
- (erledigt R23, vom PM entschieden) Vorgabeblöcke rechnen aus dem Seitenmaß in pt; Koordinaten wie 20,999999 mm stehen so in
  `cover.layout` (die Oberfläche rundet). `default_blocks` rundet künftig auf 0,01 mm.
- (erledigt R20c statt R20a, Gutachten) Umschlag für die Druckerei (`wrap_source`) und Höhenmessung (`block_measure_source`) setzen nur
  `lang: "de"`; im Buch erben die Umschlagseiten alle Text-/Absatzregeln der Werkvorlage (`lang`, `hyphenate`,
  `tracking`, Show-Regeln). Bei solchen Vorlagen kann ein Klappentext im Blocksatz verschieden umbrechen.
- (erledigt R23, Gutachten) `book_page_mm` und `block_heights` rufen `baruch_render::compile` ohne die Zeitgrenze von 60 s.
- (erledigt R23, Gutachten) Kontrastprüfung entpackt das Umschlagbild bei jedem Aufruf ohne Pixelgrenze (Vorbild
  `assets.rs::thumbnail_bytes`, 20 000 px) und ohne Zwischenspeicher.
- (erledigt R23, Gutachten) `exchange_apply::insert_cover_block` übernimmt Blöcke ohne `check_cover_block` (Steuerzeichen im
  Text, Leerraum um Schriftnamen, NULL statt "" bei eigenem Text); ein solcher Block lässt sich danach im Editor erst
  nach Bereinigen ändern.
- (erledigt R23, Sichtprüfung) Namensschilder der Rückenblöcke überlagern sich auf schmalem Rücken gegenseitig und mit denen
  der Vorderseite (`.cover-block-name`).
- (keine Aufgabe, dokumentiert) `template_font` zählt ein `set text(font: …)` im Rumpf von `#show: rest => { … }` nicht
  mit (wie `sheet_style`); eine Schrift als Variable wird übergangen.
- (keine Aufgabe, beobachtet) Headless-Firefox zeigt im dunklen Thema helle Bildlaufleisten; Rahmen reichen von der
  Versalhöhe bis zur Grundlinie, Unterlängen ragen darunter. Das Sichtbild `ohne-js.png` ist älter als die Korrektur
  des doppelten Blocknamens (`druckpruefung-vorschau.png` zeigt den Endstand).
- (keine Aufgabe, beobachtet) `round13_quellen::an_idle_session_ends_with_delete_and_the_token_goes_along` war bei
  umschlag-satz einmal im Bündel rot (Sitzungsvorrat 2 statt 1), allein grün; im Integrationslauf grün.

## Nach Runde 18 offen (Integration 2026-11-08)

Zuordnung endgültig im PM-Abschluss (2026-11-08); dazu die niedrigen Befunde des Gutachtens, die nicht nachgebessert
wurden (die vier mittleren sind behoben, `d721b1c`; drei niedrige hat der PM selbst behoben: Modulkopf
`typeset.rs` zum Abbruch, „Werkzeugleiste“ in Bedienung/Architektur, `aria-label` an K und F).

- (→ Nutzer) Handprüfung mit Werk 3 nach dem Einspielen (beim Einspielen läuft Migration 0044, **vorher
  Sicherung**): acht nummerierte Schritte mit erwarteten Ergebnissen in `HANDOFF.md` (Runde 18, „Handprüfung durch
  den Nutzer mit Werk 3“) – 1. Werkvorlage von Werk 3 auf `#let szene`/`#let arbeit` prüfen (davon hängen die Knöpfe
  Szene/Arbeit ab), 2. Fußnote im Quelltext, 3. Listen/Begriff im Satzbild, 4. Insel per Doppelklick, 5. Einstellungen
  → Werk → Satzbild und „Alles aus der Vorlage“, 6. Bild an der Cursorstelle, 7. „Satz abbrechen“, 8. Kanten der
  Gedankenkarte.
- (offen nach R24) Typst lässt sich nicht hart abbrechen: Auswertung und Umbruch laufen zu Ende und halten so lange die Sperre
  (gemessen: 512 Seiten abgebrochen nach 200 ms → 252 ms statt 777 ms; eine Auswertungsschleife wird nicht
  unterbrochen). Ein harter Abbruch bräuchte einen eigenen Prozess je Lauf. PDF-Erzeugungen bricht `preview.cancel`
  nicht ab.
- (offen nach R24) Kapitelzuordnung einer geteilten Seite über `page_lines` zählt nur Text: Endet ein Kapitel auf der Seite nur
  mit Bild oder Linie, zählt die Seite nicht zu ihm; Text, den die Vorlage auf fremde Seiten zitiert, erkennt sie nicht.
- (erledigt in R27a, Austauschformat Unterversion 3 mit `<satzbild>`) ~~Satzbild-Einstellungen je Werk reisen nicht
  mit~~ (`docs/Austauschformat.md`).
- (offen nach R24, aus R21 verschoben) Begriffslisten bleiben ohne Schachtelung; gemischte Listenzeichen und springende Nummern bleiben eine Insel.
- (keine Aufgabe, dokumentiert) `emit` schreibt einen Verweis vor einem Zeichen, das zum Ziel gehören könnte, als
  `#ref(<…>)`: `@kai.` wird in einem **bearbeiteten** Absatz zu `#ref(<kai>).` (in Typst gleichbedeutend; unberührte
  Blöcke bleiben byte-gleich). `\` vor Leerraum gilt seit 18b als Umbruch wie in Typst; das ändert Blockgrenzen bei
  Texten mit `\` am Zeilenende (Rundreise bleibt byte-gleich).
- (keine Aufgabe, dokumentiert) Abweichungen vom Vertrag R18: `POST /werke/{id}/satz/abbrechen` leitet ohne laufenden
  Satz auf `?abgebrochen=0` (sonst `=1`); der Zwischenspeicher der Zusammenfassungen schlüsselt über Datenordner,
  Modell und SHA-256 des ganzen Aufrufs (nicht nur des Texts); `align` zerlegt ein Stück nur bei ungleich vielen
  Blöcken (gleich viele bleiben paarweise ersetzt).
- (erledigt R21b in `baruch-wysiwyg/src/html.rs`; Sichtprüfung Integration) Im Satzbild steht nach einem erzwungenen Umbruch `\` die nächste Zeile ein
  Leerzeichen eingerückt (das weiche Zeilenende hinter dem Umbruch wird als Leerzeichen gezeigt; Typst verschluckt es).
  Eine Bildinsel mit mehreren Bildern zeigt alle Bilder, als Unterschrift aber nur den ersten Pfad.
- (keine Aufgabe, Sichtprüfung) Im dunklen Thema zeichnet Firefox headless die Bildlaufleisten des Blatts und der
  Seiten Satz/Karte hell; ob ein echter Browser das auch tut, prüft der Nutzer (keine Änderung in R18 an
  `color-scheme`).
- (erledigt R19c, PM) Die fünf Punkte „→ R18 oder später“ aus „Nach Runde 17 offen“ (Zitat-Bindeglied, Lektor-Hinweis zur
  Wortlautprüfung, Rückabbildung der Bibelseite, Lemma-Probe, „Mal“-Zahlwort) waren nicht Teil von R18; sie gehen
  nach R19c (dort umgetragen).
- (erledigt R19c: Knöpfe „Bibelstelle“/„Literaturverweis“ aus der Vorlage; Gutachten R18, niedrig) Menüband: Knöpfe gibt es nur für `#szene`/`#arbeit` und die eingebauten Formate;
  „Bibelstelle“, `@cite` und andere `#let`-Funktionen der Vorlage fehlen (der Wunsch unter „WYSIWYG-Editor“ nannte
  sie).
- (erledigt R23; Gutachten R18, niedrig) `ribbon::defined_functions` liest Zeilen, nicht Typst: ein `#let szene` in einem
  Blockkommentar `/* … */` oder im Rumpf einer Funktion der Vorlage ergibt trotzdem den Knopf „Szene“ (der Satz
  scheitert dann); `#import "x.typ": szene` ergibt keinen.
- (erledigt R23; Gutachten R18, niedrig) `work.sheet_set` liest, ändert und schreibt die Satzbild-Einstellungen ohne
  gemeinsame Transaktion; zwei gleichzeitige Teiländerungen (eine nur `font`, eine nur `fill`) können sich
  überschreiben.
- (erledigt R23; Gutachten R18, niedrig) Menüband mit `role="toolbar"` ohne Pfeiltasten (nur Tab; in Bedienung.md
  vermerkt).
- (erledigt R21c: `insert_figure_at` setzt hinter das Konstrukt; Gutachten R18, niedrig) Bild einfügen mit `offset:` aus der Quelltext-Ansicht: Steht der Cursor in einem
  mehrzeiligen Inline-Konstrukt (`#footnote[eins⏎zwei]`, `#link(…)[…]`), nimmt `insert_figure_at` die nächste
  Zeilengrenze und setzt `#figure` in die Fußnote. Über das Satzbild tritt das nicht auf (dort immer Blockgrenze).
- (offen nach R24; Rest aus „Nach Runde 8/9 offen“, dort als erledigt geführt) Die Tests prüfen weiter nicht, ob Typst
  genauso zählt wie Baruch; der Test der Kopfzeile in schmalen Fenstern liest nur CSS-Regeln und misst keine Breite.
- (erledigt R22b; Rest aus „Nach Runde 8 offen“) Der Spiegel (`main.typ` mit `#include`) wird weiter nicht gesetzt, nur der
  Export.
- (→ Nutzer; Gutachten R18, niedrig) Gedankenkarte: Seit der Sichtprüfung liegen die Kantenbeschriftungen über den
  Knoten; bei nahen Einfällen verdecken sie deren Nummern und Kurztitel (`karte-kanten.png`). Ein Bild im dunklen
  Thema nach dem Fix fehlt; in Schritt 8 der Handprüfung mit ansehen, ob die Lesbarkeit so reicht.

## Nach Runde 25 offen (2026-11-08)

- (→ Nutzer) Handprüfung der Gedankenkarte an Werk 3, siehe „Beim Nutzer“ und `HANDOFF.md` (Runde 25); vorher
  Sicherung, beim Einspielen läuft Migration 0043.
- (→ Nutzerfrage 1 zu R25, Vorgabe später) Werkübergreifende Karte (alle Werke, Einfälle ohne Werk).
- (→ Nutzerfrage 2 zu R25, Vorgabe ja; erledigt R19b: „Gruppen benennen (Assistent)“, Stub entfernt) Ein Assistent benennt auf Wunsch Gruppen („Gruppe: Scham“) als Vorschlag
  in der Karte; heute gesperrter Knopf mit `// STUB: Cluster durch einen Assistenten benennen (R19)`.
- (erledigt R18a, integriert 2026-11-08) Kanten tragen ihre Art nur im Tooltip (`<title>`), gerichtete Arten haben keine Pfeilspitze; „führt
  weiter“, Baum und Bezug sind alle durchgezogen und unterscheiden sich nur durch Farbe und Stärke. Plan: Strichart
  **und** Beschriftung (Gutachten, niedrig).
- (erledigt R18a, integriert 2026-11-08) Die offene Seitenleiste des Widgets überdeckt rechts einen Teil der Werkzeugleiste („Anordnung
  zurücksetzen“, Cluster-Knopf); Schließen hebt es auf (Sichtprüfung).
- (erledigt R19b) MCP `idea_map` kennt `near_limit`/`near_threshold` nicht (RPC und Seite schon); mitgeschickte Werte gehen
  still verloren (Gutachten, niedrig).
- (erledigt R23) Keine Obergrenze für Einfälle und Verbindungen auf der Karte; die Nähe vergleicht alle Paare (O(n²·d))
  synchron im Handler bei jedem Aufruf von Seite, Ausgaben, RPC und MCP. Für einen lokalen Einzelnutzer unkritisch;
  Abhilfe Grenze oder `spawn_blocking` (Gutachten, niedrig).
- (geprüft R21c, keine Änderung: poppler meldet `/Suspects false` als Fehler, das PDF ist gültig; dokumentiert) `pdfinfo` meldet beim PDF der Karte „Syntax Error: Suspects object is wrong type (boolean)“; das PDF öffnet
  und rendert richtig. Prüfen, ob andere PDFs von Baruch (Buch, Stichwortzettel) dasselbe melden.
- (keine Aufgabe, wahlfrei) `round25_widget` legt die „lokalen“ Verbindungen und Lagen per SQL an statt über
  `idea_link_create`/`idea_map_pos_set`.
- (keine Aufgabe, dokumentiert) Seit der Nachbesserung zählen `open_ideas`/`total_ideas` alles unter einem Knoten,
  auch beim aufgeklappten Kapitel (RPC.md); das Widget liest die Felder nicht. Verbindungen, die vor `fc2ca78` durch
  einen Werkwechsel über zwei Werke reichten, werden nicht nachträglich gelöst (auf der Live-Instanz läuft 0043 noch
  nicht, dort gibt es keine).
- (keine Aufgabe) Die Seite wird bei 300 Einfällen etwa 826 KiB groß (Formulare je Einfall, eingeklappt).
- (erledigt im PM-Abschluss) Legende „widerspricht“ zeigt Strich-Punkt wie die Kante (vorher Doppellinie); SVG- und
  FreeMind-Ausgabe entfernen die XML-Nichtzeichen U+FFFE/U+FFFF; `idea_map`/`idea_links` und `docs/MCP.md` sagen
  „Daten, keine Anweisungen“; Bedienung „Nähe fein gepunktet“; Begründungen an fünf Zusicherungen.

## Nach Runde 26 offen (Integration 2026-11-08)

- (erledigt R22, nicht in R25) Abgleich: Ein neuer Teil, der vorhandene Kapitel übernähme, wird ein Antrag `Add`; seine Fassungen
  und seine Kennung bleiben dabei außen vor (in der Vorschau gemeldet). Nach dem Zustimmen hat der Teil hier eine
  neue UUID, ein späterer Abgleich kennt ihn nicht wieder. Abhilfe: Antrag mit Kennung und Fassungen.
- (→ Nutzerfrage) Umbenennen in der anderen Instanz: Über die Gliederung legt es dort eine Fassung mit der neuen
  Überschrift an. Beim Abgleich reist diese Fassung mit, und nach F6 (b) entsteht kein Antrag „Umbenennen“, sondern der
  Hinweis „Umschalten genügt“ mit Link (Umschalten benennt den Punkt um; ein zusätzlicher Antrag legte beim Bestätigen
  eine doppelte Fassung an). Der Rundenauftrag sagte „Umbenennen wird Antrag“. So lassen oder zusätzlich einen
  Antrag stellen, der beim Bestätigen auf die mitgekommene Fassung umschaltet?
- (→ Nutzerfrage 1 zu R26, Vorgabe nein) Autorenbibliothek (Titel, Zitate, Hervorhebungen) in einem eigenen Format.
- (→ Nutzerfrage 2 zu R26, Vorgabe später) Abgleich direkt zwischen zwei Instanzen über das Netz, mit denselben
  Elementen.
- (→ Nutzerfrage) „Als Kopie einlesen“: ein bekanntes Werk mit neuen Kennungen als zweites Werk anlegen (heute nur
  neu oder abgleichen).
- (→ Nutzerfrage, PM-Vorgabe: kein Hinweis) Fassungen mit genau demselben Text werden beim Abgleich still
  übersprungen (Nachbesserung, gegen doppelte Fassungen beim Hin und her), damit ein ruhiger Abgleich „nichts“
  meldet.
- (erledigt R25c: eigenes XSD `docs/schema/baruch-werk-1-u2.xsd` mit den Arten `verbindung`/`lage`, das XSD der
  Unterversion 1 bleibt Byte für Byte) XSD-Typ `GrabsteinArt` bleibt in Unterversion 1 geschlossen; R25 erweitert ihn im XSD der Unterversion 2
  (ein Leser der Unterversion 1 überspringt unbekannte Grabstein-Arten bei `version` > 1 schon, Austauschformat.md
  §6).
- (keine Aufgabe, dokumentiert) Lokal sind `<` und `>` im Instanznamen erlaubt; beim Einlesen auf der anderen
  Instanz werden sie zu ‹ › (für den Git-Autor des Spiegel-Commits), der Name ist dort nicht zeichengleich.
- (keine Aufgabe, dokumentiert, Gutachten) Der Abgleich kennt keine gemeinsame Basis: Eine **ältere** Datei
  erzeugt Anträge, eigene Gliederungsänderungen zurückzunehmen (nur Anträge; Austauschformat.md §4 „Ältere
  Datei“). Eine echte Drei-Wege-Basis gehört zum Netzabgleich (Nutzerfrage 2).
- (erledigt R23) Das Einlesen prüft die Typst-Ebene der Überschrift in einer Fassung nicht gegen die Art des Punkts (Text
  bleibt exakt): Eine handgeschriebene Datei mit `===` an einem Abschnitt wird angenommen, erst ein späteres
  Speichern verlangt die passende Ebene. Prüfen, ob `validate` das als Hinweis melden soll.
- (keine Aufgabe, dokumentiert) Über RPC höchstens etwa 24 MiB Datei (Base64 in `works::RPC_BODY_LIMIT`), über das
  Formular 128 MiB, über die Kommandozeile ebenso; MCP `exchange_export` höchstens 4 MiB, die RPC-Antwort ist nicht
  begrenzt.
- (keine Aufgabe) Einfälle eines gelöschten Werks bleiben ohne Werk und ohne Grabstein (ON DELETE SET NULL);
  `post_to_idea` hinterlässt einen Grabstein für den Beitrag. Der Schreiber schreibt immer `version="1"`, auch für
  ein aus einer höheren Unterversion gelesenes Dokument.
- (→ Nutzer) Handprüfung nach dem Einspielen (Migrationen 0041 und 0042 beim Start; vorher Sicherung): Einstellungen →
  Daten → Name der Instanz setzen; ein eigenes Werk über Werk ▾ → Austausch: Ausgeben herunterladen und auf einer
  zweiten Instanz (Wegwerf-Datenordner) einlesen; Vorschau und Ergebnis ansehen.

## Nach Runde 17 offen (Integration 2026-11-08)

- (erledigt R19c) Zitat-Zuordnung braucht ein Bindeglied ohne Wörter: „wie Lk 15,6 erzählt: „…““ ergibt „ohne
  Zitat“ (Handprüfung). Kurze Einleitungswörter („erzählt:“, „heißt es:“, „sagt:“) zwischen Stelle und Zitat zulassen?
- (erledigt Nachbesserung R17: ganze Wörter, „fehlt“ nur mit Ort wie „im Text“) Stellenvermerk L2 ist eine Wortliste: „fehlt“ trifft auch „Kritisch: Joh 3,16 – hier fehlt ein
  Übergang“ und hängt dann „(Stelle steht im Text: …)“ an. Engere Formen (Stelle als Subjekt der Behauptung) prüfen.
- (→ Nutzer) Die Behauptung „Lk 15,5 nicht belegt“ trat in der Handprüfung von R17 nicht wieder auf (zwei Anfragen);
  der Vermerk ist nur mit dem Nachbau getestet. Beim nächsten „Prüfen, ganze Predigt“ mit Lk 15,4–6 darauf achten.
- (→ Nutzer) Gemma ahmt „(nicht im Text belegt)“ mitten in einer Zeile nach (Handprüfung); Baruch wertet das nicht
  mehr als Stellenaussage. Ob der Vermerk dadurch in Antworten verwirrt, zeigt der Alltag.
- (erledigt R19c) Lektor-Antworten tragen weiter „Hinweis: Bibelstellen und Wortlaut prüft @schrift; hier
  ungeprüft.“, obwohl Baruch mit gewählter Bibel den Wortlaut selbst prüft – Hinweis an die Wortlautprüfung anpassen.
- (erledigt R19c) Bibelseite der Literatur: Bei einem Modul in anderer Zählung tragen die Verse dessen Nummern;
  ein Klick setzt sie als deutsche Stelle ins Feld (Rückabbildung je Vers fehlt, Architektur.md R17c).
- (keine Aufgabe, dokumentiert; Nachbesserung R17: ein Buch ohne Regel mit abweichenden Verszahlen ist ganz „passt
  nicht“, statt einzelne Kapitel gleicher Länge zu raten) Abbildung: 2. Kön 15 (German 39, KJV 38 Verse; das ganze
  Kapitel, auch 15,1, ist nicht abbildbar), Calvin, Catholic, Catholic2, DarbyFr,
  Segond und die Psalmen der griechisch-lateinischen Zählungen (LXX, Orthodox, Synodal, SynodalProt, Vulg) haben
  keine Regel und bleiben „Versifikation passt nicht“. Die Regeltabelle entstand mit einem Hilfsskript außerhalb des
  Repos; geprüft wird sie in `baruch-scripture/tests/mapping.rs`.
- (keine Aufgabe, dokumentiert) Parser-Faustregeln: „Joh 3,16, 12 Jünger“ liest Vers 12; ein Zitat derselben Art im
  Zitat („„…„…“…“) wird nicht erkannt; „usw.“/„etc.“ beenden im Stichwortzettel keinen ersten Satz.
- (erledigt R19c) Lemma-Probe der Lexikon-Stichwörter ohne Umlaut-Rückführung („Bäume“ → „Baum“) und Komposita;
  Lexika mit `StrongsPadding` erreichen „G26“ nur als „G0026“.
- (→ Nutzer) Einstellungen → Assistenten fragt die Modellliste nur noch auf Wunsch ab („Modellliste abfragen“, L4);
  die Warnung „Modell unbekannt“ erscheint erst danach. Soll die Liste nach dem Laden per JavaScript nachkommen?
- (keine Aufgabe) Nach falschem Schlüssel springt Einstellungen → Quellen nach oben (Meldung dort), nach Erfolg zu
  `#bibeln`.
- (erledigt R19c; Gutachten R17, niedrig) „Mal“ ist Zahlwort und Maleachi-Abkürzung: „Er las Joh 3,16 Mal 3,20
  vor.“ findet nur Mal 3,20, Joh 3,16 entfällt still (`count_word_follows` in `baruch-scripture` `scan`). Zahlwort nur
  gelten lassen, wenn danach keine Kapitel-/Versangabe folgt.
- (erledigt R23; Gutachten R17, niedrig) `GET /einstellungen/assistenten?modelle=abfragen` fragt die Gegenstelle ab; GET-Seiten
  laufen nicht über `guard::same_origin`, eine fremde Seite kann die Abfrage per `<img>` auslösen (nur lesend, Ergebnis
  nicht ausgeliefert). Als POST mit Origin-Prüfung oder `Sec-Fetch-Site` prüfen.
- (keine Aufgabe; Gutachten R17, niedrig) Regeltabelle `versification-map.tsv` entstand mit einem Hilfsskript außerhalb
  des Repos; wer weitere Zählungen (LXX-Psalmen, Catholic) belegt ergänzt, schreibt es neu. `tests/mapping.rs` prüft
  Stimmigkeit und (seit der Nachbesserung) jedes Buch ohne Regel.
- (Pflege) `vendor/sword-modules` weicht von sword-modules 0.1.0 ab (Obergrenze 64 MiB je Block in `src/compress.rs`):
  bei einem Update übertragen oder upstream vorschlagen; LZSS bleibt unbegrenzt (≈ 9-fach der gepackten Größe).
- (keine Aufgabe) Die diatheke-Gegenprobe in `baruch-sword/tests/libsword.rs` läuft nur mit installiertem libsword, in
  der CI wird sie übersprungen.
- (beobachten) `round13::the_sync_uploads_only_after_confirmation_replaces_changed_chapters_and_fetches_for_offline_use`
  scheiterte bei pruefanzeige einmal unter Last (danach grün); in der Integration grün.

## Nach Runde 16 offen (Integration 2026-11-08)

- (erledigt R17) Lektor-Befunde mit falscher Sachaussage bei richtigem Zitat: In der Handprüfung behauptete Gemma, „Lk 15,5“
  sei im Predigttext nicht belegt, obwohl der Vers in Lk 15,4–6 steht. Die Zitatprüfung sieht nur den Wortlaut;
  Stellenangaben in „Kritisch:“-Befunden ließen sich gegen den Bereich des Predigttexts prüfen.
- (→ Nutzer) Die Erkennung „Kritisch:“ hinter der Teil-Angabe ist mit den echten Antwortformen der Handprüfung
  getestet, aber nicht noch einmal gegen Eliezer (Grenze zwei Anfragen, beide verbraucht): beim nächsten „Prüfen,
  ganze Predigt“ darauf achten, dass kein Hinweis „nennt keinen mit „Kritisch:“ markierten Befund“ mehr erscheint.
- (erledigt R17) Fäden ohne Abschnitt prüfen keine „Kritisch:“-Zitate (kein Abschnittstext); die Zitatprüfung zählt Teile,
  die die Persona nur als Zusammenfassung sah, im Wortlaut mit (je Stück den gezeigten Text als Korpus nehmen).
- (erledigt R17) Einstellungen → Assistenten zeigt beim Lektor die eingestellte, nicht die wirksame Kontextlänge (Anzeige).
- (erledigt R17) Zeitwächter: Die Live-Zählung im Satzbild zählt den sichtbaren Text (innerText) und kann bei Typst-Inseln mit
  gesprochenem Text leicht vom Quelltext abweichen (beim Speichern rechnet der Server genau); bei einer nicht
  aktuellen Fassung ist die Gesamtzeit im Editorkopf eine Was-wäre-wenn-Rechnung (dokumentiert).
- (erledigt R17) Stichpunkt-Regel ist heuristisch: Abkürzungen wie „z. B.“ beenden den ersten Satz zu früh; `@`-Verweise
  nur nach Leerraum erkannt (dokumentiert).
- (erledigt R17) Antrag „neuer Teil“ in einer Predigt: Die Meldung sagt „5 vorhandene Kapitel wandern …“ und die Vorschau
  „Predigtteil I“ (ohne Punkt, Schema zeigt „I.“) – Wortwahl und Nummernform für Predigten angleichen.
- (erledigt R17) Lektor-Nachbesserung: Römische Nummern zählen jetzt auch in `strip_list_prefix`; ein Vorschlag verliert damit
  auch Zeilen „II. Kritisch: …“ (gewollt einheitlich, trifft im seltenen Fall eine Buchzeile). Ob die neuen Formen der
  Teil-Angabe (fett, Klammer, römisch) in echten Antworten vorkommen, zeigt die Handprüfung des Nutzers (HANDOFF).
- (keine Aufgabe) Stichwortzettel: Inhalt tiefer als 32 Ebenen `#name[…]` fehlt (Schutz gegen Stacküberlauf,
  dokumentiert in Bedienung).
- **Niedrige Befunde aus dem Gutachten zu Runde 16 (nicht behoben, PM-Abschluss 2026-11-08):**
  - (erledigt R17) „Lektor automatisch nach Kontext“: `effective_lector_of` fragt je aktiver Persona `state.model.context`
    nacheinander (bis 10 s je Anfrage bei abgelaufenem Zwischenspeicher), auch beim Laden von Seiten
    (Einstellungen → Assistenten, `persona.list`, Arbeitsseite); `context_cached` ohne Netz nehmen oder parallel fragen.
  - (erledigt R17) Zitatprüfung: `Frame::corpus` enthält die Stellenprüfung (`scripture_facts`, im System) nicht – ein Zitat
    des Bibelwortlauts daraus wird zu Unrecht „(nicht im Text belegt)“; umgekehrt enthält er die ganze Präambel
    (fremde Befunde, Chat-Beiträge, Web-/RAG-Treffer), sodass ein Zitat aus einem ungeprüften fremden Befund als belegt
    gilt. Korpus auf Text und Belege beschränken, Grenze in Architektur.md nennen.
  - (erledigt R17) Gedächtnis (Einfälle) steht ungerahmt und nicht entschärft im Prompt („Einfälle des Autors“), obwohl
    MCP-Clients Einfälle anlegen dürfen; wie Befunde rahmen (`framed_line`) und `OTHERS_RULE` auch dafür anhängen.
  - (erledigt R17b) Predigttext: Eine Stelle mit genau 400 Versen gilt als gekürzt (`verses.len() == MAX_PASSAGE_VERSES`);
    kein Test mit Typst-Sonderzeichen im Verstext auf dem Weg Predigttext → `#quote` → Wortzählung/Wortlautprüfung.
  - (erledigt R23) Anlegen einer Predigt mit Bibel ist nicht atomar (`create_work_with_text`, dann `set_work_bible`); scheitert
    der zweite Schritt, existiert das Werk ohne Spiegel-Sync und ein zweiter Versuch endet mit DuplicateTitle.
  - (erledigt R23) Die GET-Seiten des Stichwortzettels liegen wie die anderen lesenden Seiten nicht hinter `same_origin`
    (der Absturz durch tiefes Markup ist behoben).
  - (erledigt PM-Abschluss R16) MCP `outline_requests` mit `preview: true`: neue Felder (`kind_word`, `before_*`,
    `label`, `before_label`, `changed_label`) in `docs/MCP.md` und der Werkzeugbeschreibung nachgetragen;
    Zusicherungen ohne Begründung in `round16_predigt.rs` (Arbeitsseite, `/werke/neu`) begründet; Marken im Backlog.

## Nach Runde 15 offen (Integration 2026-11-08)

Aus den Berichten der drei Entwickler und der Integration; Gutachten nachgebessert (2026-11-08: gestückelter Kontext
gemessen und ehrlich abgelehnt, wenn auch komprimiert zu groß; Ollama meldet die wirksame Länge; keine Einbettung über
ein externes Eliezer; Lage „lokal/extern“ nach der Adresse mit Warnung; kein Einrahmen fremder Seiten).

- (→ Nutzer) Ollama: Die Vorgabe 4096 ohne `num_ctx` und das Feld `context_length` in `GET /api/ps` sind Annahmen
  nach der Ollama-Doku, nur gegen den Nachbau geprüft; mit einem echten Ollama einmal `model.context` ansehen.
- (erledigt R18b) Gestückelter Kontext: Ausgelassene Teile stehen je mit Titel und Hinweis im Aufruf; bei sehr vielen Teilen
  (etwa 20 überlappende Stücke bei 4096 Tokens) lehnt Baruch deshalb ehrlich ab. Aufeinanderfolgende ausgelassene
  Teile zu einer Zeile zusammenfassen („nicht mitgegeben: A; B; C“) würde Platz sparen.
- (offen nach R24) Lage „lokal/extern“: Ein Name im Intranet (z. B. `gpt.example.family`) gilt ohne DNS als nicht erkennbar lokal
  (Vorgabe extern, Warnung, wenn lokal gesetzt); eine Auflösung des Namens bei „Verbindung prüfen“ könnte das
  genauer machen.

- (→ Nutzer) **Handprüfung mit Werk 3, Kapitel I** (Anleitung in `HANDOFF.md`): Kontextlänge @lektor, „Prüfen“ für
  Abschnitt und ganzes Kapitel, Kontext testweise 8192 (Komprimiert-Hinweis), Vorschlag für das ganze Kapitel (ein
  Vorschlag je geändertem Abschnitt), Frage an @nathan; Verbindungen/Kontextabgleich gegen den echten Eliezer und
  ggf. Ollama (`/api/show`) sind nur gegen Nachbauten getestet; ob Eliezer ein noch nicht geladenes Modell mit der
  gemeldeten Länge lädt, sieht Baruch nicht.
- (erledigt R16a: Belegpflicht, „Kritisch:“ mit Zitat außerhalb des Kontexts bekommt „(nicht im Text belegt)“; falsche Sachaussage bei richtigem Zitat → „Nach Runde 16 offen“) Beobachtung der Handprüfung (Integrator, Gemma, erfundenes Kapitel): Eine Antwort auf „Passt das so?“ trug
  einen „Kritisch:“-Befund mit einer erfundenen Tatsache („laut Kapitel erst dreißig Jahre alt“ – im Text 30 Jahre
  als Wärter). Regel 3 („Urteile nur mit Beleg“) wird nicht geprüft; ein Zitatabgleich wie bei der Websuche wäre möglich.
- (erledigt R16a: Gliederung, SWORD-Belege, wirksame Länge) Fäden ohne Abschnitt haben einen eigenen, schlanken Kontext (keine RAG-, SWORD- oder Web-Belege, Budget aus
  `persona.context_length` statt `state.model.context`); bei Bedarf an den Kontextaufbau des Lektors anschließen.
- (erledigt R16a: Einstellung „Chat nur Abschnitt“) Der Chat bekommt bei jeder Antwort das ganze Kapitel – bei großen Kapiteln langsamer; Einstellung „Chat nur
  mit knappem Kapitelumfeld“ erwägen.
- (erledigt R16b) Die Persona-Liste der Arbeitsseite (Akkordeon „Assistenten“) zeigt Modell und Kontext, aber nicht
  „lokal/extern“ und keine Sperre bei fehlender Freigabe (nur Einstellungen → Assistenten).
- (erledigt R18b) Zusammenfassungen der Stückelung zwischenspeichern (an Modell und Text-Hash gebunden); heute fasst jeder
  komprimierte Auftrag dieselben Teile neu zusammen.
- (erledigt R18b) „an Persona geben“ auch auf `/quelle/multigpt` (Volltext ohne übernommenen Titel).
- (erledigt R18b) Retry im Strom bei gestückelten Aufträgen verwirft auch die Deltas früherer Stücke (der Endtext kommt mit
  `done` richtig).
- (erledigt R17b: Strong-Nummern aus Bibeln mit Auszeichnung, sonst Lemma-Probe) Lexikon-Stichwörter in `bible_research` sind eine Faustregel (großgeschriebene Wörter ab vier Buchstaben,
  Strong-Nummern, höchstens 5 Artikel je Lexikon); besser Lemma-Zuordnung bzw. Strong aus Bibeln mit Auszeichnung.
- (erledigt R17b) `bible_research` erkennt „Psalm 139, … Vers 23“ als ganzes Kapitel „Ps 139“ (Beleg und Quelle „Ps 139“);
  die Stellenprüfung meldet dann alle fehlenden Verse des Testmoduls. Fließtext-Verweise mit getrenntem Vers prüfen.
- (→ Nutzerfrage, nicht in R17 umgesetzt) Hervorheben an Bibeltexten geht nicht (eine Bibelstelle ist kein Titel der Bibliothek);
  bräuchte eine eigene Tabelle (Migration 0108 der Bibliothek ist dafür frei; 0107 belegt seit R28 `post_edit_index`).
- (erledigt R17c: Widget `commentaries.ts`, einheitlich „Kommentar · Modul“) Kein TypeScript-Widget für `data-widget="commentaries"` (ohne JavaScript klappt `<details>` von selbst).
  Überschriften der Treffergruppen „Bibel · Modul“ (klein, grau) und „Kommentar: Modul“ (fett) sehen verschieden aus.
- (erledigt R19c: Kasten „Bibeln“ serverseitig) Bibel-Kontext auf der Arbeitsseite (`view.rs`, `fragments/library.html`) nicht eingebaut; mit JavaScript
  lädt das Popup über `literature.render`, ohne JavaScript gibt es `/literatur`.
- (erledigt R23) RPC `model.connection_delete` und `model.connection_delete_secret` verlangen kein `confirm` (die Formulare
  verlangen das Pflicht-Kästchen).
- (erledigt R23) Zwischenspeicher der Modellliste und der Kontextlängen sind je Adresse, nicht je Art; zwei Verbindungen
  verschiedener Art auf derselben Adresse teilen sie (für OpenAI behoben: nie eine gemeldete Länge).
- (keine Aufgabe) `model.list` gleicht `personas` nur noch für die Personas der abgefragten Verbindung ab.
- (keine Aufgabe) Ehrlich vermerkt: Scheitert die „Kritisch:“-Nachforderung mit einem Modellfehler, bleibt die erste
  Antwort mit Vermerk stehen.
- (erledigt R16c: Art und Nummer, „vorher → nachher“) Vorschau des Antrags „neuer Teil“ (Nutzerfrage 12): Teil und erstes übernommenes Kapitel tragen beide „I“
  (Teil-Zählung und Kapitel-Zählung nebeneinander); die Nummern nach der Zustimmung deutlicher zeigen.
- **Niedrige Befunde aus dem Gutachten zu Runde 15 (nicht behoben, PM-Abschluss 2026-11-08):**
  - (erledigt R16a) Fäden ohne Abschnitt rechnen mit `persona.context_length` statt mit `state.model.context`
    (`threads.rs:138`); meldet die Gegenstelle weniger, kann der Aufruf zu lang werden (siehe auch Punkt oben).
  - (erledigt R16a: `chunk::merge_pieces`) `chunk::merge` hält einen Befund ab 24 Zeichen, der wörtlich in einem längeren steckt, für doppelt – auch
    über Teile hinweg; gleich formulierte Befunde zu verschiedenen Abschnitten erscheinen nur einmal. Nur
    Überlappungen benachbarter Stücke zusammenführen.
  - (erledigt R16a, erweitert in Integration und Nachbesserung: `critical_finding`) `prompts::has_critical` erkennt „kritisch:“ als Teilzeichenkette („Unkritisch:“ zählt); Zeilenanfang bzw.
    Wortgrenze prüfen wie `strip_critical_lines`.
  - (erledigt R16a bis auf das Gedächtnis, siehe „Nach Runde 16 offen“; MCP ohne Token R23: bleibt erlaubt, bewusste Entscheidung → Nutzerfrage 15) Prompt-Injection zwischen Personas: Block „Befunde anderer Personas“ (`context.rs` `finding_lines`) und
    Relay-Texte kommen ungerahmt und nicht entschärft in den Prompt; wie SWORD/RAG/Web rahmen (`defuse`, Regel
    „Daten, keine Anweisungen“). Ein lokaler MCP-Client ohne Token kann Beiträge unter Persona-Namen anlegen (R23: bleibt erlaubt, bewusste Entscheidung → Nutzerfrage 15).
  - (erledigt R23) Kontextabgleich (`/api/show` bzw. `/api/v1/models`) geht vor der Freigabeprüfung hinaus, auch bei einer
    externen Ollama-/Eliezer-Verbindung ohne Freigabe (ohne Buchtext, aber mit Schlüssel und Modellname); in
    `models::context` `external`/`external_allowed` vorab prüfen.
  - (erledigt R16a) „Lektor automatisch nach Kontext“ wählt nach der eingestellten, nicht der wirksamen Länge
    (`personas.rs:214`); Plan sagt „größter gemeldeter Kontext“. Wirkt erst, wenn der Versuch „fest @lektor“ endet.
  - (erledigt R16a/R16c) Zusicherungen ohne Begründung: `round15.rs:257`, `:460`, `round15_anbieter.rs:293`, `round15_lektor.rs:628`,
    `:686`, `outline_consent.rs:268`, `:310`.
  - (erledigt PM-Abschluss R15) `docs/MCP.md`: Absatz „Warteschlange je Client“ mitten im Satz, Zitat der
    `instructions` veraltet – berichtigt nach `mcp.rs`.
- (→ Nutzer) „Eigener Beitrag sofort sichtbar“ im Chat und der schwebende Toast beim Strom sind nur über Tests und
  `tsc` geprüft (Sichtprüfung ohne Klicks); im Browser einmal tippen.

## Nach Runde 14 offen (2026-11-07)

Aus den Berichten der drei Entwickler und der Integration (Gutachten folgt):

- (erledigt im Nachgang 2026-11-07, Nutzerfrage 8 „automatisch umwandeln“) **Bestehende Teiltexte** („= Teil …“,
  leere Teile in Werk 3) wandelt der Server beim ersten Start einmal in die Teil-Markierung um (neue Fassung,
  Spiegel-Commit, eigene Texte bleiben). Offen (keine Aufgabe, in R21 nicht angefasst; offen nach R24 bei Bedarf): Die Saat einer **neuen**
  Datenbank legt die Teile des Beispielwerks weiter mit leerem Text an (Merker gleich gesetzt); erst Umbenennen
  schreibt dort die Markierung.
- (keine Aufgabe: Entscheidung des Integrators, Einspruch jederzeit) **Projektion auch ohne Regel in der Vorlage:**
  Werke „aus der Werkvorlage“ ohne `#set heading(numbering: …)` gelten mit der Vorgabe „1.1“ als nummeriert; ihr Export
  stellt Punkte mit „Nummer aus“ darum als Markierung dar (das Beispielwerk sein Vorwort). Im Satz ändert das nichts,
  Export → Import behält so „ohne Nummer“.
- (erledigt R19a: `NO_EMBEDDER_OWN_DATA`) **Wegwerf-Server fragen beim Start Eliezer:** `rag::catch_up` bettet beim Start über die Verbindung
  „Eliezer“ ein, deren Vorgabe `http://127.0.0.1:1234` ist – auch mit eigenem `--data` (in der Sichtprüfung der Runde 14
  eine Anfrage ohne Schlüssel, 401, bevor die Adresse umgestellt war). Vorschlag: mit eigenem `--data` ohne gespeicherte
  Verbindung nicht einbetten oder die Adresse beim Start angeben lassen (`SKILLS.md` beschreibt den Umweg).
- (erledigt R21b: die Show-Regel wird beim Import `numbered = 0`) **Re-Import einer Überschrift mit eigenem Label und „Nummer aus“:** Sie kommt über die Show-Regel des
  Rückfalls mit `numbered = 1` zurück (Export byte-gleich, Baruch zählt sie aber wieder mit).
- (offen nach R24, aus R21b verschoben; Grenze in Architektur.md) **Funktionen der Werkvorlage** mit `p.at(i) ± k`, `slice()` oder Inhalt als Ergebnis wertet Baruch nicht je
  Ebene aus (Näherung wie bisher); die Predigt-Regel mit `context` erkennt es nur über Textgleichheit. Andere
  Typst-Zählzeichen als `1 a A i I *` (z. B. `α`, `一`) sind für Baruch Text (Architektur.md).
- (erledigt R18b) **Satzbild:** Ein Client kann einen `part`-Block in einen Kapiteltext schreiben (die Oberfläche bietet es
  nicht an; `outline.save` lehnt eine Teil-Markierung als erste Überschrift eines Kapitels ab, als weitere Überschrift
  beim Teilen kann sie einen Teil anlegen – Aktion des Menschen, aber nicht eigens getestet).
- (erledigt R18b) **Testgenerator `baruch-wysiwyg/tests/emit.rs`:** `canonical` entfernt Leerraum am Anfang einer Überschrift
  nicht, wenn er in einer Hervorhebung steht (mit anderer Saat z. B. Fall 1168); Lücke im Generator, nicht im Produkt.
- (erledigt R23) **Vorschau eines veralteten Antrags** kommt als `StoreError::Heading("Veraltet: …")` statt `Refused` mit Grund
  (`Refused` nimmt nur festen Text; über RPC trotzdem -32000).
- (erledigt R23) **`/gliederung`** rechnet je offenem Antrag eine Vorschau (Schreib-Transaktion mit Zurückrollen); bei vielen
  Anträgen langsam.
- (erledigt R18a) **Rechter Rand der Seiten „Satz“ und „Gliederungsschema“:** ein heller Streifen rechts neben dem Inhalt
  (kosmetisch, nicht aus Runde 14; Bildschirmfotos der Sichtprüfung).
- (→ Nutzer) **Markdown-Überschriften im Browser prüfen** (Nachgang 2026-11-07): `# Titel` + Enter im Quelltext und
  im Satzbild, Cursor danach; nur Server-Tests und `tsc`. Offen (erledigt R18b): `revision.update` (Fassung ändern) wandelt
  nicht um, und die Live-Prüfung in `markdown.ts` ist eine vorsichtige Näherung an den Scanner des Servers.
- (→ Nutzer) **Im Browser prüfen:** Kontextmenü „Neuer Teil davor/danach“ im Baum und Ziehen von Teilen – nur per
  Server-Test, `tsc` und Bildschirmfoto geprüft (Bilder der Sichtprüfung dauerhaft unter `.claude/sicht/r14/`, nicht
  im Repo).

Aus dem Gutachten (13 Befunde; behoben in `bba23db`: Titel mit Code wie `#emph[…]` werden nicht mehr zur Markierung
verfälscht, sondern fallen auf die Show-Regel zurück; „Teil ohne Nummer“ wird abgelehnt; `* k` in Funktionen der
Werkvorlage höchstens 16; `docs/Architektur.md` „Export und Spiegel“ auf dem Stand der Projektion. Vom PM: veralteter
Satz „aus der Werkvorlage byte-gleich“ unter „Nach Runde 6 offen“ berichtigt, Begründung an der Zusicherung in
`round14.rs`, Test für Teil am Werkende und zwei Teile hintereinander, Bilder der Sichtprüfung nach `.claude/sicht/r14/`):

- (erledigt 2026-11-08, Nutzerfrage 12 „aktive Zustimmung (Popup)“) **Neuer Teil über MCP übernimmt vorhandene
  Kapitel** – jetzt ein Antrag mit Zustimmung im Dialog (`crate::consent`, `docs/MCP.md`).
- (erledigt R21b: Rückfall-Show-Regel auch bei ausgeschalteter Nummerierung, getestet) **„Nummer aus“ geht bei „Ohne Nummern“ verloren:** Die Projektion läuft nur, wenn die wirksame Nummerierung
  nicht aus ist (`snapshot::project`); ein auf „ohne Nummer“ gestellter `=`-Abschnitt kommt nach Export → Import unter
  „Ohne Nummern“ bzw. einer Vorlage mit `numbering: none` nummeriert zurück (und umgekehrt). In `docs/Architektur.md`
  beschrieben, ein Test fehlt.
- (keine Aufgabe; in R21b nur dokumentiert, keine Migration) **Teile mit `numbered = 0` aus der Zeit vor `bba23db`:** möglich nur in Runde 14 über RPC oder bestätigten
  Antrag; zurückstellen geht nur über RPC `outline.set_numbered` (kein Knopf, weil die Oberfläche ihn für Teile
  ausblendet). Keine Migration.
- (erledigt R22, aus R21b verschoben) **Titel mit Code an einem gleichlautenden nummerierten Knoten** bekommt keine Show-Regel (dieselbe Grenze wie
  beim eigenen Label).
- (erledigt R23) **Vorschau-Kosten auf `/gliederung`:** offene Anträge haben keine Obergrenze je Client; ein MCP-Client kann
  viele verschiedene Anträge stellen, jede Seite hält dann N-mal kurz die Schreibsperre (Vorschau nur auf Anforderung
  oder Obergrenze je Client).

## Nach Runde 13 offen (2026-11-07)

Aus der Nachbesserung nach dem Gutachten der Runde 13:

- (→ Nutzerfrage 11) **Abgleich: Kommentare/Befunde nach MultiGPT.** `docs/Plan.md` („RAG-Abgleich mit MultiGPT“) sieht je Werk
  „aktuelle Revisionen und Kommentare/Befunde“ als Dokumente vor; hochgeladen werden bisher nur die aktuellen
  Fassungen je Kapitel (`sync/upload.rs`). Fehlt für das Einsatzbild Bibelschule („Stand und Befunde“). Offen ist
  vorher die Nutzerfrage, welche Beiträge hinausdürfen (nur Befunde der Personas oder auch Chat-Beiträge des
  Menschen) und ob dafür ein eigener Schalter nötig ist – das vergrößert, was das Haus verlässt, darum nicht still
  eingeschaltet. In Bedienung.md und Architektur.md als Abweichung vermerkt.
- (erledigt R19a) **Verbindungsprüfung ohne `DELETE`:** Die Prüfung nutzt keinen Sitzungsvorrat und beendet ihre MCP-Sitzung
  nicht (Rest von R13a; `round9_connections.rs` zählt jede Anfrage und müsste das `DELETE` erwarten).
- (erledigt R15c) **Markieren im Volltext per Kontextmenü** (`literature.ts`) fehlt; ohne JavaScript trägt der Mensch Anfang und
  Ende als Zahlen ein (`data-offset` des Teils).
- (→ Nutzer) **Annahmen über das echte MultiGPT** (Werkzeugnamen, Parameter, Antwortformen, Seitenzählung, `site:`,
  ob `read_document` seitenweise liefert, ob `upload_document` eine ID nennt): `docs/Architektur.md`, Abschnitte
  „Annahmen“ und „Abgleich“. Ungeprüft, kein Zugriff im Team.
- **Niedrige Befunde aus dem Gutachten zu Runde 13** (19 Befunde: 0 hoch, 7 mittel, 12 niedrig; die sieben mittleren
  in `1251a5b` behoben bzw. – Kommentare/Befunde – als Nutzerfrage 11 vermerkt; im PM-Abschluss behoben: Doc-Kommentar
  von `collection_for`, „1 Dokument bei MultiGPT“, sieben Zusicherungen mit Begründung, Hinweis am Schalter der
  Websuche, was als Suchanfrage hinausgeht; Plan/Backlog/Fahrplan nachgezogen):
  - (erledigt R19a) Nennt `upload_document` keine Dokument-ID, gilt das Hochladen als gescheitert, obwohl das Dokument bei
    MultiGPT liegt; jeder Lauf lädt es erneut hoch (`sync/upload.rs` `put`). Mindestens Hinweis bzw. Sperrvermerk.
  - (erledigt R19a) Jeder Schreibfehler blockiert den Rest des Laufs, auch `isError` für ein einzelnes Dokument (etwa leere
    Kapitel); das Hochladen hängt dann dauerhaft am selben Kapitel. Nur Rechte- und Transportfehler blockieren lassen.
  - (erledigt R19a) Vollständigkeit der Liste beim Holen hängt an Annahmen (Seitenzählung `page` ab 1, `has_more`); kappt der
    Server `limit` still, löscht der Abgleich lokale Kopien, die es noch gibt (`sync/download.rs` `list`).
  - (erledigt R19a) Testlücken: keine Einträge der Art `remote` im Index nach Abwählen einer Sammlung (`rag.search`), Abbruch
    zwischen Hochladen und Zustand, Umbenennen eines Werks bei eingeschaltetem Hochladen.
  - (erledigt R19c) Keine Unicode-Normalform (NFC) in `check_range` (`baruch-library/src/highlights.rs`) und im Zitatabgleich
    der Websuche (`websearch/verify.rs`): zerlegte Umlaute (z. B. aus PDFs) ergeben „nicht gefunden“/„(nicht belegt)“.
  - (erledigt R23) Adressen aus `web_search` gehen ungefiltert an `fetch_url` (nur http/https geprüft); private, Loopback- und
    Link-Local-Ziele werden nicht abgewiesen, MultiGPT ruft sie im Intranet ab (`websearch/search.rs`). Ob MultiGPT
    selbst filtert, ist unbekannt.
- (entschieden R23: bleibt im Speicher, Begründung in docs/MCP.md „Runde 23 – Härtung“) **MCP `source_read`:** Die Treffer, die ein Dokument freigeben, merkt sich der Prozess (höchstens 4096,
  nur im Speicher); nach einem Neustart braucht ein MCP-Client erst wieder `rag_search`. Bei Bedarf dauerhaft
  speichern.

## Nach Runde 12 offen (2026-11-07)

Aus der Integration der Runde 12 „Verbindungen nutzen, RAG“ (der umgesetzte Abschnitt „Einstellungen:
Verbindungen“ steht unter „Erledigt“):

- (erledigt in Runde 13) **Runde 13 „Quellen-Dokumente und Websuche“:** Volltext einer Quelle über MultiGPT `read_document` in der
  Dokumentansicht (STUB `literature.rs`/`source.html`, `MultigptWay::read_full`), Prüfung eines Bereichs ohne Zitat
  gegen den Volltext (STUB `baruch-library` `highlights.rs`), Websuche für `@belege` (Abschnitt „Werkzeuge der
  Personas“, STUB `connections/sources.rs`), Abgleich Baruch ↔ MultiGPT (Menü „Abgleichen“, STUB `base.html`).
- **Erledigt (R13.0, 2026-11-07):** Server-Integrationstests als eine Binärdatei (`tests/main.rs`, `autotests =
  false`), pgvector-Tests teilen einen Container (je Test eine eigene Datenbank). Nach `cargo clean` und vollem
  Lauf: `target/` 38 GB → 9,0 GB, Testbinärdateien 146 → 86, Dauer 1081 s → 811 s, 1195 → 1196 Tests bestanden.
  LanceDB-Feature war nicht nötig. Ursprünglicher Eintrag:
  ~~(→ R13.0) **Größe von `target/` – vor dem nächsten parallelen Arbeiten (erste Aufgabe der Runde 13):** Jede
  Test-Binärdatei des Servers linkt LanceDB mit (≈ 500 MB statt ≈ 145 MB); ein Worktree braucht nach dem vollen
  Lauf ≈ 40 GB statt der nach `cfad08b` genannten 14 GB (nachgemessen im Hauptordner: `du -sh target` 40G). Bei
  drei bis vier parallelen Worktrees sind das 120–160 GB auf /home. Abhilfe: Server-Tests zu wenigen
  Binärdateien bündeln (`tests/main.rs` mit Modulen) oder LanceDB hinter ein Cargo-Feature, das nur die RAG-Tests
  einschalten. Bis dahin höchstens zwei parallele Worktrees und nach jeder Runde aufräumen.~~
- (→ Nutzerfrage 1, danach eigene Aufgabe; in R19a nicht umgesetzt) **RAG-Inhalt nach der Entscheidung (unten unter „Erledigt“) unvollständig:** Der LanceDB-Index enthält je
  Abschnitt nur die **aktuelle Fassung**, dazu Einfälle, Hervorhebungen (ohne abgelehnte) und die Bibliothek.
  Es fehlen noch **ältere Revisionen** und **Befunde/Beiträge** der Personas (Plan: „Texte, Revisionen und
  Kommentare“; für Befunde Plan „später im (Graph-)RAG“). Offen: welche Fassungen (alle oder nur benannte),
  Art `revision`/`post` mit Herkunft, und dass Belege für Personas den eigenen Abschnitt in jeder Fassung
  ausschließen.
- (erledigt R24 `ci`: `ci.yml` und `release.yml` installieren `protobuf-compiler` bzw. `protobuf`/`protoc` je Runner; kein Ablauf ist je gelaufen) **CI:** `lance` braucht `protoc` beim Bauen; in der CI nachrüsten, falls dort keins liegt.
- (erledigt R13a: `connections::multigpt::Session` mit Vorrat und Zwischenspeicher) **MultiGPT:** je Suche eine eigene MCP-Sitzung (4–6 Anfragen); Werkzeugliste zwischenspeichern.
- (erledigt R13a/c: `source.adopt` und Übernahmen rufen `rag::notify`) **Bibliothekstitel** melden sich nicht beim Index; die Suche frischt den gemeinsamen Bereich höchstens alle
  30 s auf (heute ändert nur die Saat Titel).
- (erledigt R19c) **`idea.search` mit `mode: "rag"`** meldet einen Ausfall der Bedeutungssuche als -32000 (wie die übrigen
  Ablehnungen); die Seite `/einfaelle` zeigt den Grund.
- (erledigt R13a) **`round10_models.rs`** erwartet bei MultiGPT einen `data-stub`-Hinweis (offener Teil Runde 13); entfällt er,
  muss der Test mit.
- (erledigt R19b) **Einbettung ohne Aufgaben-Präfix:** nomic-embed-text empfiehlt `search_query: ` / `search_document: `; Baruch
  bettet roh ein (damit LanceDB und pgvector die Anfrage teilen). In der Handprüfung stimmte Rang 1 immer, der
  Abstand zum zweiten war aber teils knapp (0,671 zu 0,666). Präfixe je Weg einstellbar machen (pgvector: wie die
  Daten eingebettet sind).
- (erledigt R19c) **Keine Untergrenze der Ähnlichkeit:** Literatur und Einfälle zeigen auch Treffer mit 0–4 %; eine Schwelle oder
  ein Hinweis „schwach“ wäre hilfreich.
- ~~(→ R13.0) Der pgvector-Container braucht im Test 30–45 s zum Start; alle pgvector-Szenarien laufen je Testdatei gegen
  einen Container.~~ **Erledigt (R13.0):** gleichzeitig laufende pgvector-Tests teilen sich einen Container
  (`tests/pgvector_support`, je Test eine Datenbank `rag_<n>`).
- **Aus dem Gutachten zu Runde 12, nicht behoben (niedrig):** (18 Befunde: 0 hoch, 6 mittel, 12 niedrig; die
  sechs mittleren sind in `77330aa` nachgebessert bzw. hier eingeplant, Fahrplan im PM-Abschluss nachgezogen)
  - (erledigt R13a: eigenes Zeitlimit und Budget je Nachtrag; `DELETE` für Sitzungen im Vorrat, Prüfung erledigt R19a) MultiGPT: Nachträge (`document_info` bis 5×, `read_document` bis 3×) laufen im selben 20-s-Zeitlimit des Wegs;
    ist die Gegenstelle dabei langsam, gehen schon gefundene Treffer verloren („ausgefallen“). Die MCP-Sitzung wird
    nie mit HTTP `DELETE` beendet.
  - (erledigt R19a) Index-Worker: Jede Suche reiht `Job::Prune` ein und damit alle liegengebliebenen Aufträge erneut; bei
    ausgefallenem Eliezer startet so jede Suche einen neuen Versuchszyklus (bei Zeitlimit 120 s je Stapel), und
    `rag.status` zeigt zwischendurch „kein Problem“.
  - (erledigt R19a) `rag.reindex` wartet auf die Schreibsperre, die der Worker einen ganzen Auftrag lang hält – „Index neu
    aufbauen“ kann bei hängendem Eliezer minutenlang auf die Antwort warten.
  - (erledigt R19a) `Index::replace` löscht nach Kennung über alle Werke; `node.id` ohne `AUTOINCREMENT` kann nach einem Löschen
    neu vergeben werden, dann kann der Abgleich von Werk A frische Zeilen von Werk B entfernen (aus dem Code
    abgeleitet, nicht nachgestellt). Abhilfe: Löschen auf den Bereich beschränken.
  - (erledigt R19a) Nach `work.delete` werden die Einfälle des Werks „ohne Werk“, der gemeinsame Bereich zieht sie aber erst mit
    der nächsten Suche nach (höchstens 30 s); bis dahin fehlen sie in der Bedeutungssuche.
  - (erledigt R19b) pgvector: nur der Text ist gekürzt; Dokument, Stelle, Titel, URL, Sammlung (und bis zu 1000 Sammlungsnamen in
    der Prüfung) kommen ungekürzt – `left(…, n)` in SQL wie beim Text.
  - (erledigt R19b) pgvector: das Schema aus `information_schema` wird ohne Prüfung gequotet (`quoted()` prüft nur per
    `debug_assert!`); Anführungszeichen verdoppeln oder ablehnen.
  - (erledigt R19b) pgvector: Vorgabe `tls: prefer` – seit Runde 12 geht das Passwort bei jeder Bedeutungssuche mit; Vorgabe
    `require`/`verify-full` oder Hinweis auf der Seite bei Passwort mit `prefer`/`disable`.
  - Behoben im PM-Abschluss: Nutzungstext von Eliezer nennt die Einbettungen; Bedienung zur Suche der Arbeitsseite
    und Backlog zur MultiGPT-Prüfung (Konto und Budget meldet MultiGPT nicht) richtiggestellt.

## Nach Runde 11 offen (2026-11-07)

Aus dem Gutachten (17 Befunde: 0 hoch, 7 mittel, 10 niedrig). Die mittleren sind nachgebessert (Zeitlimit zählt
die gemessene Zeit, leere Innenseite des Umschlags hält Seitenzahl und Parität des ersten Kapitels, Kapitelbild
folgt Typsts `counter(heading)`, höchstens zwei Vorschaubilder zugleich, Knopf „Farbe der Palette“, Doku der
Farben, Fahrplan). Offen, niedrig:

- (erledigt R20c) **Umschlagbild gelöscht:** `ON DELETE SET NULL` entfernt den Umschlag ohne Spiegel-Commit „Umschlag entfernt“;
  beim Löschen eines Bilds, das Umschlag ist, den Umschlag erst über `work.set_cover` entfernen.
- (erledigt R20c) **ISBN:** `isbn13_check` prüft das Präfix 978/979 nicht; „isbn-13 …“ in Kleinschrift wird falsch abgeschnitten.
  Eine nur anders geschriebene ISBN (Bindestriche) gilt als unverändert und wird nicht gespeichert.
- (erledigt R18c) **Bild einfügen:** Der Pfad wird vor dem Speichern der Fassung vergeben; scheitert das Speichern, bleibt er ohne
  Spiegel-Commit. Einfügen geht nur am Ende des Abschnitts, nicht an der Cursor-Stelle.
- (erledigt R21c: je Client und Werk in 24 h 30 Aufrufe, 64 MiB, 10 offene Anträge; Inhalt entschiedener Anträge nach 30 Tagen entfernt) **MCP ohne Kontingent:** `asset_add`/`asset_replace` beliebig oft (je bis 8 MiB); Anträge zum Ersetzen behalten
  ihren Inhalt für immer. Kontingent je Client und Aufräumen entschiedener Anträge.
- (erledigt R21c) **Tests fehlen** für die Kopfzeilen (`Content-Security-Policy: sandbox`, `nosniff`) der Antragsdatei
  `GET /werke/{id}/bilder/antraege/{nr}/datei`.
- (erledigt R20c, Vorgabe → Nutzerfrage 13) **Rückenbreite:** Vorgabe 0,1 mm je *Seite* (= 0,2 mm je Blatt) ist eher dick; Druckereien nennen die Dicke je
  Blatt. Feld und Beschriftung klären („Rücken 0,5 mm (7 Seiten, ohne 2 Umschlagseiten)“ ist missverständlich).
- (→ Nutzer; zusammengefasst mit „Bilder im Browser des Nutzers ansehen“ (Runde 6)) **Vorschaubilder** laden erst beim Hinscrollen; Werkübersicht im normalen Browser prüfen.
- (erledigt R20c) Ein bestätigter Umschlag-Antrag erzeugt zwei Commits („Umschlag gesetzt“, „Bildantrag n bestätigt“).
- (erledigt R21c: eigenes Feld `LibraryView.images`) `ImageView` reist unter einem reservierten Schlüssel in `LibraryView.image_alts` statt in einem eigenen Feld.
- (keine Aufgabe: harmlos, Darstellung und Maße stimmen) poppler meldet bei Typst-PDFs „Suspects object is wrong type (boolean)“; Darstellung und Maße stimmen.

## Nach Runde 10 offen (2026-11-06)

- (erledigt R15b) **Musterpersonas und freie Anbieter (Entscheidung des Nutzers 2026-11-06, berichtigt):** Die Musterpersonas
  in `sample.rs` behalten ihre Modellnamen. In die Einstellungen gehören nur die **LLM-Einstellungen**
  (Anbieter/Verbindung, Adresse, Schlüssel im Schlüsselbund, Kontextlänge), nicht fest im Code. Der **Anbieter
  ist frei** wählbar: je Persona eine Verbindung (Eliezer, beliebiger OpenAI-kompatibler Dienst, Ollama, …),
  nicht nur Eliezer; die Modellliste kommt aus `GET /v1/models` der gewählten Verbindung. Externe Anbieter
  bleiben je Persona sichtbar als „extern“ markiert und brauchen die ausdrückliche Freigabe des Nutzers.
- ~~Verszählung Joel/Maleachi~~ **erledigt 2026-11-06 (Nachbesserung):** Die eBible-Tabelle war durchgehend
  englisch (KJV, 31102 Verse), nicht nur bei Joel/Maleachi. Ersetzt durch die deutsche Zählung „German“ (SWORD,
  31172 Verse). **Lizenz entschieden (Nutzer 2026-11-06):** Die Verszahlen aus SWORD `canon_german.h` (GPLv2)
  werden als reine Zahlen (Tatsachen) übernommen; damit fangen wir an. Erledigt R17a: Abbildung englischer Angaben (z. B. aus englischer Literatur) auf die deutsche Zählung.
- (erledigt R15a) **Lektor nach Plan:** Laut Plan bekommt der Lektor das ganze Kapitel bzw. die ganze Predigt, sammelt die
  Befunde der anderen Personas, und „Prüfen“/„Vorschlag“ gibt es auch für ein Kapitel. Umgesetzt ist nur der
  einzelne Abschnitt mit Kapitelumfeld (Nachbarn je 300 Zeichen) und Einfällen; der Rollen-Prompt sagt das jetzt
  ehrlich („nicht das ganze Werk“, Verweise nur auf Stellen im Kontext).
- (erledigt R15b) **Kontextlänge gegen Eliezer prüfen:** Die Kontextlänge einer Persona muss zu der geladenen passen (Gemma 32768,
  Vorgabe des Beispiels seit der Nachbesserung 32768 statt 65536); Einstellungsseite und `persona.update` gleichen
  sie nicht ab. Abgleich mit der von Eliezer gemeldeten Kontextlänge bzw. Warnung bei Abweichung.
- (erledigt R17b) Fließtext: „Johannes 3 Mal“ wird als Joh 3 erkannt; Verslisten mit Komma („Joh 3,16, 18“) liest der Parser nicht.
  (Ein nummeriertes Buch nach Komma – „Joh 3, 1. Joh 4“ – ist seit der Nachbesserung eine neue Stelle.)
- (erledigt R15c) **Strom im Chat:** Während die Persona schreibt, zeigt der Chat den eigenen Beitrag des Menschen noch nicht (er
  ist gespeichert und erscheint nach `done`). Ohne JavaScript wartet die Seite die ganze Antwort ab (kein
  Fortschritt sichtbar); im Strom keine eigene Anzeige, wenn eine verbotene Einleitung neu angefordert wird.
- (erledigt R23; zusammengefasst mit „Verschiedene Token-Namen“) **Client-Name aus dem Token-Namen:** Umlaute werden zu `-` („Laptop Küche“ → `@mcp:laptop-k-che`); Regel des
  Client-Handles aus Runde 8. Umlaute umschreiben (ü → ue) oder Namen beim Erzeugen vorschlagen.
- (erledigt R15c) Formular „Einfall verarbeiten“ ohne Persona-Auswahl (immer der Lektor; über RPC `persona`).
- (erledigt R15c) Chat: Scheitert eine Persona, zeigt die Seite ohne JavaScript 422 mit Text statt der Seite mit Hinweis.
- (erledigt R15b) Modellliste wird bei einem Fehler nicht aus dem abgelaufenen Zwischenspeicher bedient; letzter Kurztest nur im
  Speicher des Servers; `Endpoint` baut je Aufruf einen eigenen HTTP-Client.
- (→ Nutzer; R15b gleicht die gemeldete Länge ab, der Rest steht unter „Nach Runde 15 offen“) Ob Eliezer ein nicht geladenes Modell mit der richtigen Kontextlänge (32768) lädt, sieht Baruch nicht (es fragt
  nie nach dem Ladezustand) – die Länge muss in Eliezer eingestellt sein.
- (erledigt R24 `webseite` als Doku: `docs/Betrieb.md`, `--allow-host`; der Server selbst spricht weiter kein TLS, an keinem echten Proxy geprüft) Kein TLS im Server: Zugang von außen nur im eigenen Netz oder über Tunnel/Reverse-Proxy (dann Host-Regel
  beachten).
- ~~Wortlaut der Bibelzitate~~ **erledigt 2026-11-06 (Zweig `team/sword`):** lokale SWORD-Module (Einstellungen →
  Quellen → Bibeln (SWORD)), Wortlautprüfung je Stelle, siehe „SWORD-Bibeln“ unten.
- (Rest „mit dem echten Buch prüfen“ → Nutzer: Handprüfung Werk 3, „Nach Runde 15 offen“) **Handprüfung mit echtem Eliezer** (Integrator, 2026-11-06, erfundenes „Senfkornbuch“, alle sechs Personas auf
  Gemma, `BARUCH_MAX_TOKENS=300`): Gemma war geladen; Kurztest 60 ms, je Frage 0,6–1,3 s, Antworten 190–480
  Zeichen, Deutsch sauber, keine verbotene Einleitung, Lektor/Exeget/Nathan mit „Kritisch:“, Nathan ohne
  Freigabe, Hörer in Ich-Perspektive, Schrift und Belege nur Zuordnung nach der Stellenprüfung (Ps 151,1 als
  ungültig erkannt, „Wortlaut ungeprüft“), andere mit dem Hinweis auf @schrift. **Befund behoben:** Ein Vorschlag
  des Lektors trug seine „Kritisch:“-Anmerkung im neuen Abschnittstext (Regel 6 galt auch beim Vorschlag) – Regel
  und Auftrag nehmen den Vorschlag jetzt aus; zweiter Versuch sauber. Noch mit dem echten Buch (Werk 3, Kapitel I)
  und längeren Antworten zu prüfen (Zeiten bei vollem Kontext).
- (Befund „Regel 6 nicht erzwungen“ erledigt R15a: „Kritisch:“ wird geprüft und nachgefordert) **Handprüfung der Nachbesserung** (2026-11-06, echter Eliezer, eigener Server `--secrets keyring`, Wegwerf-Ordner,
  erfundenes „Wabenbuch“, nur `google/gemma-4-26b-a4b` mit 32768, nacheinander, `BARUCH_MAX_TOKENS=40`):
  `model.list` live (16 Modelle); Eliezer meldet an der Grenze `finish_reason` „length“ → Prüfen des Lektors als
  Beitrag mit „… (Antwort abgeschnitten: Längengrenze erreicht)“, `cut: true` (0,65 s); Vorschlag abgelehnt
  (-32000), keine neue Fassung; kurze Frage ohne Abbruch `cut: false`. `scripture.check`: „Ps 51,21“, „Joel 4,1“,
  „3. Joh 15“ vorhanden, „Mal 4,1“ → „Maleachi hat nur 3 Kapitel“. Beobachtet: Bei einer Ein-Satz-Frage ließ der
  Lektor den Pflichtbefund „Kritisch:“ weg (Regel 6 wird nicht erzwungen).

- **Aus dem Gutachten zu Runde 10, nicht behoben (niedrig):**
  - (erledigt R15c) Abbrechen im Chat meldet „es wurde nichts gespeichert“ (`assist.ts`, `ModelFailure::Cancelled`, Bedienung):
    Bei `persona.chat` ist der Beitrag des Menschen und jede schon fertige Persona-Antwort gespeichert.
  - (erledigt R17b) Mehrdeutige Abkürzungen mit Doppelpunkt gelten im Fließtext als Stelle: „Mi 19:30“ (Uhrzeit) → Micha 19,30,
    ungültig gemeldet, auch im Faktenblock und als Hinweis an Antworten.
  - (erledigt R23) Verschiedene Token-Namen ergeben denselben Handle („Laptop A“/„laptop-a“, lange Namen nach 32 Zeichen; ein
    neuer Token mit dem Namen eines widerrufenen) und damit Eigentum an fremden Beiträgen. Eindeutigkeit über den
    Handle prüfen.
  - (erledigt R15c) Warteschlange nur gemeinsam (1 laufend, 8 wartend): Ein Token-Inhaber kann sie mit `chat` füllen; Grenze je
    Client/Token fehlt.
  - (erledigt R15a) Prüfung auf verbotene Einleitungen gilt auch beim Vorschlag: Ein Abschnitt ohne Überschrift, der mit
    „Natürlich …“ oder „Gerne …“ beginnt, wird neu angefordert und verliert beim zweiten Mal seinen ersten Satz.
  - (erledigt R15a; zusammengefasst mit der Handprüfung oben) „Kritisch:“ im Vorschlag ist nur per Prompt ausgeschlossen (kein Test, keine Prüfung der Ausgabe; ein eigener
    Rollen-Prompt ersetzt die Regel ganz). Ebenso wird „Kritisch:“ bei Lektor/Exeget/Nathan nicht erzwungen
    (Handprüfung: bei einer Ein-Satz-Frage fehlte er).
  - (erledigt R15a) Neun Zusicherungen ohne Begründung (`assist/prompts.rs` 180–193, `tests/round10.rs` 231/307/308,
    `tests/round10_personas.rs` 483/800; Zeilen Stand `50f5bbd`).
  - (erledigt R15c) MCP `chat`: `key` steht in `required`, der STUB-Zweig „Faden ohne Abschnitt“ ist darum für schema-treue Clients
    unerreichbar (Plan: `chat {handle?, node?, message, thread?}`).

## Nach Runde 9 offen (2026-11-06)

- (→ Nutzer) **Im Browser des Nutzers prüfen:** Menü-Tastatur (Pfeile, Pos1/Ende, Esc, Klick daneben), Einstellungs-Popup
  mit Alt+, (Seitenwechsel im Dialog, Formulare per `fetch`, `baruch:settings-changed` → Thema sofort);
  geprüft sind Server-Tests, `tsc` und die Bildschirmfotos der Integration (HANDOFF), nicht die Bedienung mit
  Maus und Tasten.
- (→ Nutzer) **Echter Schlüsselbund:** `KeyringSecrets` ist bewusst ungetestet (Tests nur mit `MemorySecrets`); einmal
  beim Nutzer prüfen, ob der vorhandene Eintrag `baruch/eliezer` (aus Python `keyring`) als „hinterlegt“
  erscheint.
- (überholt: `connections/check.rs:178` folgt `nextCursor` seit Runde 12) MultiGPT-Prüfung liest nur die erste Seite von `tools/list` (kein `nextCursor`).
- (erledigt R23; zusammengefasst mit „Ohne JavaScript außerhalb der Arbeitsseite kein Abzeichen“) Abzeichen außerhalb der Arbeitsseite nur mit JavaScript (`menu.badges`); auf der Arbeitsseite kann die Zahl
  zwischen zwei Abrufen kurz veraltet sein.
- (keine Aufgabe) Abweichung vom Vertrag (bewusst): reqwest/sqlx mit Systemzertifikaten (`rustls-tls-native-roots`,
  `tls-rustls-ring-native-roots`) für die Haus-CA im Intranet; `connection.set` ersetzt nur die gegebenen Felder.
- (erledigt R24 `ci`: `cargo deny check licenses sources bans`, `deny.toml`; `advisories` nur in der CI, nie gelaufen) Rund 100 neue Crates durch keyring, reqwest und sqlx-postgres (Lizenzen geprüft: permissiv).
- **Niedrige Befunde aus dem Gutachten zu Runde 9 (nicht behoben):**
  - (erledigt R13a: gemeinsames Zeitlimit 10 s) Zeitlimit der Verbindungsprüfung gilt je HTTP-Anfrage; die MultiGPT-Prüfung (drei Anfragen) kann bis etwa
    30 s dauern, Bedienung.md verspricht höchstens 10 s – ein gemeinsames `tokio::time::timeout` um die Prüfung.
  - (erledigt Nachbesserung R15, `guard::no_framing`) Kein Schutz gegen Einbetten in fremde Seiten (`X-Frame-Options`/CSP `frame-ancestors` fehlen); betrifft alle
    Seiten, besonders „Geheimnis entfernen“ und „Sicherung jetzt“.
  - (erledigt R23) Kein Test für „Prüfung folgt keiner Weiterleitung“ (`redirect::Policy::none()`); `scrub` ersetzt nur
    wörtliche Vorkommen des Geheimnisses (nicht z. B. Base64).
  - (erledigt R23) Daten: `?ok=sicherung&ziel=…` zeigt den Ordnernamen ungeprüft aus der Adresse (kein XSS, aber falsche
    Meldung möglich) – wie bei Verbindungen nur feste Codes verwenden.
  - (erledigt R23) `settings.ts` ohne Test (auch kein Inhaltstest des Skripts für Alt+, wie bei `menu.js`/`literature.js`).
  - (teilweise R18a: neuer Test für schmale Fenster; misst weiter keine Breite offen nach R24, „Nach Runde 18 offen“) `the_topbar_stays_on_one_line_at_1024_pixels` prüft nur CSS-Regeln, misst keine Breite.
  - (erledigt R23) Ohne JavaScript außerhalb der Arbeitsseite kein Abzeichen und kein Eintrag „Anträge zur Gliederung“.
  - (erledigt R23) Popup meldet `baruch:settings-changed` auch nach abgelehnten Formularen von Verbindungen/Quellen (303 auf
    `?fehler=`); heute folgenlos.
  - (erledigt R13b) Drei Zusicherungen ohne Begründung (`round9_connections.rs` zwei `assert_eq!` auf `details`,
    `baruch-store/src/connections.rs` `source_url`).
- (keine Aufgabe) **Nachbesserung, zur Kenntnis:** Wer die Adresse von Eliezer, MultiGPT oder pgvector ändert, trägt das
  Geheimnis neu ein (sonst hält „Verbindung prüfen“ es zurück); künftige Nutzer der Geheimnisse (Runde
  „Verbindungen“) müssen dieselbe Bindung (`secret_bound`) prüfen. „Sicherung jetzt“ hat keine Zeitdrosselung
  (Grenze 10 Sicherungen und 512 MiB Rücklage genügen); die Platzprüfung gibt es nur unter Unix.

## Nach Runde 8 offen (2026-11-06)

- (erledigt Runde 14: Teil-Markierung, `outline.add`/`outline_add` mit `level: 0`) **Echte Teile auch in Werken mit Werkvorlage (Wunsch 2026-11-06):** Teile (Ebene 0) entstehen heute nur
  neben einem vorhandenen Teil und nur ohne Werkvorlage; sie haben keine Überschriftszeile und gehen im
  Typst-Export verloren. Gebraucht: ersten Teil vor einem Kapitel anlegen (Oberfläche, RPC, MCP `outline_add`
  mit `level: 0`), Darstellung im Typst-Text als Markierung, die `baruch_typst::parse` wieder als Teil erkennt
  (Rundreise byte-gleich, `baruch-wysiwyg` zieht mit), Export/Spiegel/Satz. Anlass: Werk 3 „Herr prüfe mich“
  hat Teil I–IV nur als Plan-Beiträge in den Kapiteln.
- **Gutachten zu Runde 8 (niedrig, offen):**
  - (erledigt R18b) `wysiwyg.ts` `guardHeadingMerge`: steht ein Vorschlag (`.pv`) zwischen Überschrift und Absatz, greift der
    Schutz gegen das Verschmelzen nicht (Nachbar ist das `<details>`); `.pv`-Geschwister überspringen.
  - (erledigt R18a) Typst-Lauf lässt sich nicht abbrechen: ein hängender Satz hält die Sperre bis zum Neustart; GET-Routen
    (`/werke/{id}/satz`) lösen Läufe auch von fremden Seiten aus. Mindestens dokumentieren, besser eigener
    Prozess bzw. Abbruch im World-Zugriff.
  - (erledigt R18c) Test „Satzbild als Vorgabe“ prüft das Widget nur über Textstellen in `wysiwyg.js`, kein Verhaltenstest.
  - (erledigt R18c) Zusicherungen ohne Begründung in `round8_inline.rs` (162, 261) und `round8_posts.rs` (345).
  - (erledigt R18a) Satz-Seite: Blätterknöpfe „‹“/„›“ ohne `aria-label`; „neu“ bei Beiträgen nur über die Farbe.
  - (erledigt R24 `ci`: `cargo deny check licenses sources bans`, `deny.toml`) Rund 196 neue Crates durch Typst (Bauzeit, Lizenzprüfung der Abhängigkeiten).
  - (teilweise überholt: mit Token heißt der Client wie der Token (`mcp/session.rs:7`, Runde 10); Rest ohne Token R23: `lokal-` gegen Token-Namen erledigt; `persona` ohne Token → Nutzerfrage 15) MCP-Beiträge: „eigene“ nach Angabe des Clients (keine Anmeldung) – mit Tokens für `/mcp` (B2) an die
    Sitzung binden.
- (→ Nutzer) **Im Browser des Nutzers prüfen:** Vorschläge im Satzbild (Stücke als `contenteditable=false` zwischen den
  Blöcken; eine Auswahl über ein Stück hinweg kann das `<details>` beim Löschen mitnehmen – es kommt beim
  nächsten Wechsel bzw. Neuzeichnen zurück, der Text bleibt unberührt), Ein-/Aufklappen mit Cookie, Beiträge
  verwalten (Formulare unter „Verwalten“), Abzeichen „k neu“ (`chat.ts`). Geprüft sind `tsc`, Server-Tests und
  die Bildschirmfotos der Integration (HANDOFF), nicht die Maus- und Tastenbedienung. Aus Runde 7 weiter offen:
  Umschalt+Enter, mehrzeiliges Einfügen, Rückschritt/Entf an der Grenze Überschrift ↔ Absatz, Alt+E.
- (erledigt R18c) **Vorschläge an ihrer Stelle (Reste):** „Alle einklappen“ ist im Satzbild nicht zu sehen (die Leiste steht im
  verborgenen Lesefluss); in der Vorschau zeigt der Zeilen-Unterschied eines Stücks am Ende eine leere „−“-Zeile
  (Leerzeile hinter dem Block); bei sehr ähnlichen Blöcken entscheidet die LCS – ein umformulierter Absatz neben
  einem eingefügten erscheint als ein „geändert“-Stück über beide.
- (erledigt R18c) **Satzbild vorab im HTML:** verdoppelt den Abschnittstext im HTML (Textfeld, Blatt, mit Vorschlägen auch
  Lesefluss); für sehr lange Abschnitte ggf. nur auf Anfrage. Der Stil der Werkvorlage (`wysiwyg.style`) kommt
  weiter per RPC – das Blatt wechselt kurz nach dem Laden Schrift und Größe.
- (keine Aufgabe: Festlegung) **Adressen:** `pick` bleibt `<Stelle>:<Nummer>`; Verweise in Beiträgen (`post_ref.node_seq`) bleiben Stellen
  (ihre Links werden über den Schlüssel des Ziels gebaut); `idea.process` mit `as: post` nennt in `done_ref`
  weiter die Stelle des Beitrags (die Antwort trägt `post_id`).
- (erledigt R15c) **Abzeichen „neu“:** beim ersten Besuch ist nichts neu; eigene neue Beiträge zählen nach dem Neuzeichnen als
  neu; bei einem leeren Chat merkt sich das Widget die Uhrzeit des Browsers.
- (erledigt R18a bis auf den Zähl-Abgleich Typst ↔ Baruch offen nach R24 und den Satz des Spiegels (erledigt R22), „Nach Runde 18 offen“) **Typst-Vorschau (Reste):** PDF seit Runde 11; bis zu 8 gesetzte Stände mit allen Seiten im Arbeitsspeicher;
  eine Seite, die zwei Kapitel teilen, zählt in der Seitenliste zum nächsten Kapitel; die Satz-Seite lädt
  `main.js` mit, ohne es zu brauchen; Systemschriften werden beim ersten Satz je Prozess gesucht (einmalig
  spürbar). Die Tests prüfen, dass alle zwölf Gliederungsschemata fehlerfrei setzen, nicht, ob Typst genauso
  zählt wie Baruch. Der Spiegel (`main.typ` mit `#include`) wird nicht gesetzt, nur der Export.
- (erledigt R18a) **Kopfzeile:** Bei 1400 px passt sie jetzt in eine Zeile (Werktitel und Autor kürzen sich); in schmaleren
  Fenstern laufen die Knöpfe weiter aus dem Bild (schon vor Runde 8).
- (→ Nutzer) **Leerer Abschnitt im Buch des Nutzers:** steht weiter als `== ` im Werk; seit Runde 8 lässt er sich speichern,
  solange die Überschrift unverändert bleibt. Der Nutzer benennt ihn um oder löscht ihn.
- (erledigt R22c) **Spiegel der Einfälle ohne Werk** (`<data>/einfaelle/`): wird nur bei einer Aktion an einem Einfall ohne Werk
  nachgeführt, nicht beim Start (`sync_all`) und nicht, wenn ein gelöschtes Werk seine Einfälle zu Einfällen ohne
  Werk macht (SQLite stimmt, der Spiegel holt bei der nächsten Aktion auf).
- (überholt bis auf die Wahl der Persona: Gedächtnis in `assist/context.rs:150`, Auftrag an den Lektor in `templates/idea.html:94` (Runde 10); andere Persona erledigt R15c) **Lektor und Einfälle (B2, STUB):** Der Lektor kennt die Einfälle als Gedächtnis („dazu hattest du am …
  notiert“) und schlägt passende Stellen vor; „Auftrag an eine Persona“ beim Verarbeiten eines Einfalls ebenso
  (B2, braucht Modell-Backends). Bedeutungssuche über Einfälle (LanceDB/RAG) seit Runde 12.
- (erledigt R15b) **`persona.list`** liest Rolle, Modell, Kontext, aktiv und Lektor aus dem Vorgabewerk (die Store-API kennt nur
  Farben); ohne Werk sind diese Felder leer.
- (erledigt R18b) **Satzbild:** Ein Block mit mehreren `image()`-Aufrufen zeigt nur das erste Bild (`image_path` liefert einen
  Pfad). `image_ref` lässt ein führendes `/` in der Adresse stehen (`p=%2Fbilder%2F…`); die Seite findet jede
  Schreibweise.
- (teilweise überholt: Anlegen lehnt Kollision in Normalform ab (`baruch-store/src/assets.rs:623`); Altbestand erledigt R21c mit `asset.path_conflicts`/`asset.path_resolve`) **`/x.png` und `x.png`** können weiter zu zwei Bildern gehören: `asset_by_path` nimmt zuerst den exakten Pfad,
  dann die Normalform (Befund aus Runde 6 nur teilweise behoben; beim Anlegen ablehnen, mit Test).
- **Kleine Befunde aus dem Gutachten zu Runde 7 (niedrig, offen):**
  - (erledigt R23) MCP: Titelregel „einzeilig“ über ein Typst-Zeilenende (U+2028) in `propose_revision` ohne Test
    (`round7.rs` überspringt MCP bei mehrzeiligen Titeln).
  - (erledigt Runde 14: `Store::idea_to_outline`) `idea.process` als Gliederungspunkt ist nicht atomar (`outline_add`, danach `idea_mark_done`); scheitert der
    zweite Schritt, bleibt der Punkt stehen und der Einfall offen.
  - (erledigt R15c) Verarbeitet der Mensch einen Einfall einer Persona oder eines MCP-Clients als Beitrag, steht als Autor der
    Mensch (`ideas.rs`, `IdeaUse::Post`); Herkunft nur über `sources = "Einfall n"`. Herkunft als Autor oder
    sichtbar im Beitrag zeigen.

## Nach Runde 6 offen (2026-11-05)

- (→ Nutzer) **Im Browser prüfen:** Einfügen mehrerer Absätze, Enter und Löschen in Szenen und über Blockgrenzen, Markieren
  über mehrere Blöcke, Satzbild nach `ui.render`, Diff im Satzbild, Kontextmenü und Umbenennen im Baum – bisher
  nur per Test, `tsc` und Bildschirmfoto (Runde 6) geprüft.
- (→ Nutzer) **Bilder im Browser des Nutzers ansehen:** Das Vorschaubild der Bilderliste (`templates/assets.html`) trägt
  weiter `loading="lazy"`; Firefox headless zeigte es im Bildschirmfoto leer (iframe, lazy). Bilder im Satzbild
  laden seit Runde 7 mit `loading="eager"`.
- **Kleine Befunde aus dem Gutachten zu Runde 6 (niedrig, offen):**
  - (erledigt R21b: `check_targets` vor der ersten Datei) `export --out` folgt einem schon vorhandenen Ordner-Symlink im Zielordner (`out/bilder -> …`); Bilder können
    so außerhalb von `--out` landen (`write_all` in `baruch-server/src/import.rs` prüft nur die letzte
    Komponente). Zwischenordner prüfen bzw. `canonicalize(…).starts_with(out)`, mit Test.
  - (erledigt R21b: `check_targets`) Kollidieren zwei Bildpfade als Datei und Ordner (`a.png` und `a.png/b.svg`, `check_path` erlaubt beides),
    bricht `export --out` nach einem Teil der Dateien ab – entgegen „nichts geschrieben“. Vorab prüfen.
  - (erledigt R21c) Tests für die Maskierung von Alternativtext, Bildunterschrift, Notiz und Pfad mit `"<>&'` auf den Seiten
    „Bilder“ und „Bild“ und für ein SVG mit `<script>` über `/werke/{id}/bilder/pfad` (Code nach Lesung korrekt,
    aber ohne Test).
- (erledigt Runde 14: Kopf als enger Block) **Seite „Gliederungsschema“:** große Abstände zwischen den Absätzen oben (kosmetisch).
- (erledigt R18c) **Literatursuche im Arbeitsbereich:** Ein Treffer markiert die Fundstelle im Textfeld, das bei offenem
  Satzbild verborgen ist (`openWorkHit` in `literature.ts`); beim Öffnen über die Literatur auf Quelltext
  schalten oder die Fundstelle im Satzbild markieren.
- (erledigt Runde 14: `Numbering::from_template`, Markierung „ohne Nummer“ statt Show-Regel) **Gliederungsschemata:** Die Regeln der Schemata mit Wörtern liest Baruch beim Re-Import nur näherungsweise
  (juristisch, Teile und Kapitel → „1.1“, Predigt → „I.“). Kapitel mit `numbered = 0` (Vorwort) bekommen im
  Export eine Show-Regel `numbering: none` (an Ebene und Titel erkannt; setzt seit Runde 8 mit dem eingebetteten Typst fehlerfrei); bei
  „aus der Werkvorlage“ (byte-gleich) zählen sie in Typst weiter mit, und bei gleicher Überschrift wie ein
  nummerierter Knoten entfällt die Regel – dort können die Typst-Nummern von der Anzeige abweichen. Ein Typst-Muster mit `;` oder `|`
  in der Vorlage wird als Mustersprache gelesen.
- (keine Aufgabe: Beschreibung) **Verhalten bestehender Werke:** Werke mit `work.numbering = ''` oder eigenem Muster sind seit Migration 0008
  Schema „Ohne Nummern“ bzw. „Eigenes Muster“; ihr Export bekommt die Regel angehängt (vorher beachtete der Export
  `work.numbering` nicht). Werke „aus der Werkvorlage“ blieben bis Runde 13 byte-gleich; seit Runde 14 stellt ihr
  Export Punkte mit „Nummer aus“ als Markierung `<baruch-unnumbered>` dar (gespeicherte Texte unverändert, siehe
  „Nach Runde 14 offen“).

## Eine Gliederung, Klick öffnet den Abschnitt (Rest)

- (erledigt Runde 14: Markierungszeile, Abgleich beim Umschalten) **Umbenennen eines Teils** ändert weiter nur den Titel, nicht seinen Text (Teile haben keine
  Überschriftszeile); Umschalten auf Fassungen eines Teils gleicht die Gliederung nicht ab.
- (erledigt Runde 14: je Ebene ausgewertet, Rest „Nach Runde 14 offen“) **Nummerierung aus einer Funktion:** Seit Runde 6 erkennt Baruch Tiefenbedingungen (`len() == k`, `<= k`,
  `< k`); andere Funktionen, die je Ebene ein anderes Muster wählen, zeigt die Gliederung weiter nur angenähert
  (erstes `numbering("…")` für alle Ebenen).

## Werke (Rest)

- (PDF überholt: `GET /werke/{id}/pdf`, `src/pdf.rs` (Runde 11); EPUB erledigt R20a; Markdown erledigt R21b; Quellen über MultiGPT erledigt R21a) Export als PDF/EPUB, Import von Markdown und Quellen über MultiGPT.
- (erledigt R22c) Wiederherstellen eines archivierten Werks aus der Oberfläche (der reservierte slug in `work_retired` passt
  dann zum Archivordner).

## Gliederungsmanager (Rest)

- (erledigt Runde 14: `outline.requests` mit `preview`, `<details>` auf `/gliederung#antraege`) Gliederungs-Vorschläge der Assistenten mit Vorschau.
- (→ Nutzer) Ziehen, Tastenkürzel und Rückfrage im Browser erst beim Nutzer geprüft (kein Browser im Test).

## Revisionsbaum über das ganze Dokument (Rest)

- (erledigt R22b) die Seitenzweige im Git-Spiegel für nicht aktuelle Fassungen (übrige Fassungen, offene Vorschläge und
  Alternativen als eigene Refs `refs/revisions/<kapitel>/<n>`; Wortlaut unter „Erledigt“, Revisionsbaum);
- (→ Nutzerfrage 5) der **Baum über das ganze Dokument**: umgesetzt ist ein Baum je Abschnitt (`based_on` = Nummer im selben
  Abschnitt); eine Revision als Stand des Werks, die mehrere Abschnitte ändert, und Zusammenführungen gibt
  es noch nicht;
- (erledigt R22a) der **Hinweis auf Alternativen im Lesefluss**: am Abschnitt fehlt der Hinweis „weitere Fassung“ mit
  Umschalten per Klick; heute zeigt der Editor „aktuell: Fassung k“ nur, wenn eine nicht aktuelle Fassung
  offen ist, und Baum bzw. Gliederung zeigen nur Vorschläge und Befunde.

## Literatur als Popup (Rest)

- (Volltext erledigt R13a; an Persona geben erledigt R15c, `literature.rs` Aktion `persona`) Volltext aus MultiGPT (Runde 13, Stub) und „an Persona geben“ (Runde „Literatur (Rest)“, Stub; B2 ist erledigt, die
  Personas antworten seit Runde 10).
- (erledigt R15c) Ein Weg, Persona-Hervorhebungen zu übernehmen.

## WYSIWYG-Editor mit den Originalformaten (2026-11-05)

Der Abschnitt wird so bearbeitet, wie er im Buch aussieht: Satz aus der Werkvorlage (z. B. bei
Das Buch des Nutzers: 12 × 19 cm, EB Garamond, blaue Hervorhebungen, Blöcke `#szene` und `#arbeit`,
Fußnoten, Kapitelbilder), nicht als Typst-Quelltext.

- **Quelle bleibt Typst:** Jede Bearbeitung ergibt Typst-Text und wird wie bisher eine neue Revision
  (Mensch: aktuell; Assistent: Vorschlag). Der Diff der Vorschläge im Lesefluss zeigt Änderungen ebenfalls
  im Originalformat (seit Runde 7).
- (erledigt R18a für `#szene`, `#arbeit` und die eingebauten Formate; Bibelstelle und `@cite` erledigt R19c) **Formate der Vorlage als Werkzeuge:** Was die Vorlage definiert (`#szene`, `#arbeit`, Überschriften,
  Fußnote, Zitat, Bibelstelle, `@cite`), wird zur Schaltfläche bzw. zum Absatzformat – je Werk aus der
  importierten Vorlage abgeleitet.
- (→ Nutzerfrage 4; heute Satzbild aus Blöcken, siehe „Stand Runde 5“) **Darstellung:** Typst eingebettet (`typst`-Crate) rendert den Abschnitt mit der Werkvorlage als SVG
  bzw. HTML; das Widget (TypeScript) legt die Bearbeitung darüber und schreibt Typst zurück.
- **Ohne JavaScript:** Der Quelltext-Editor (Textfeld) bleibt als Rückfall und zum Umschalten erhalten.
- (→ Nutzerfrage 4) **Offen:** Wie weit echtes WYSIWYG geht (direkt im gesetzten Text tippen) gegenüber einem
  synchronen Paar aus Quelltext und Satzansicht; Rundreise ohne Verlust der Typst-Feinheiten
  (Kommentare, eigene Funktionen) ist Pflicht.
- Baut auf der Typst-Vorschau (B1) und der Werkvorlage aus dem Import auf.
- **Stand Runde 5:** Satzbild als HTML aus Blöcken (`baruch-wysiwyg`), nicht als gesetztes Typst; Stil aus der
  Vorlage abgeleitet. Offen:
  - (erledigt R18c) **Einstellungen je Werk** für das Satzbild (Schrift, Farben, Satzbreite überschreiben, wenn die Vorlage
    sie nicht hergibt oder anders gewünscht ist) – heute nur aus der Vorlage.
  - (erledigt R18a; zusammengefasst mit dem Punkt oben) Formate der Vorlage als Werkzeuge (Knöpfe für `#szene`, `#arbeit`, Fußnote, Zitat, Liste; heute nur
    Absatz/Überschrift 1–3, Hervorhebung, Fett).
  - (erledigt R18b) Mehr Konstrukte statt Inseln: Aufzählungen `1.`, Begriffslisten, geschachtelte Listen, erzwungener
    Zeilenumbruch `\`, `#link`, Verweise `@…` als Schaltflächen.
  - (→ Nutzerfrage 4) Echter Satz (Typst eingebettet) für Seitenumbruch, Silbentrennung und Kapitelbilder.
  - (erledigt R18b) Inseln im Satzbild bearbeitbar machen (kleines Quelltextfeld an Ort und Stelle).

## EPUB-Vorschau für verschiedene Reader (2026-11-05)

Neben dem gesetzten PDF entsteht das Buch als EPUB (der bestehende Build des Buchs erzeugt
heute PDF, EPUB, HTML und Hörbuch). Baruch zeigt vor dem Export, wie das EPUB auf verschiedenen
Readern aussieht. (erledigt R20a/b/c; Kindle als Annäherung, EPUBCheck nur gefunden → Nutzerfrage 6)

- **Reader-Profile:** typische Geräte und Programme nebeneinander, z. B. Kindle (Paperwhite),
  Tolino, Kobo, Apple Books (iPhone/iPad), Google Play Books, Calibre/Thorium am Rechner – je mit
  Bildschirmgröße, Standardschrift, Rändern, Silbentrennung und Nachtmodus.
- **Umbruch fließend statt fester Seiten:** Kapitelbilder, Fußnoten (als Popup/Endnote), Blöcke wie
  `#szene`/`#arbeit`, Bibelzitate und Inhaltsverzeichnis werden je Profil geprüft; Warnungen, wo ein
  Reader etwas nicht kann (z. B. CSS-Hintergründe, eingebettete Schriften).
- **Aus derselben Quelle:** EPUB aus den aktuellen Revisionen und der Werkvorlage (Typst → HTML/EPUB),
  kein zweiter Text; die Vorschau nutzt dieselben Profile wie der Export.
- **Prüfung:** EPUBCheck-Ergebnis und Barrierefreiheit (Sprache, Alternativtexte, Lesereihenfolge)
  in der Vorschau anzeigen.
- **Pflicht (Entscheidung 2026-11-05):** alle eingebauten Reader der mobilen Endgeräte und Amazon:
  - Apple Books auf iPhone und iPad,
  - der vorinstallierte Reader von Android (Google Play Books) und die Reader der großen
    Android-Hersteller, soweit vorinstalliert (welche das heute sind, vor der Umsetzung prüfen),
  - Amazon: Kindle-Geräte (E-Ink) und Kindle-Apps; das EPUB geht dort durch Amazons Umwandlung
    (Send-to-Kindle bzw. KDP, KF8/KFX) – die Vorschau muss das Ergebnis danach zeigen, nicht das rohe EPUB.
  Weitere Reader (Tolino, Kobo, Calibre/Thorium) sind erwünscht, aber nicht Pflicht.
- **Je Reader ein eigenes CSS (Entscheidung 2026-11-05):** Jeder Reader bekommt ein eigenes
  Stylesheet, das seine Eigenheiten nachbildet (Standardschrift und -größe, Ränder, Zeilenabstand,
  Silbentrennung, Nachtmodus, was er ignoriert oder überschreibt). Die Vorschau zeigt das EPUB-HTML mit
  dem jeweiligen Reader-CSS im Browser-Widget nebeneinander. Die Stylesheets liegen versioniert im Repo
  (z. B. `crates/baruch-server/static/readers/<reader>.css`) und werden gegen echte Geräte bzw. Amazons
  Kindle Previewer abgeglichen; Abweichungen werden dort nachgezogen.

## Modelle der Personas über Eliezer (Hinweise 2026-11-06, für B2)

**Umgesetzt in Runde 10:** Modellliste aus `GET /v1/models` als Auswahl (Freitext bleibt), Abgleich mit Warnung,
Aliasse für die Ollama-Namen, Rollen-Prompts mit harten Regeln, Lektor fest @lektor (umschaltbar), Schrift nur
mit Stellenprüfung (Wortlaut „ungeprüft“). Offen: Anzeige „lokal“/„extern“ je Persona (erledigt R15b: Abzeichen lokal/extern und Freigabe je Persona), echte Bibeldatenbank (überholt: SWORD-Bibeln, `baruch-sword`, Migration 0030).

Weitergegeben von einer Claude-Sitzung im Auftrag des Nutzers (umgesetzt in Runde 10 bis auf „lokal“/„extern“, das R15b nachgeholt hat):
- **Modellnamen nicht fest verdrahten:** Die Personas tragen heute Ollama-Namen (`llama3.2:3b`, `qwen3:4b`),
  Eliezer kennt andere Schlüssel (`llama-3.2-3b-instruct-q4-k-m`, Qwen3-4B als GGUF Q4_K_M folgt). Die Auswahl
  kommt aus `GET /v1/models` des Backends (Einstellungen „Assistenten“: Liste statt Freitext, gespeicherter
  Name wird beim Prüfen gegen die Liste abgeglichen, unbekannt → Warnung); dazu Aliasse für alte Namen.
- **Exeget** (Text, Zusammenhang, Hauptaussage) auf 3B/8k ist für deutsche theologische Texte zu schwach
  (Beispiel: „Fähigkeit, Gott zu sprechen“); Empfehlung 26–27B (`qwen/qwen3.8-27b`, `google/gemma-4-26b-a4b`).
- **Schrift** und **Belege:** Kein Modell ist Quelle für Wortlaut. Die Persona prüft nur die Zuordnung
  (passt die Stelle?); der Wortlaut kommt aus einer echten Quelle (Elberfelder CSV des Autors bzw. Literatur/
  RAG) oder wird ausdrücklich als **ungeprüft** markiert; ebenso Fußnoten und Quellen bei Belege.
- **Hörer** auf 3B ist passend (einfacher Leser).
- **Eliezer-Modellnamen (Stand 2026-11-06):** `llama-3.2-3b-instruct-q4-k-m` (ersetzt `llama3.2:3b`, ca. 190 Token/s),
  `qwen3-4b-q4-k-m` (ersetzt `qwen3:4b`; llm, reasoning, 2,5 GB, Kontext 40960, ca. 164 Token/s). Kurztest auf
  Deutsch: schwache Sprache (Wortsalat, Grammatikfehler) – bestätigt: Schrift und Belege prüfen nur die
  Zuordnung, erzeugen keinen Wortlaut.
- **Nathan** steht auf `gpt-4.1-mini`, einem externen Dienst: Manuskripttext ginge nach außen. Je Persona
  sichtbar „lokal“/„extern“ anzeigen; extern nur nach ausdrücklicher Entscheidung des Nutzers (Einstellung je
  Persona, Vorgabe aus). **Entschieden 2026-11-06: Nathan läuft lokal auf Gemma.**

### Versuch: ein gemeinsames Modell für alle Personas (2026-11-06)

Vorgabe des Nutzers: höchstens 22 GB VRAM für alle Personas zusammen. `google/gemma-4-26b-a4b` belegt bei 32k
Kontext 19,8 GB (64k: 20,5 GB) – daneben passt kein zweites Modell; im Test einziges Modell mit sauberem Deutsch
(112 Token/s). Darum laufen Lektor, Exeget, Schrift, Belege und Hörer seit 2026-11-06 in den Einstellungen auf
Gemma mit 32768 Kontext (eine Kontextlänge, damit das Modell nicht doppelt geladen wird); Nathan folgt nach Entscheidung des Nutzers
ebenfalls (vorher extern `gpt-4.1-mini`): alle sechs Personas lokal, kein Manuskripttext nach außen. Folgen für B2:
- (überholt: Gleichstand → die erste Persona (`baruch-core/src/lib.rs:193`, Test `automatic_lector_takes_the_first_on_a_tie`), dazu fest @lektor) Die Personas unterscheiden sich nur durch den Prompt. **Lektor-Wahl nach größtem Kontext** greift bei
  gleicher Kontextlänge nicht mehr eindeutig – Gleichstand-Regel festlegen oder Lektor fest setzen
  (Nutzerentscheidung).
- (umgesetzt in Runde 10: Rollen-Prompts mit harten Regeln, `assist/prompts.rs`; Rest erledigt R15a (Rollen-Prompts, „Kritisch:“ geprüft), Handprüfung mit Werk 3 → Nutzer) Handversuch gegen Werk 3, Kapitel I (fünf Rollen nacheinander, Modell einmal geladen, je ~25 s): Lektor
  brauchbar (echte Befunde, aber Füllpunkte); Hörer brauchbar, fällt aber aus der Rolle („Als Assistent…“);
  Exeget beschreibt statt zu prüfen; Schrift unzuverlässig (urteilt „passt“/„ungeprüft“ ohne Prüfung,
  behauptet Übereinstimmung mit einer Übersetzung, die er nicht kennt); Belege markiert Bibelzitate pauschal.
- (umgesetzt in Runde 10 (`assist/prompts.rs`); „Kritisch:“ erzwingen erledigt R15a) Rollen-Prompts brauchen harte Regeln und Gegenbeispiele: nicht „Als Assistent“ beginnen, mindestens ein
  kritischer Befund, Bibelzitate nur bei Schrift, Urteile nur mit Beleg.
- (überholt: Stellenprüfung `baruch-scripture`, Wortlaut über SWORD (`baruch-sword`)) **Schrift prüft Stellen nur gegen eine echte Bibeldatenbank** (lokale Vers-Tabelle bzw. Davids
  `bible/resolve`), nie per Modellurteil; ohne Treffer → „ungeprüft“.

## SWORD-Bibeln (Wunsch 2026-11-06: „Wir brauchen das SWORD-Modul“)

Umgesetzt auf `team/sword` (Migration 0030): SWORD-Leser über die Crate `sword-modules` (CC0) in `baruch-sword`,
Modulordner und Bibel je Werk (Einstellungen → Quellen), Wortlautprüfung je Stelle („stimmt“, „weicht ab“ mit
Wort-Diff, „kein Zitat gefunden“, „Vers fehlt im Modul“, „Versifikation passt nicht“), Faktenblock und Hinweis für
@schrift, RPC `bible.*`, MCP `bible_modules`/`bible_lookup`/`bible_search`, `scripture_check {key}`. Befund
„Mk 1 0,42–45“: getrennte Ziffern aus dem Text des Modells, Baruch setzt sie in der Antwort wieder zusammen.
Handprüfung mit dem freien Modul GerSch (Schlachter 1951) gegen `diatheke`: Ps 23, Joh 3,16, Ps 51,21, Joel 4,1,
2. Chr 1,18 gleich.

Offen:
- (erledigt R17a, Grenzen unter „Nach Runde 17 offen“) **Weitere Verszählungen** (Luther, NRSV(A), Catholic, Synodal, Vulg …) und eine **Abbildung zwischen den
  Zählungen** (heute: andere Zählung nur geprüft, wo das Kapitel in beiden gleich lang ist; sonst „Versifikation
  passt nicht“). `sword-modules` kennt alle 18 Zählungen; es fehlen die Abbildung und Tests (STUB in
  `baruch-scripture/src/versification.rs`).
- (erledigt R17c, Nachbesserung: Abgleich mit libsword, Entpack-Grenze) **Verschlüsselte Module** (`CipherKey`, Schlüssel im Schlüsselbund) – STUB in `baruch-sword`; Kompression BZIP2/XZ
  (Features von `sword-modules`, bewusst aus).
- (erledigt R17c: „Zitate prüfen“ im Editor, Prüfseite ohne JavaScript) **Oberfläche der Prüfung:** Ergebnisse stehen heute in Antworten von @schrift, RPC und MCP; eine eigene Anzeige im
  Editor (Markierung der Zitate mit „stimmt/weicht ab“) fehlt.
- (erledigt R17b: über Absätze, verschachtelt, `#szene`) Zitat-Zuordnung ist eine Faustregel (direkt davor/danach, höchstens 24 Bytes Satzzeichen/Leerraum, Blockzitat mit
  Zuschreibung). Zitate über mehrere Absätze, verschachtelte Anführungszeichen und Blockzitate in `#szene[…]`
  findet sie nicht.
- (keine Aufgabe: ein Index lohnt erst bei vielen Modulen) Suche linear (GerSch: alle 31170 Verse in etwa 1 s im Testlauf); ein Index lohnt erst bei vielen Modulen.
- (erledigt R17c: Vermerk auf Quellen-, Bibel-, Prüfseite, im Widget und in den Antworten) Lizenz der Module beachten: Baruch zeigt den Vermerk der conf (GerSch: „Copyrighted; Free non-commercial
  distribution“), prüft ihn aber nicht.

## Werkzeuge der Personas (2026-11-05)

Seit Runde 12 bekommen @belege und der Lektor auf Wunsch die Treffer der Bedeutungssuche als Belege (Einstellungen
→ Assistenten). Seit Runde 13 sucht @belege auf Wunsch im Web (erledigt, Bedienung „Websuche für @belege“).

- (erledigt R13b) **`@belege` bekommt die Websuche:** über MultiGPT (`web_search` über die SearXNG-Instanz des Haushalts,
  `fetch_url` zum Lesen der Seite) bzw. später ein eigenes Such-Backend. Regeln aus der Erfahrung mit
  Chat 59 (Modell-Recherche lieferte fremde Quellen):
  - Jede Quelle wird abgerufen und gelesen, bevor sie genannt wird; zitiert wird nur Wortlaut, der auf
    der abgerufenen Seite steht, mit URL und Abrufdatum.
  - Ohne abrufbaren Beleg lautet das Urteil „nicht belegt“ bzw. „Angabe prüfen“, nie eine erfundene Quelle.
  - (erledigt R13b über Einstellungen → Quellen; Import der Lesezeichenliste → Nutzerfrage 2) **Bevorzugte Quellen zuerst:** Die Lesezeichenliste des Nutzers mit bevorzugten Quellen wird in Baruch
    gepflegt (eigene Liste, importierbar) und von `@belege` zuerst durchsucht (Suche auf diese Seiten
    eingeschränkt, z. B. `site:`); erst danach das allgemeine Web. Treffer aus bevorzugten Quellen werden
    so gekennzeichnet. Ort und Format der bestehenden Liste: beim Nutzer erfragt.
  - (erledigt R13b: Belegseite, `websearch.adopt`) Gefundene Quellen gehen auf Wunsch in die Autorenbibliothek (Titel, Zitat, Hervorhebung), damit
    sie durchsuchbar bleiben und `@cite` sie findet.

## Bilder: Erzeugung und Oberfläche (Rest nach Runde 6)

- (Vorschau und PDF überholt: Bilder als virtuelle Dateien in `baruch-render/src/lib.rs` (Runde 8/11); EPUB erledigt R20a) **Erzeugung (Vorschau, PDF, EPUB):** Für Typst werden die Bilder beim Setzen zu Bytes entschlüsselt und als
  virtuelle Dateien bereitgestellt (eingebettetes Typst liest aus Baruchs eigener „Welt“). Hängt am
  eingebetteten Typst (B1).
- (Literatur erledigt R21c; Satzbild erledigt R18c: Bild an der Cursorstelle; Hochladen mit Bildern erledigt R21b) **Oberfläche:** Bild in der Literatur zeigen; Bild aus dem Satzbild heraus einfügen (heute nur über den
  Quelltext und die Seite „Bilder“). Hochladen über „Importieren…“ und `work.import` bringen nur den Text.
- (erledigt R21c: `asset.replace` mit `rename`, Satz über `format:`) **Typwechsel beim Ersetzen:** Ein PNG kann ein `.svg` nicht ersetzen (Endung passt nicht); ggf. Umbenennen
  mit Pfadübernahme anbieten.
- (überholt: `asset.search` sucht in Name, Bildunterschrift und Alternativtext (`baruch-store/src/assets.rs:838`, Runde 11)) **Suche nach Name und Bildunterschrift:** Der ursprüngliche Wunsch nannte Bilder „durchsuchbar nach Name und
  Bildunterschrift“. Es gibt keine Suche über Bilder (kein FTS in Migration 0009, nichts in `library.search`);
  die Seite „Bilder“ listet nur.
- (überholt: der Weg über virtuelle Dateien ist umgesetzt, die Prüfung ist gegenstandslos) **Base64 in Typst:** Vor der Erzeugung prüfen, ob Typst Base64 selbst entschlüsseln kann; der Weg über
  virtuelle Dateien funktioniert unabhängig davon (aus dem ursprünglichen Wunsch, noch nicht geprüft).

## Kleinigkeiten

- (→ Nutzerfrage 9) **Überschrift per `#heading(...)` vor der ersten `=`** bleibt Teil der Vorlage (keine Änderung).
- (erledigt R24 `webseite`: `--allow-host NAME`/`BARUCH_ALLOWED_HOSTS`, `docs/Betrieb.md`) **Erlaubte Rechnernamen hinter Proxy:** nicht nötig, solange Baruch nur auf 127.0.0.1 läuft (B2 mit Token/TLS).
- (→ Nutzer, in R24 nicht bearbeitet; siehe „Nach Runde 24 offen“, Schritt „Historie“ – alle drei Commits liegen in `development` und würden mit dem ersten Push öffentlich) **Git-Historie:** Ein kurzer Überschriftsteil aus dem Buch (eine englische Redewendung) steht in drei
  unveröffentlichten Commits (a231aff, e4c5999, 9f3034c); der aktuelle Stand ist sauber. Kein Umschreiben
  der Historie (nichts ist gepusht; vor einer Veröffentlichung des Repos erneut prüfen).

## Erledigt

Abgeschlossene Vorhaben, neueste zuerst; Einzelheiten in `HANDOFF.md`, `docs/Fahrplan.md` und den Commits.

### Runde 21 – „Import und Bilder (B3)“: pdf-import · markdown-import · bilder (2026-11-09)

- pdf-import: PDF lokal (`pdftotext -bbox-layout`) oder über MultiGPT, EPUB lokal, Vorschau mit bearbeitbarer
  Gliederung, Entwürfe, RPC `source_import.*`, Seiten ohne JavaScript, CLI, Quelle in die Bibliothek (Migration 0108),
  Ersatz-Score `rank_score`.
- markdown-import: Markdown nach Typst mit Warnungen je Form, ZIP/Ordner mit Bildern, Berichtsseite,
  `work.import_archive`, `export --out` prüft vorab, Rückweg eines Exports (Buchende, erste Erwähnung, Regelblock,
  „Nummer aus“).
- bilder: Typwechsel mit `rename`, Altbestand `/x.png`/`x.png` auflösen, MCP-Kontingent, Aufräumen nach 30 Tagen,
  `LibraryView.images`, Bilder in der Literatur, EPUB-Cover als Rasterbild mit Titel.
- Integrator: Abnahme `tests/round21_abnahme.rs`, `/quelle` mit lokalem Volltext, Sichtprüfung (`.claude/sicht/r21/`).
  Nachbesserung `fc6b3f6`: Schutz-`;` beim PDF/EPUB-Import, Fußnoten-Nummern und -Bilder, EPUB-Bildtyp nach Inhalt,
  Seitengrenze lokal, Handprüfung in sieben Schritten.

### Runde 31 – „Register und Glossar“: glossar-daten · register-satz · kontextmenue (2026-11-09)

- glossar-daten: Migration 0049 (Glossar, Varianten, eigene Register, Registervorschläge, Grabsteine), Store und RPC
  `glossary.*`, Spiegel `glossar.md`, MCP `glossary_list`/`glossary_propose`/`index_propose` (nur Vorschläge),
  `glossary.suggest` über den Lektor, Austausch Unterversion 7.
- register-satz: Registermarken `#metadata(…)<baruch-idx>`, Registerseiten mit Seitenzahlen aus Typst (Bereiche,
  Unterstichwörter, „siehe auch“, Spalten), Bibelstellenregister in biblischer Reihenfolge, Glossarseite, erste
  Erwähnung je Kapitel, EPUB `glossary.xhtml`/`index-*.xhtml`, `index.mark`/`unmark`/`entries`/`overview`.
- kontextmenue: Kontextmenü in Satzbild und Quelltext (ARIA, Umschalt+F10), Menüband „Register und Glossar“, Seiten
  Markierung/Glossar/Register ohne JavaScript, Satz-Einstellungen.
- Integrator: Abnahme `tests/round31_abnahme.rs`, Import mit Buchende, Sichtprüfung (`.claude/sicht/r31/`).
  Nachbesserung `877d485`: keine doppelten Registermarken, „Begriffe vorschlagen lassen“ als Formular, S1-Zweige der
  Tests entfernt, Handprüfung mit Werk 3.

### Runde 19 – „RAG-Pflege, Reste der Stellen“: index · pgvector · treffer (2026-11-09)

- index: Index-Worker ohne Versuchszyklus je Suche, ehrlicher Status (`index.stale`, „n Aufträge liegengeblieben“),
  `rag.reindex` sofort (`deferred`), unterbrochene Aufträge kommen wieder (Nachbesserung), Bereichs-`replace`,
  Einfälle gelöschter Werke sofort, Start mit eigenem Datenordner ohne Einbettungsanfrage, Präfixe in LanceDB,
  Abgleich mit MultiGPT gehärtet (Hochladen ohne ID, Fehler je Dokument, volle Seite, „hochladen-begonnen“, `DELETE`).
- pgvector: Felder in SQL gekürzt, Bezeichner immer gequotet, TLS-Hinweis (auch bei `require`), Aufgaben-Präfixe,
  Gruppen der Gedankenkarte benennen (Vorschlag, Übernehmen nur Mensch), MCP `idea_map` mit Nähe-Parametern,
  Menüband „Bibelstelle“/„Literaturverweis“, NFC.
- treffer: „schwach“ unter 0,45 in Literatur, Einfällen, Belegen und JSON, `idea.search` -32001, Wortlaut gegen Module
  mit Trennresten, „Mal“ als Maleachi, Zitat-Bindeglied, Lektor-Hinweis, `german_ref` auf der Bibelseite, Lemma-Probe,
  Kasten „Bibeln“ auf der Arbeitsseite.
- Integrator: Abnahme `tests/round19_abnahme.rs`, Beschreibung von MCP `idea_search`, Sichtprüfung (`.claude/sicht/r19/`).
  Nachbesserung `ec25ac9`: drei mittlere Gutachtenbefunde (unterbrochener Auftrag, TLS bei `require`, Handprüfung).

### Runde 29 – „Buchsatz in den Einstellungen“: satz-daten · satz-typst · satz-oberflaeche (2026-11-09)

- Einstellungen → Werk → Satz ohne JavaScript mit Vorschau, Regelblock „// Baruch: Satz“ hinter der Werkvorlage,
  Migration 0048, Austausch u6, RPC `work.typeset_*`, MCP `typeset_settings`; Einzelheiten unter „R29“ oben und in
  `HANDOFF.md` (Eintrag im PM-Abschluss R29 vergessen, in R19 nachgetragen).

### Runde 30 – „Suchen/Ersetzen, Querverweise, Chat: Lösungsvorschläge, Varianten, Bedienung“: suchen-verweise · chat-loesungen · chat-bedienung (2026-11-09)

- suchen-verweise: `search.find|preview|replace|undo` (Groß/klein, ganzes Wort, Regex mit Grenzen, Bereiche,
  Fassung, Textarten, Zeitgrenze 2 s), Ersetzen als je eine Fassung vom Menschen und ein Commit, Rückgängig ein
  Commit, `/suche` ohne JavaScript, Suchleiste mit VS-Code-Tasten, Seitenleiste; Sprungmarken `<kN>` in Export,
  Spiegel und Satz, `xref.*`, `/verweise`, Menüband „Verweis“, Strg+Klick, Alt+←, MCP `text_search`.
- chat-loesungen: Lösungspflicht mit einmaliger Nachforderung, Lösungen je Befund, `solution.adopt`, „Mein
  Vorschlag:“ als Sammel-Vorschlag, gleiche Prüfgrundlage (`basis`), Varianten A/B/C mit `/varianten`, Gliederung mit
  `[key:N]` im Prompt, Austausch u5.
- chat-bedienung: Anrede ohne @, Relay bei direkter Anrede (eine Stufe), ↻ Wiederholen, ✕ Löschen, Live-Anzeige
  (`post.since`, „schreibt …“), sicheres Markdown-Teilset, geprüfte Sprungmarken-Links.
- Integrator: Abnahme `tests/round30_abnahme.rs`, Verdrahtung MCP (`basis`, `post_delete {thread}`), Sichtprüfung
  (`.claude/sicht/r30/`). Nachbesserung `88df145`: fünf mittlere Gutachtenbefunde.

### Runde 28 – „Befunde: Befundliste, Übersicht übers Werk, Aufträge im Chat“: befunde · befund-uebersicht · chat-auftrag (2026-11-09)

- befunde: Migration 0046 (`finding`, `finding_status`, `post_version`, Grabstein-Arten), Zerlegen der Antworten
  (Marken, Gliederung „Was trägt/fehlt/stört“, Zitat, Prüfvermerk, `[B<id>]`), Ähnlichkeit und Anhängen,
  einmaliges Nachzerlegen, RPC `finding.list|get|set_status|decide`, Spiegel `befunde.md`, Austauschformat u4.
- befund-uebersicht: Seite `/werke/<id>/befunde` (Matrix, Filter, Listen, Sammelvorschläge), `finding.matrix`,
  Abzeichen in Gliederung, Editorkopf, `/gliederung` und Menü, Akkordeon mit Statusformularen, Ebene Befunde in der
  Gedankenkarte, „setzt Befund x um“ im Fassungsbaum, MCP `findings` und `finding_propose_status`, Widget `findings`.
- chat-auftrag: Kritisch-Pflicht nur für @nathan, Lektor als Begleiter, Entscheidungen und Chatverlauf gerahmt,
  RFC/`@alle` (Lektor zuletzt mit Sammlung), Schreibauftrag als Vorschlag, Beiträge bearbeiten mit Verlauf
  (`post.edit`, `thread.post_edit`, MCP `post_edit`), Chat rollt, Kopieren/Zitieren, `/beitraege` und `/beitrag`.
- Integrator: Abnahme `tests/round28_abnahme.rs` (RFC mit sechs Personas → Befunde und Sammelvorschläge, Status über
  Formulare mit je einem Commit, Entscheidungen im nächsten Aufruf, angehängte Wiederholung, gleiche Zahlen in Matrix,
  Abzeichen, `/gliederung` und Karte, Fassungsbaum, Lektor/Nathan-Regel, Rundreise u4, Chat ohne JavaScript).

### Runde 27 – „Umschlag-Editor und Impressum in der Werkvorlage“: umschlag-daten · umschlag-satz · umschlag-widget (2026-11-09)

- umschlag-daten: Migration 0045 (`cover_block`, Grabsteinart, Anschriften, Antragsart `cover_layout`), Store mit
  Grenzen und Fingerabdruck, RPC `cover.layout|layout_set|layout_reset|fonts`, MCP `cover_layout` (lesen) und
  `cover_layout_set` (Antrag), Austauschformat Unterversion 3 (Blöcke, Anschriften, Satzbild).
- umschlag-satz: Umschlag und Buch-Vorderseite aus Blöcken (Lage ab Schnittkante, Rücken auf/ab), Blockhöhen gemessen,
  Schriften mit Rückfall, Titelei-Funktionen der Werkvorlage (`baruch_buch`), Impressum-Vorlage für neue Bücher,
  Druckprüfung (Beschnitt, Sicherheit, Strichcode, Rücken, Kontrast, Schrift, Pflichtangaben); ohne Anordnung
  byte-gleich (Goldstände).
- umschlag-widget: Umschlagseite als Editor (Rahmen, Hilfslinien, Seitenleiste), Vorschau per GET, Formulare ohne
  JavaScript, Widget `cover-editor` (Ziehen, Breite, Tastatur, Einrasten, Rückgängig), Anschriften in den
  Buchangaben, Knopf „Impressum-Vorlage in die Werkvorlage übernehmen“.
- Integrator: Abnahme `tests/round27_abnahme.rs` (Lage in Umschlag- und Buch-PDF gemessen, gleiche Quelle von Vorschau
  und PDF, Formular = RPC, Rückkehr zum byte-gleichen Satz, Impressum aus der Vorlage, MCP, Austausch).
- Nachbesserung: Grundschrift der Werkvorlage wie Typst (letzte `#set text(font: …)` auf oberster Ebene), Handprüfung
  Werk 3 in neun Schritten; PM: MCP-Hinweis mit `cover_layout`.

### Runde 18 – „Editor und Menüband“: menueband · konstrukte · satzbild-werk (2026-11-08)

- menueband: Menüband mit Formaten der Werkvorlage (Szene/Arbeit nur, wenn definiert), RPC `ribbon.formats`, Satz-Seite
  ohne JavaScript mit eigenem Menüband, „Satz abbrechen“ (`preview.cancel`, `compile_cancellable`), fremde Seiten
  bekommen bei Satz/PDF 403, Zwischenspeicher mit Bytegrenze, geteilte Seiten zählen zu beiden Kapiteln, Schriften
  beim Start, Kopfzeile in schmalen Fenstern, Kanten der Gedankenkarte mit Pfeil und Beschriftung.
- konstrukte: Aufzählung, Begriffsliste, geschachtelte Listen, Umbruch `\`, `#link`, `#ref`, `Edit::Raw` (Insel an Ort
  und Stelle), Teil im Kapiteltext abgelehnt, mehrere Bilder je Insel; Lektor-Reste (ausgelassene Teile in einer
  Zeile, Zwischenspeicher der Zusammenfassungen, „an Persona geben“ auf `/quelle/multigpt`, Ereignis `piece`).
- satzbild-werk: Satzbild-Einstellungen je Werk (Migration 0044 `work_sheet`, RPC `work.sheet`/`work.sheet_set`,
  Formular), Stil schon im HTML, Bild an der Cursorstelle in einer Transaktion, Vorschläge-Reste, Literatur-Treffer im
  Satzbild, Verhaltenstest „Satzbild als Vorgabe“ in Node.
- Integration und Nachbesserung: Abnahmetest `round18_abnahme.rs`, Sichtprüfung (Editor unter 900 px, Beschriftungen
  der Karte); nach dem Gutachten Rohblock-Insel im Rumpf behält ihren Einzug, Umschlag/Stichwortzettel/Karte-PDF für
  fremde Seiten gesperrt, Kopfzeile der Satz-Seite ohne `main.js`, Handprüfung mit Werk 3 in acht Schritten.

### Runde 25 – „Gedankenkarte (Mindmap der Einfälle)“: karte-daten · karte-bild · karte-widget (2026-11-08)

- karte-daten: Migration 0043 (`idea_link`, `idea_map_pos`, Grabstein-Arten), Store `idea_links.rs`, RPC `idea.map/
  links/link/unlink/link_restore/map_pos_set/map_pos_reset`, MCP `idea_map/idea_links/idea_link/idea_unlink`,
  Spiegelzeile „Verbindungen“ in `einfaelle.md`, Nähe aus gespeicherten Vektoren und gleichen Schlagwörtern.
- karte-bild: Anordnung radial und deterministisch (300 Einfälle ≈ 1 ms, ohne Überlappung), Seite `/werke/{id}/karte`
  ohne JavaScript mit Gliederungsliste, Widersprüchen und Formularen, Ausgaben SVG/PDF/Markdown/FreeMind, Menüpunkt.
- karte-widget: `idea_map.ts` (Zoom, Verschieben, Ziehen, Tastatur, Seitenleiste, Alt+E), Austauschformat
  Unterversion 2 (`<karte>`, `docs/schema/baruch-werk-1-u2.xsd`).
- Integration und Nachbesserung: Grabstein-Trigger bei hart gelöschtem Einfall, Abnahmetest, Sichtprüfung (Höhe,
  Einpassen); nach dem Gutachten löst ein Werkwechsel die Verbindungen, Teile zählen ihre Kapitel, leere Kapitel
  sind mit „leer“ hervorgehoben.

### Runde 26 – „Austauschformat zwischen Instanzen (XML)“: ids · export · import (2026-11-08)

- ids: UUIDs für zehn Tabellen mit Fülltriggern und Grabsteinen (Migrationen 0041, 0042), Instanzkennung und -name
  (Einstellungen → Daten, RPC `instance.*`), UUID in „Werk ändern“.
- export: Crate `baruch-exchange` (Modell, Schreiber, deterministisches ZIP mit Manifest), XSD, `Store::exchange_dump`,
  Seite `/werke/{id}/austausch` mit Downloads, RPC `work.exchange_export`, CLI `export --baruch`, MCP `exchange_export`.
- import: sicherer Leser (keine DTD, Grenzen, Pfade, Prüfsummen), `validate`, `exchange_apply` (neu und abgleichen
  nach F6, Anträge mit Herkunft „Austausch: <Instanz>“, ein Spiegel-Commit), Seiten `/austausch/*` ohne
  JavaScript, RPC `work.exchange_preview/_import`, CLI `import DATEI.baruch`.
- Integration: Wächtertest und Rundreise-Test nach dem Merge berichtigt, Umbenennen-Test, MCP-Hinweis, Einzahl/
  Mehrzahl und Layout der Austauschseiten, Ende-zu-Ende mit zwei Wegwerf-Instanzen (75 Prüfungen).
- Nachbesserung `1c4a3ca` nach dem Gutachten: gelöschtes Werk aus eigener Datei zurückspielen (Einfälle ohne Werk
  werden wieder zugeordnet), keine inhaltsgleichen Fassungen beim Hin und her, Instanzname aus der Datei gesäubert
  (Git-Autor), unbekannte Grabstein-Art bei `version` > 1 übersprungen, Rundreise-Test ohne Ausweg, Startbefehl der
  Wegwerf-Instanz in HANDOFF. PM-Abschluss: Besitzvermerke entfernt, RPC-Einleitung zum Vorgabewerk berichtigt,
  „Ältere Datei“ in Austauschformat.md §4.

### Runde 16 – „Predigt (B4)“: predigt-vorlage · zeitwaechter · stichwortzettel (2026-11-08)

- predigt-vorlage: Neues Werk → Predigt (Schema `sermon`: Predigttext · Einleitung · I.–III. · Anwendung · Schluss),
  Predigttext aus SWORD als Zitat (ohne Bibel nur die Stelle), Prompts „Predigt“, fremde Befunde/Chat/Relay gerahmt,
  Belegpflicht für „Kritisch:“, wirksame Kontextlänge für Fäden und „Lektor automatisch“, „Chat nur Abschnitt“.
- zeitwaechter: Sprechdauer (`baruch_typst::timing`, Migration 0039), Zielzeit mit Warnung in Baum, Editorkopf und
  `/gliederung`, Einstellungen → Werk → Predigtzeit, RPC `work.timing*`/`timing.count`, MCP `work_timing`; lokal/extern
  im Akkordeon „Assistenten“.
- stichwortzettel: A5/Karten über `baruch-render` (Seite, `notes.get`/`notes.pdf`, Download, CLI `--stichwort`),
  Protokoll-Commit beim Ablehnen jeder Antragsart (Entscheidung 2), Vorschau „neuer Teil“ mit Art und Nummer.
- Integration mit Ende-zu-Ende-Tests (`round16.rs`), Handprüfung mit zwei Anfragen an Eliezer („Kritisch:“ hinter der
  Teil-Angabe gefunden und behoben), `static/predigt.css`, Gutachten (13 Befunde) und Nachbesserung `7dc257c`
  (Fußnoten im Stichwortzettel, Stacküberlauf durch tiefes Markup, weitere Formen der Teil-Angabe, Handprüfung in HANDOFF).

### Runde 15 – „Lektor und Personas“: lektor · anbieter · auftraege (2026-11-08)

- Vorlauf Nutzerfrage 12: neuer Teil mit Folgekapiteln über MCP wird ein Antrag mit Zustimmung im Popup bzw. Formular
  (Migration 0035, `consent.rs`).
- lektor: ganzes Kapitel (Predigt: ganzes Werk) im Wortlaut, Befunde anderer Personas, `scope` section|chapter,
  Stückeln/Komprimieren statt still kürzen, „Kritisch:“ im Code, SWORD-Faktenblock, Toast.
- anbieter: Verbindungen je Persona (Eliezer, OpenAI-kompatibel, Ollama; Migration 0036), lokal/extern mit Freigabe,
  Kontextabgleich, SWORD-Kommentare und -Lexika (Migration 0037, MCP `commentary_lookup`/`lexicon_*`).
- auftraege: SSE `posted`/`retry`/`compressed`, Hinweisseite statt 422, Literatur „an Persona geben“, Fäden ohne
  Abschnitt (Migration 0038), MCP `chat` mit Relay und Warteschlange je Client, Bibeln in der Literatur.
- Integration mit Ende-zu-Ende-Tests (`round15.rs`), Handprüfung mit drei Anfragen an Eliezer, Gutachten (15 Befunde)
  und Nachbesserung `ace0e36` (Stücke gemessen, Ollama-Kontext, Einbetten bei externem Eliezer, Lage nach Adresse,
  `frame-ancestors`).

### Runde 14 – „Teile und Gliederung“: teile-typst · teile-gliederung · nummerierung (2026-11-07)

- teile-typst: Teil als Markierung `#heading(level: 1, numbering: none)[T] <baruch-part>`, „ohne Nummer“ als
  `<baruch-unnumbered>` (`baruch_typst::marker`), Import, Satzbild-Blöcke, Teil-Seite auf einer rechten Seite im Satz.
- teile-gliederung: Teile anlegen auch mit Werkvorlage (RPC, Formular mit „Ebene“, Kontextmenü, MCP `outline_add` mit
  `level: 0`), Umbenennen schreibt die Markierung, Abgleich beim Speichern/Umschalten, Anträge mit Vorschau,
  `idea.process` als Gliederungspunkt atomar.
- nummerierung: `Numbering::from_template` (Funktionen je Ebene, Wort-Schemata beim Re-Import), Projektion „ohne
  Nummer“ in Export, Spiegel und Satz, Typst prüft die Nummern selbst (`#context assert.eq`), Kopf der Seite
  Gliederungsschema.
- Integration mit Ende-zu-Ende-Tests (`round14.rs`), Gutachten (13 Befunde) und Nachbesserung `bba23db`; keine
  Migration.

### Runde 13 – „Quellen-Dokumente und Websuche“: bau · quellen · websuche · abgleich (2026-11-07)

- bau (Vorlauf): Server-Integrationstests als eine Binärdatei (`tests/main.rs`), pgvector-Tests teilen einen
  Container; `target/` 38 → 9,0 GB.
- quellen: gemeinsame MultiGPT-Sitzung mit Vorrat, Volltext `/quelle/multigpt`, `source.read`/`source.adopt`, MCP
  `source_read` (nur gewählte Sammlungen), Prüfung von Hervorhebungen gegen den Volltext (Migration 0106).
- websuche: @belege sucht auf Wunsch im Web über MultiGPT, Zitate im Code geprüft, Quellenzeile, Belegseite,
  Übernahme belegter Zitate in die Bibliothek (Migration 0034); nicht über MCP.
- abgleich: Hochladen der aktuellen Fassungen je Kapitel nur nach Bestätigung, gewählte Sammlungen als lokale Kopie
  im Index (Art `remote`), Seite `/abgleich`, Menü „Abgleichen“ (Migration 0033).
- Integration mit Ende-zu-Ende-Tests (`round13.rs`), Gutachten (19 Befunde) und Nachbesserung `1251a5b`.

### Runde 12 – „Verbindungen nutzen, RAG“: lancedb · pgvector · multigpt (2026-11-07)

Umgesetzt ist der Abschnitt „Einstellungen: Verbindungen“ (2026-11-05) mit der Entscheidung „RAG umschaltbar,
LanceDB ist das Haupt-RAG“:
- lancedb: Crates `baruch-rag` (gemeinsame Schnittstelle) und `baruch-lancedb`, Einbettungen über Eliezer
  (`/v1/embeddings`, eine Anfrage zur Zeit), Index im Datenordner folgt Fassungen, Einfällen, Hervorhebungen und
  Bibliothek, Modellwechsel ehrlich, RPC `rag.search`/`rag.status`/`rag.reindex`, „Verbindung prüfen“ für LanceDB.
- pgvector: Suchweg direkt (nur lesend, Kosinus, Zuordnung der Tabelle), Sammlungen je Werk (Migration 0031),
  Einstellungen → Verbindungen mit Sammlungen, Status der Wege und „Index neu aufbauen“, Umschalter
  Wortlaut/Bedeutung in Literatur und Einfällen.
- multigpt: MultiGPT-MCP als Suchweg, Prüfung über `nextCursor` und `list_collections`, MCP `rag_search`/`rag_status`,
  Treffer als Belege für @belege und den Lektor (Migration 0032).
- Integrator: Haken verdrahtet, `rag::catch_up` beim Start, `work.delete` räumt den Index, Ende-zu-Ende
  `tests/round12.rs`.

#### Einstellungen: Verbindungen (2026-11-05, umgesetzt in Runde 12)

**Stand Runde 9:** Die Seite Einstellungen → Verbindungen speichert und prüft alle vier Wege (Migration 0013,
Geheimnisse im Schlüsselbund), RAG-Wege je Werk sind wählbar, die Quellenliste steht (MCP `sources_list`).
Offen ist hier die **Nutzung** (STUB): Einbetten und Suchen über LanceDB, pgvector und MultiGPT, Sammlungen je
Werk, Zusammenführen der Treffer, Prüfen von LanceDB.

**Entscheidung: RAG umschaltbar, LanceDB ist das Haupt-RAG (2026-11-05, korrigiert).** Umgesetzt in Runde 12
bis auf ältere Revisionen und Befunde im Index (siehe „Nach Runde 12 offen“).
- **Hauptweg: LanceDB lokal, eingebettet** (wie im Plan): Texte, Revisionen, Befunde, Hervorhebungen und
  abgeglichene Quellen werden lokal eingebettet und durchsucht – auch offline.
- **Umschaltbar: zwei Remote-Wege, beide verbindbar** über die Einstellungen:
  1. **pgvector direkt** (Postgres mit den Einbettungen, siehe unten),
  2. **MultiGPT-MCP** (`search_documents`, `read_document`, `list_collections`, `document_info` – mit
     MultiGPTs eigener Aufbereitung, Rechten und Quellenangaben).
  Je Werk wählbar, welche Wege gefragt werden (Mehrfachauswahl): lokal (LanceDB), pgvector, MultiGPT-MCP;
  bei mehreren führt der Lektor die Treffer zusammen und entfernt Doppelte (gleiches Dokument/Abschnitt).
- Volltextsuche (FTS5) bleibt unabhängig davon immer lokal verfügbar.
- Fällt ein Remote-Weg aus, gelten die übrigen weiter, zuletzt der lokale; die Oberfläche zeigt, welche Wege verbunden sind.

- **Remote-RAG über das MultiGPT-MCP:** Adresse (Vorgabe: bis Runde 23 eine Intranet-Adresse, seit Runde 24 leer) und Token
  (Schlüsselbund, Dienst `baruch`, Konto `multigpt`); „Verbindung prüfen“ zeigt Server, Werkzeuge und Sammlungen (Konto und Budget meldet MultiGPT nicht; offen);
  Sammlungen je Werk wählbar.
- **Remote-RAG direkt auf pgvector:** die Postgres-Datenbank mit pgvector (die Einbettungen der Sammlungen):
  - Verbindung: Host, Port, Datenbank, Benutzer (am besten ein eigener Nur-Lese-Benutzer für Baruch),
    TLS; Passwort im Schlüsselbund (Dienst `baruch`, Konto `pgvector`). Knopf „Verbindung prüfen“ zeigt
    Sammlungen, Zahl der Abschnitte, Dimension und Einbettungsmodell.
  - Abfrage über den Vektor-Operator von pgvector mit gebundenen Parametern; die Anfrage wird mit
    **demselben Einbettungsmodell** eingebettet wie die Daten in der Datenbank (Vorgabe
    `nomic-embed-text`, 768 Dimensionen, über Eliezer) – sonst sind die Treffer wertlos; Baruch prüft
    Modell und Dimension beim Verbinden.
  - Sammlungen je Werk auswählen; nur diese werden durchsucht.
  - Tabellen- und Spaltennamen kommen aus der Konfiguration bzw. einer kleinen Zuordnung, damit Baruch
    nicht still bricht, wenn sich das Schema auf der anderen Seite ändert (Prüfung beim Verbinden).
  - Ohne Verbindung arbeitet Baruch lokal (LanceDB) weiter und zeigt das an.
  - Zugangsdaten trägt der Nutzer selbst ein und prüft die Verbindung selbst; Agenten bekommen sie nicht.
    Tests laufen gegen ein eigenes Postgres mit pgvector im Test (oder werden ohne es übersprungen).
- Auf derselben Seite die übrigen Verbindungen: Endpunkt je Persona (Eliezer `http://127.0.0.1:1234`,
  Schlüssel `baruch`/`eliezer`), Websuche für `@belege`, Liste bevorzugter Quellen, Git-Remote des Spiegels.

### Runde 10 – B2 „backend · personas · scripture“ (2026-11-06)

- backend: Crate `baruch-llm`, `models.rs` (RPC `model.*`, Kurztest, Abgleich der Persona-Modelle).
- personas: Port `ModelPort`, Prüfen/Vorschlag/Chat mit Strom, Rollen-Prompts (0015), Lektor fest/automatisch,
  MCP `ask_persona`/`chat`, Einfall an Persona.
- scripture: Crate `baruch-scripture` (Schlachter 1951; Verszählung nach der Nachbesserung deutsch nach SWORD „German“),
  `scripture.check`/`scripture_check`,
  MCP-Tokens (0016), `guard::remote_only_mcp`.
- Integrator: `EliezerPort`, Verdrahtung, Ende-zu-Ende `tests/round10.rs`, Zeilenumbrüche in Beiträgen,
  `BARUCH_MAX_TOKENS`, Vorschläge ohne Befunde.

### Runde 9 – „menu · settings · connections“ (2026-11-06)

- **Kopfzeile mit Menüs** (menu): eine Kopfzeile in `base.html` für alle Seiten – Baruch, Werkwahl, Titel,
  **Werk ▾**, **Ansicht ▾**, **Werkzeuge ▾** (`<details>`, ohne JavaScript bedienbar; mit JavaScript
  ARIA-Menü-Tastatur), seitenlokale Links, kleiner Thema-Umschalter, **⚙ Einstellungen** (Alt+,); Abzeichen
  für offene Anträge und Vorschläge (RPC `menu.badges`); bei 1024 px einzeilig. EPUB-Vorschau und Abgleichen
  als gesperrte STUB-Einträge.
- **Einstellungen als Popup** (settings): sieben Seiten (Darstellung, Werk, Assistenten, Verbindungen, Quellen,
  MCP-Zugang, Daten) als `<dialog>` und ohne JavaScript unter `/einstellungen/<seite>`, jede mit „Gespeichert:
  …“; Cookie `baruch_pv`; Werk-Formulare wiederverwendet (`back`); Personas mit Rolle, Modell, Kontext, aktiv
  und Lektor-Anzeige (`persona.update`); MCP-Clients; „Sicherung jetzt“ (`VACUUM INTO`, nie überschreiben);
  RPC `settings.*`. Kein MCP-Werkzeug `settings_read` (Begründung in `docs/MCP.md`).
- **Verbindungen und Quellen** (connections, Migration 0013): Eliezer, MultiGPT, pgvector, LanceDB speichern und
  prüfen (10 s, 1 MiB, keine Wiederholung), Geheimnisse nur im Schlüsselbund (nie in HTML/RPC/Adresse/
  Prüfergebnis), RAG-Wege je Werk, Lesezeichen bevorzugter Quellen mit MCP `sources_list`. Nutzung der
  Verbindungen folgt (STUB, Runde 13).
- **Gutachten und Nachbesserung** (`8f74107`): Geheimnis an das Ziel gebunden (Migration 0014,
  `connection.secret_target`; nach Adressänderung schickt die Prüfung nichts), Geheimnis vor dem Speichern
  geprüft, Werk-Formulare im Popup behalten bei Fehlern Rahmen und Eingabe, „Sicherung jetzt“ mit Grenze und
  Platzprüfung, `serve --secrets keyring|memory` (Wegwerf-Server mit eigenem `--data` ohne Schlüsselbund),
  „Fassungen“ unter Darstellung wählbar, Abzeichen-Kontrast 5,07 : 1. Abschnitt „Vergrößern“ in Bedienung.md
  aus den Einstellungen gelöst (PM).

### Runde 8 – „inline · posts · typst“ (2026-11-06)

- **Vorschläge an ihrer Stelle** (inline): `baruch_wysiwyg::align` richtet jeden offenen Vorschlag an den Blöcken
  der gezeigten Fassung aus; je Stück ein `<details>` in Persona-Farbe direkt am betroffenen Block (Satzbild,
  Quelltext-Lesefluss, Vorschau), eine Kopfzeile, „Teil i/k“, eingeklappt eine Linie an genau dieser Stelle.
  Linien mit mindestens 3 : 1 Kontrast in beiden Themen (`--pc-line`, `--pc-line-dark`).
- **Satzbild als Vorgabe** (inline): Ursache belegt – der Server schickte ein leeres Blatt, `wysiwyg.ts`
  schaltete beim Einhängen erst auf Quelltext und öffnete das Satzbild nach zwei RPC-Antworten; Bildschirmfotos
  nach `load` sahen den Zwischenzustand. Jetzt rendert der Server den Zustand aus dem Cookie `baruch_editor`
  (Vorgabe „satz“) samt fertigem Satzbild.
- **Adressen über den festen Schlüssel** (posts): `/?work=W&key=K` überall angenommen und in allen erzeugten Links
  und Weiterleitungen; `node=` nur Rückfall; unbekannter Schlüssel → Rückfall mit Hinweis. MCP-Antworten tragen
  `url`; Treffer `node_key`.
- **Beiträge verwalten** (posts): löschen, verschieben, in Einfall umwandeln – Oberfläche ohne JavaScript, RPC
  `post.*`, MCP nur eigene Beiträge; je Aktion ein Spiegel-Commit, Löschen und Umwandeln mit dem ganzen Beitrag
  im Rumpf. Chat-Abzeichen mit Zahl und „k neu“.
- **Gutachten-Reste aus Runde 7** (posts, inline): unveränderte leere/überlange Titel blockieren das Speichern
  nicht mehr; `href()` prüft auch im Release; Linienkontrast.
- **Typst-Vorschau mit Werkvorlage** (typst): Crate `baruch-render` (Typst 0.15.1 eingebettet, offline, SVG),
  Export-Karte im Store, RPC `preview.typst`, Seite `/werke/{id}/satz` ohne JavaScript, MCP `typst_check`; die
  zwölf Gliederungsschemata und die Regel für Überschriften ohne Nummer setzen mit Typst fehlerfrei (vorher
  „nicht mit `typst compile` geprüft“).

### Fehler aus der Arbeit am Buch (Runde 7, „bugs“)

Ursache „Absatz wird Titel“: Das Satzbild schickte eine Überschrift mit einfachem Zeilenumbruch hinter dem Titel
(Umschalt+Enter oder `<br>` des Browsers, danach der Absatz) als **einen** Überschriftsblock; `baruch_wysiwyg::emit`
machte aus dem Umbruch ein Leerzeichen, und `outline.save` übernahm die ganze Zeile als Titel (Beleg:
`baruch-server/tests/round7_titles.rs` scheiterte vor dem Fix mit „Laub Anna's Laube steht im Herbst …“). Jetzt
endet eine Überschrift am ersten Umbruch (Server und Widget). Titelregeln überall (`check_title`: nicht leer, ohne
Zeilenumbruch, höchstens 200 Zeichen; RPC -32602, MCP `isError`, Formular 422), vorhandene leere Titel bleiben
lesbar. Bildpfade in Normalform (`normalize_asset_path`, `./` und `//`), Gliederungskopf mit Schemanamen
(„Nummern: Nur Kapitel römisch“), Bilder im Satzbild als Bild mit Unterschrift (`ws-figure`, byte-gleich beim
Speichern).

Wortlaut des Wunsches (Nacht 05./06.11.2026, gekürzt): Ein Gliederungspunkt wurde in einen ganzen Absatz
umbenannt; Umbenennen auf „“ ging durch; den leeren Abschnitt nicht automatisch entfernen; `./`-Pfade
normalisieren; Schemaname statt „I|1“; Bilder im Satzbild als Bild.

### Nach Runde 7 – Sichtprüfung (2026-11-06, erledigt in Runde 8)

- **Vorschläge wirklich an ihrer Stelle im Text:** Runde 7 zeigt die Vorschläge als Liste **unter** dem Editor
  (aufgeklappt bzw. als Linie in Persona-Farbe). Gewünscht ist die Einblendung **im Lesefluss an der Stelle**,
  die der Vorschlag ändert: der geänderte Absatz bzw. Block erscheint direkt beim Original, in Persona-Farbe
  hinterlegt, einklappbar zu einer Linie an genau dieser Stelle (im Satzbild wie im Quelltext-Lesemodus).
  Kopfzeile nur einmal (heute doppelt: „Lektor (@lektor) · Fassung 2“ und „Lektor @lektor · unbekannt …“).
- **Satzbild als Vorgabe prüfen:** Auf allen Bildschirmfotos (Team und eigene, frisches Firefox-Profil) ist nach
  dem Laden „Quelltext“ aktiv, obwohl die Vorgabe „satz“ ist. Ursache klären (Zeitpunkt des Bildschirmfotos vor
  dem Ende von `openSheet`, oder Fehler beim Öffnen) – mit einem Test, der das Widget nach dem Laden prüft.
- **Abschnitte über den festen Schlüssel adressieren (Fehler 2026-11-06):** MCP und Store nennen den festen Schlüssel
  (`key`, = `node.id`), die Oberfläche adressiert aber über `?node=<seq>` (laufende Nummer, ändert sich beim
  Umstellen). Folge: Eine andere Claude-Sitzung meldete Beiträge an „key 68“; `/?node=68` zeigt einen fremden
  Abschnitt, `/?key=68` wird nicht verstanden und fällt auf das erste Kapitel zurück – die Beiträge (an seq 59)
  schienen zu fehlen. Neu: `/?work=…&key=…` überall annehmen und in allen erzeugten Links verwenden (Baum, Treffer,
  Verweise, Einfälle, Vorschläge); `node=` nur noch als Rückfall. MCP-Antworten (`add_post`, `read_node`,
  `outline`, `search`) enthalten zusätzlich die direkte Adresse (`url`) des Abschnitts.
- **Beiträge verschieben und löschen (2026-11-06):** Beiträge im Abschnitts-Chat lassen sich heute nur anhängen.
  Gebraucht: „In Einfall umwandeln“ (legt Einfall mit Bezug an und entfernt den Beitrag, ein Commit), Beitrag
  löschen (Mensch, wiederherstellbar über den Spiegel) und an einen anderen Abschnitt verschieben – Oberfläche,
  RPC und MCP (MCP: nur eigene Beiträge des Clients bzw. der Persona). Anlass: Eine Claude-Sitzung legte
  Stichpunkte per `add_post` statt `add_idea` ab; sie wurden als Einfälle 1–10 übernommen, die Beiträge 8–10
  stehen weiter im Chat von Werk 2.
- **Abschnitts-Chat sichtbar machen:** Hat ein Abschnitt Beiträge, zeigt der eingeklappte Akkordeon-Kopf die Zahl
  deutlich (Abzeichen), und neue Beiträge seit dem letzten Besuch sind markiert.
- **Leerer Abschnitt im Buch des Nutzers:** Werk 2 hat seit der Nacht 05./06.11. einen Abschnitt ohne Titel unter
  „Einleitung“; nach den neuen Titelregeln lässt er sich erst nach Umbenennen speichern. Der Nutzer entscheidet.

## Vorschläge der Assistenten im Buchtext, in der Farbe der Persona (Runde 7, „proposals“)

Migration 0010 (`persona.color`, NULL = Palette nach Reihenfolge), Seite „Farben der Assistenten“
(`/einstellungen/personas`, RPC `persona.list`/`persona.set_color`, nur Mensch), Kontrast WCAG AA in hell und
dunkel (`tints`). Farbe in Assistentenliste, Chat, Fassungsliste, Revisionsbaum und Fassungskopf. Offene
Vorschläge als `<details>` im Lesefluss (Editor und Vorschau) mit Satzbild- bzw. Zeilen-Diff, „Übernehmen“ und
„Verwerfen“ ohne JavaScript, nach Persona gruppiert, einklappbar zur farbigen Linie (Cookie `baruch_folded`,
„Alle ein-/aufklappen“). Die Vorschlag-Karte und das Widget `wysiwyg-diff` sind entfernt.

### Einfälle mit Schlagwörtern (Runde 7, „ideas“)

Migration 0011 (`idea`, `idea_word`, `idea_fts`), Store `idea_*`, RPC `idea.*`, Seiten `/einfaelle` (Liste,
Schlagwort-Wolke, Filter, Volltextsuche, Gelöschte) und `/einfaelle/{id}` (Bearbeiten, Verarbeiten zu
Gliederungspunkt, Beitrag oder Notiz, Löschen, Wiederherstellen), Schnellerfassung „Einfall“ (Alt+E) in der
Kopfzeile, MCP-Werkzeuge `idea_create/read/list/search/update/delete` mit vollem CRUD auch für Assistenten
(IDEA_CRUD). Jede Aktion ein Commit mit Herkunft im Spiegel (`einfaelle.md` je Werk, `<data>/einfaelle/` ohne
Werk); ein gelöschtes Werk lässt seine Einfälle als Einfälle ohne Werk stehen. Offen: Lektor-Gedächtnis und
Persona-Auftrag (B2), Bedeutungssuche (oben „Nach Runde 7 offen“).

### Gliederungsschemata und Nummerierung je Ebene (Runde 6)

Migration 0008 (`work.numbering_scheme`, zwölf Schemata, beim Umstieg NULL → „aus der Werkvorlage“, `''` →
„ohne Nummern“, Muster → „eigenes Muster“). Seite „Gliederungsschema“ (`/werke/<id>/nummerierung`, aus „Werk
ändern“ und dem Gliederungskopf) mit Vorschau an der eigenen Gliederung, ohne JavaScript; RPC `numbering.*`.
Mustersprache `P`, `P|k`, `P1;P2;…`, verdoppelte Buchstaben. `heading_numbering` liest nur die letzte Regel der
obersten Ebene und erkennt Tiefenbedingungen in Funktionen (`I|1` = nur Kapitel römisch). Gliederung,
Editor-Kopf, Export und `main.typ` folgen dem Schema; „aus der Werkvorlage“ bleibt byte-gleich.

### Satzbild-Feinschliff (Runde 6)

`#szene[…]` und `#arbeit[…]` sind eigene Blöcke mit bearbeitbaren Absätzen (Anweisung `body`), ein Label hinter
der Überschrift gehört zur Überschrift; `wysiwyg.style` liest nur Regeln der obersten Ebene (10,5 pt und 1,1 em
statt Fußnotengröße); keine leere Fläche im Quelltext-Modus (`data-ws-open`); Klick in der Gliederung öffnet
immer das Satzbild (Cookie nur bei ausdrücklicher Wahl „Quelltext“); Einfügen mit Leerzeilen ergibt mehrere
Absätze; Bild-Inseln zeigen das Bild. Rundreise byte-gleich, neue Testdatei `szenen.typ`.

### Bilder und Grafiken als Base64-Text (Runde 6)

Migration 0009 (`asset`, `asset_path`, `asset_revision`, Inhalt immer TEXT: Base64 bzw. SVG-Text), Fassungen
wie Abschnitte (genau eine aktuelle, Umschalten, Ersetzen = neue Fassung, Ändern nur Bildunterschrift/
Alternativtext/Notiz, Löschen nur Mensch), Seite „Bilder“ ohne JavaScript, RPC `asset.*`, MCP nur lesend
(`list_assets`, `read_asset`), Import über die CLI liest die Bilder aus `image("…")` relativ zum Ordner
(nicht hinaus), `export --out` schreibt Text und Bilder byte-gleich, Spiegel `bilder/<name>` bzw.
`bilder/<name>.b64`, ein Commit je Bildänderung. Das Satzbild zeigt Bild-Inseln mit Alternativtext. Offen
geblieben (oben unter „Bilder: Erzeugung und Oberfläche“): Erzeugung, Teile der Oberfläche, Suche nach Name und
Bildunterschrift, Prüfung von Base64 in Typst.

### Eine Gliederung, Klick öffnet den Abschnitt im WYSIWYG-Editor (Runde 5)

**Erledigt in Runde 5 (Aufgabe „tree“):** Der Baum links ist die einzige Gliederung: Ziehen und Ablegen,
Alt+Umschalt+Pfeile, Kontextmenü (Rechtsklick, Umschalt+F10, Kontextmenü-Taste, „⋯“) mit Umbenennen (Doppelklick
bzw. F2, an Ort und Stelle), Anlegen davor/danach/darunter, Umstufen und Löschen (Entf, mit Rückfrage);
Nummerierung im Kopf des Baums; `/gliederung…` nur noch als Rückfall ohne JavaScript (mit JavaScript nicht
verlinkt). Ein Klick öffnet den Abschnitt im Editor (nie die Vorschau). Umschalten auf eine Fassung mit anderer
Ebene ✓: Die Gliederung folgt (Ebene und Titel), Unterpunkte werden mitgestuft (neue Fassungen nur mit
geänderter Überschriftszeile), ein Commit „… · Ebene a → b“; die Fassung wird nie umgeschrieben
(`Store::switch_to`, Nutzerentscheidung 2026-11-05 „Nein“). Verschieben behält die Ebene. Römische
Kapitelnummern auch aus einer Nummerierungsfunktion der Vorlage. Nicht im Browser geprüft (kein Browser im
Test): Kontextmenü, Doppelklick-Zeitfenster (300 ms), Umbenennen und Anlegen an Ort und Stelle, Fokus nach dem
Neuzeichnen.

**Erledigt in Runde 5 (Aufgabe „wysiwyg“):** Erster Schritt WYSIWYG ✓: Crate `baruch-wysiwyg` übersetzt
Abschnitte in Blöcke (Absatz, Überschrift, Hervorhebung, Fett, Fußnote, `#quote`, Liste, `#szene`, `#arbeit`,
Label, Maskierungen) und zurück; Nicht-Abbildbares bleibt als Typst-Insel. Rundreise byte-gleich (Tests über
alle Testdateien, eine eigene Datei mit allen Konstrukten, alle Fassungen des Beispielwerks, Eigenschaftstest;
Ende zu Ende über `/rpc` in `tests/round5.rs`). Satzbild nach der Vorlage (`wysiwyg.style`), Umschalter
„Satzbild | Quelltext“, Satzbild-Diff der Vorschlag-Karte (`docs/WYSIWYG.md`). Quelltext bleibt erreichbar,
ohne JavaScript nur Quelltext ✓. Im Browser noch nicht ausprobiert.

**Ursprünglicher Wunsch (Wortlaut, zum Nachlesen):**

Nach Runde 4 gibt es die Gliederung doppelt: den Baum links und dazu den Gliederungsmanager („bearbeiten“,
`/gliederung`). Neu:

- **Nur noch eine Gliederung**, oben links: Drag & Drop, höher/tiefer stufen (Alt+Umschalt+←/→), anlegen,
  umbenennen, löschen geschehen direkt im Baum (Kontextmenü bzw. Tasten am markierten Eintrag); die eigene
  Seite `/gliederung` bleibt nur als Rückfall ohne JavaScript.
- **Ein Klick (`onclick`) auf einen Eintrag öffnet den Abschnitt sofort zum Bearbeiten als WYSIWYG** – im Satzbild
  der Werkvorlage statt im Typst-Quelltext (siehe „WYSIWYG-Editor mit den Originalformaten“). Doppelklick bzw.
  F2 auf den Eintrag benennt um.
- **Erster Schritt WYSIWYG (machbar ohne vollen Typst-Satz im Browser):** Der Abschnitt wird aus Typst in
  bearbeitbare Blöcke übersetzt (Absatz, Überschrift, Hervorhebung, Fettdruck, Fußnote, Zitat, Liste,
  `#szene[…]`, `#arbeit[…]`) und mit dem CSS der Werkvorlage (Schrift, Farben, Einzüge) dargestellt; beim
  Speichern zurück nach Typst. Was sich nicht sicher abbilden lässt (eigene Funktionen, Code, Mathe), erscheint
  als nicht bearbeitbare „Typst-Insel“ und bleibt Zeichen für Zeichen erhalten. Rundreise: Laden und Speichern
  ohne Änderung ergibt byte-gleich denselben Typst-Text (Test über das ganze Beispielwerk und eine eigene
  Testdatei).
- **Quelltext bleibt erreichbar:** Umschalter „Satzbild | Quelltext“; ohne JavaScript nur Quelltext.
- Revisionen wie bisher: Speichern legt eine neue Fassung an; Vorschläge zeigen den Diff im Satzbild.
- **Umschalten auf eine Fassung mit anderer Ebene (Entscheidung 2026-11-05): Die Fassung bringt ihre Ebene mit.**
  Umschalten ist immer erlaubt; Gliederung (Ebene und Titel) folgt der Überschriftszeile der gewählten Fassung,
  deren Text bleibt unverändert (keine Anpassung der Fassung). Unterpunkte bleiben gültig (passt die neue Ebene
  nicht zu den Unterpunkten, werden sie mitgestuft; der Spiegel protokolliert beides in einem Commit).
  Die bisherige Sperre (`check_current_heading` beim Umschalten) entfällt.

### Werke: Auswahl, Import, CRUD (Runde 4/5)

**Erledigt in Runde 4 (2026-11-05):** Werkwahl öffnet sofort (`works.ts`, `<noscript>`-Knopf), „Importieren…“
als Dialog bzw. `/werke/neu` (Upload `.typ` bis 16 MiB, Titel vorbelegt, eindeutig), Seite `/werke` mit Liste,
Anlegen (Buch/Predigt), Ändern (Metadaten, Werkvorlage als Fassung), Löschen mit eingetipptem Titel (Spiegel
nach `works/.geloescht/`), Export als Download, Cookie `baruch_work`; nicht über MCP. ✓ Runde 5: Nach dem
Löschen aller Werke wird nicht neu gesät (`store_meta.seeded`, Migration 0007), Werk-ids werden nie
wiederverwendet (`AUTOINCREMENT`), slugs gelöschter Werke bleiben reserviert (`work_retired`), `work.import`
über `/rpc` nimmt wie das Hochladen 16 MiB Text an.

**Ursprünglicher Wunsch (Wortlaut, zum Nachlesen):**

- **Werkwahl ereignisgesteuert:** Das Auswahlfeld in der Kopfzeile öffnet das gewählte Werk sofort beim Wechsel
  (Widget, `change`), ohne „Öffnen“-Schaltfläche. Ohne JavaScript bleibt eine Schaltfläche „Öffnen“ in
  `<noscript>` als Rückfall.
- **Die Schaltfläche wird zur Import-Funktion:** „Importieren…“ öffnet einen Dialog (ohne JS: Seite `/werke/neu`):
  Typst-Datei hochladen (Formular `multipart/form-data`, Größengrenze, nur `.typ`), Titel vorbelegt aus
  `#set document`, änderbar (Titel eindeutig). Später auch Markdown, und Quellen (PDF, EPUB …) über MultiGPT.
- **Vollständiges CRUD für Werke** (Seite „Werke“ bzw. Dialog, ohne JS bedienbar, RPC und Doku):
  - **Anlegen:** leeres Werk (Art Buch/Predigt, Titel, Autor, Vorlage „Buch“/„Predigt“) oder per Import.
  - **Lesen:** Liste mit Titel, Autor, Art, Anzahl Abschnitte, offene Vorschläge, zuletzt geändert.
  - **Ändern:** Titel (eindeutig), Untertitel, Autor, Art, Zielgruppe, Bibelübersetzung, Werkvorlage
    (als neue Fassung der Vorlage, wie Abschnitte).
  - **Löschen:** nur der Mensch, mit Bestätigung (Titel eintippen); das Werk verschwindet aus der Datenbank,
    der Git-Spiegel wird nicht gelöscht, sondern nach `works/.geloescht/<slug>-<datum>/` verschoben
    (wiederherstellbar); Assistenten/MCP können Werke weder anlegen noch ändern noch löschen.
  - **Export:** Typst-Datei herunterladen (wie `baruch-server export`), später PDF/EPUB.
- **Zuletzt geöffnetes Werk** ist beim nächsten Aufruf ohne `?work=` vorausgewählt (Cookie `baruch_work`, wie
  die übrigen Einstellungen).

### Gliederungsmanager (Runde 4)

**Erledigt in Runde 4 (2026-11-05):** Migration 0006 (`node.numbered`, `work.numbering`, aufgelöste Titel),
RPC `outline.*`, Formulare ↑ ↓ ← → und „Bearbeiten“ in der linken Spalte, Seiten `/gliederung`, Ziehen und
Ablegen und Alt+Umschalt+Pfeile (`outline.ts`), Nummerierung nach Vorlage oder einstellbar, Überschriften im
Editor gleichen die Gliederung an (Teilen/Zusammenführen mit Rückfrage), jede Änderung ein Commit im Spiegel,
Export in neuer Reihenfolge. Auch ein übernommener Vorschlag mit anderer Überschrift derselben Ebene benennt
den Punkt um (`sync_heading`); beim Umschalten folgt die Gliederung seit Runde 5 der gewählten Fassung (siehe
oben, „Die Fassung bringt ihre Ebene mit“).

**Ursprünglicher Wunsch (Wortlaut, zum Nachlesen):**

Ausgangslage vor Runde 4: Die Gliederungsebenen ließen sich weder im Text noch über ein Werkzeug bearbeiten; die
Gliederung entstand beim Import und blieb dann starr.

- **Gliederungsmanager in der linken Spalte:** verwaltet die Ebenen (Teil, Kapitel, Abschnitt, Unterabschnitt;
  bei Predigten die Predigtteile) und die Reihenfolge.
  - **Drag & Drop:** Kapitel und Abschnitte per Ziehen verschieben – innerhalb der Ebene umordnen und in einen
    anderen Teil bzw. ein anderes Kapitel hängen. Ziel und Ebene werden beim Ziehen angezeigt; ungültige Ziele
    (z. B. ein Kapitel unter einen Abschnitt) werden abgewiesen. (✓ Runde 5: Verschieben behält die Ebene;
    „in einen anderen Teil bzw. ein anderes Kapitel hängen“ geht für Punkte, deren Ebene zum Ziel passt, sonst
    erst umstufen.)
  - **Ebene ändern:** höher/tiefer stufen (z. B. Abschnitt → Kapitel), mit Tastatur (Alt+Umschalt+←/→, wie in
    Textverarbeitungen) und Schaltflächen; Unterknoten wandern mit.
  - **Anlegen, umbenennen, löschen** von Gliederungspunkten (Löschen nur Mensch, mit Bestätigung; Inhalt bleibt
    über den Spiegel wiederherstellbar).
  - **Nummerierung automatisch** („2.1“, römische Kapitel wie in der Vorlage) nach jeder Änderung.
  - **Ohne JavaScript:** dieselben Aktionen als Schaltflächen je Eintrag (↑ ↓ höher tiefer umbenennen).
- **Inline im Text:** Ändert man im Editor die Überschriftszeile (`= …`, `== …`, `=== …`), passt sich die Gliederung
  beim Speichern an (Titel und Ebene); Gliederung und Überschriftszeilen sind immer gleich (✓ Nachbesserung
  Runde 4: Übernehmen, „Fassung ändern“ und `revision.save` lehnen abweichende Überschriften ab, MCP legt solche
  Vorschläge nicht an; beim Umschalten gibt es diese Sperre seit der Entscheidung vom 2026-11-05 nicht mehr,
  die Gliederung folgt der Fassung). Eine zusätzliche
  Überschrift im Text teilt den Abschnitt beim Speichern (mit Rückfrage), das Entfernen einer Überschrift fügt
  ihn mit dem vorigen zusammen (mit Rückfrage).
- **Versioniert:** Jede Gliederungsänderung ist ein Commit im Git-Spiegel (wer, was); Umstellen ändert keine
  Revisionsinhalte außer der Überschriftszeile (neue Fassung des betroffenen Abschnitts, Mensch).
- **Rundreise:** Export bleibt nach Gliederungsänderungen gültiges Typst in neuer Reihenfolge; ohne Änderung
  weiterhin byte-gleich zur importierten Datei.
- **Assistenten** ändern die Gliederung nicht; sie schlagen Umstellungen im Abschnitts-Chat vor (später als
  Gliederungs-Vorschlag mit Vorschau, den der Mensch übernimmt).
- RPC (`outline.move`, `outline.set_level`, `outline.rename`, `outline.add`, `outline.delete`) und Doku.

### Revisionsbaum (Runde 3)

**Erledigt in Runde 3 (2026-11-05):** Migration 0004 (Elternteil je Fassung, `edited`, `node.last_rev`,
neue Trigger), „aktuell“ als umschaltbare Markierung (`revision.set_current`), Ändern/Löschen nur durch den
Menschen (`revision.update`/`revision.delete`, nicht über MCP), jede Umschaltung, Änderung und Löschung ein
Commit im Spiegel (`Baruch-Action`), Baumansicht je Abschnitt/Kapitel, Vorschau mit `pick`.

**Ursprünglicher Wunsch (Wortlaut, zum Nachlesen):**

Ausgangslage vor Runde 3: Revisionen je Abschnitt fortlaufend nummeriert, genau eine `current`, `based_on` nur
bei Vorschlägen, Übernehmen ersetzte die aktuelle Fassung – ein veralteter Vorschlag verdrängte still eine
neuere eigene Fassung aus dem aktuellen Text. Neu:

- **Baum über das ganze Dokument, Ansicht mit Fokus auf das Kapitel:** Eine Revision ist ein Stand des
  Werks mit Elternteil (wie ein Commit) und ändert einen oder mehrere Abschnitte. Gezeigt und bearbeitet
  wird der Baum des aktuellen Kapitels (Äste, Zusammenführungen, wer/wann/Modell).
- **„Aktuell“ ist eine Markierung (Tag), kein Ersetzen:** Übernehmen setzt die Markierung auf die
  gewählte Revision – auch auf einen Vorschlag, der auf einer älteren Fassung beruht. Die andere Fassung
  bleibt erhalten und ist im Lesefluss als Alternative sichtbar (Hinweis am Abschnitt, mit einem Klick
  umschaltbar). Nichts geht verloren.
- **Genau eine Fassung wird erzeugt, jederzeit umschaltbar:** Ein Abschnitt hat beliebig viele Fassungen (keine Obergrenze; z. B. 5); genau eine
  trägt die Markierung „aktuell“ und wird bei der Erzeugung genommen (Vorschau, PDF, EPUB, Git-Spiegel `main`),
  z. B. die 3. Umschalten auf die 2 oder die 4 ist ein Klick (bzw. ein RPC-Aufruf durch den Menschen; über
  MCP gibt es kein Umschalten, Entscheidung Runde 3);
  dabei ändert sich keine Fassung, nichts wird zusammengeführt oder verworfen. „‹ n/m ›“ zeigt alle
  Fassungen, die erzeugte ist hervorgehoben.
- **Fassungen durchspielen:** In der Vorschau (Typst, später EPUB) lässt sich je Kapitel bzw. Abschnitt
  wählen, welche Revision gesetzt wird; so lassen sich Varianten nebeneinander lesen, ohne die Markierung
  „aktuell“ zu ändern.
- **CRUD für Revisionen:** anlegen, lesen, ändern, löschen.
  - Ändern und Löschen nur durch den Menschen; Assistenten (Personas, MCP-Aufrufer) legen nur Vorschläge an.
  - Jede Änderung und jedes Löschen wird als Commit im Git-Spiegel protokolliert (wer, was, vorher/nachher);
    Gelöschtes ist aus Baruch weg, aber aus der Git-Historie wiederherstellbar.
  - Löschen einer Revision mit Kindern: Kinder hängen am Elternteil der gelöschten Revision; die
    Markierung „aktuell“ kann nicht gelöscht werden, ohne sie vorher umzusetzen.
- **Folgen für Regeln und Code:** AGENTS.md („Revisionen nur anhängen, genau eine Current“) wird ersetzt durch
  „je Abschnitt genau eine aktuelle Fassung (Markierung, jederzeit umschaltbar); Ändern/Löschen nur Mensch, protokolliert im Spiegel“; die
  Trigger, die DELETE/UPDATE verbieten, und die Status-Trigger aus der Nacharbeit werden umgebaut;
  `Node::accept/reject/save_human`, RPC, MCP (`propose_revision`), Vorschlag-Karte und „‹ n/m ›“ folgen.
- **Git-Spiegel:** Hauptlinie (`main`) = Stand mit der aktuellen Fassung je Abschnitt (jedes Umschalten ein Commit); übrige Fassungen, offene Vorschläge und
  Alternativen als eigene Refs (`refs/revisions/<kapitel>/<n>`).

### Literatur als Popup (Runde 3)

**Erledigt in Runde 3 (2026-11-05):** Schaltfläche „Literatur“ (Popup mit Alt+L, ohne JavaScript Seite
`/literatur`), Treffer öffnen den Abschnitt an der Fundstelle (`#fundstelle`) bzw. die Dokumentansicht
`/quelle` am Zitat oder an der Hervorhebung, Hervorhebungen (Migration 0103, vier Arten, Personas nur als
Vorschlag), Zitat als Vorschlag-Revision. Noch Stub: Volltext aus MultiGPT und „an Persona geben“ (B2);
ein Weg, Persona-Hervorhebungen zu übernehmen, fehlt.

**Ursprünglicher Wunsch (Wortlaut, zum Nachlesen):**

Die Literatur ist kein Bereich der linken Spalte mehr, sondern ein eigenes Popup (Schaltfläche
„Literatur“ in der Kopfzeile, Tastenkürzel).

- **Popup:** Liste der Titel und Zitate mit Suche (FTS5). Mit JavaScript ein `<dialog>`-Widget über dem
  Arbeitsbereich; ohne JavaScript eine eigene Seite `/literatur?lq=…` mit denselben Links.
- **Klick auf einen Eintrag öffnet das Zieldokument an der Fundstelle:**
  - Treffer im eigenen Werk → Editor auf Abschnitt und Revision, Fundstelle markiert und angesprungen.
  - Titel/Zitat aus der Autorenbibliothek → Dokumentansicht der Quelle (PDF, EPUB, MOBI … wie MultiGPT
    sie liest: `read_document` mit Abschnitt bzw. Seite), an der Stelle des Zitats, Treffer markiert.
- **Zurück:** Popup bzw. Suche bleiben erhalten (Suchbegriff `lq` in der Adresse), damit man den
  nächsten Treffer öffnen kann.
- **Relevante Hervorhebungen (wie Citavi):** In jeder Quelle werden relevante Stellen hervorgehoben und
  gespeichert (Text, Fundstelle: Dokument, Abschnitt bzw. Seite, Zeichenbereich; Farbe/Art, z. B. Zitat,
  Kernaussage, Widerspruch, zu prüfen; Notiz; wer: Mensch oder Persona, Personas nur als Vorschlag).
  Hervorhebungen erscheinen im Popup als eigene Einträge, sind durchsuchbar (FTS5, später LanceDB) und
  lassen sich als Zitat bzw. `@cite` in einen Abschnitt übernehmen. Beim Öffnen des Zieldokuments sind
  alle Hervorhebungen sichtbar, die angeklickte ist angesprungen. Der Lektor nutzt sie als
  Gedächtnis („dazu hast du in … hervorgehoben …“).
- **Eigenes Kontextmenü für markierten Text:** In der Dokumentansicht der Literatur öffnet ein
  Rechtsklick (bzw. langes Tippen) auf markierten Text ein eigenes Kontextmenü, das den markierten
  Text samt Fundstelle weitergibt (delegiert):
  - als Hervorhebung speichern (Art wählen, Notiz),
  - als Zitat bzw. `@cite` in den aktuellen Abschnitt übernehmen (als Vorschlag-Revision),
  - an eine Persona geben (`@exeget` deuten, `@schrift` Wortlaut prüfen, `@belege` Quelle prüfen,
    Lektor: „wo im Werk passt das?“) – Antwort landet im Abschnitts-Chat mit Verweis auf die Fundstelle,
  - in Werk und Bibliothek nach ähnlichen Stellen suchen.
  Ohne JavaScript: dieselben Aktionen über ein Formular unter der Dokumentansicht (Text einfügen).
- Löst die heutige Literatur-Spalte ab; die Gliederung links bekommt den Platz zurück.

### Kleinigkeiten (Runde 3–5)

**Erledigt in Runde 3 (2026-11-05)**, außer „Werktitel eindeutig“ (✓ Runde 4) und der Prüfung mit `typst compile`
(typst nicht installiert, weiter offen): Datenordner, Port 8765, MCP ohne Import, Vorlage im Spiegel, slug,
Beispielwerk, FIFO und Parser-Grenzen, Begründungen an den Zusicherungen.

### Kleinigkeiten, einzeln

- ✓ **Datenordner (Entscheidung 2026-11-05):** `baruch_store::default_data_dir()` nimmt `dirs::data_local_dir()`
  statt `dirs::data_dir()`: Windows `C:\Users\<name>\AppData\Local\baruch` (nicht Roaming – servergespeicherte
  Profile der Domäne würden SQLite-WAL und Git-Spiegel bei An-/Abmeldung kopieren), Linux unverändert
  `$XDG_DATA_HOME/baruch` bzw. `~/.local/share/baruch`, macOS unverändert `~/Library/Application Support/baruch`.
  `--data`/`BARUCH_DATA` überschreiben weiterhin. Doku (`docs/Bedienung.md`, SKILLS.md) nachziehen.
- ✓ **Port 8765 (Entscheidung 2026-11-05):** Vorgabe von `--listen` wird `127.0.0.1:8765` statt `:8740`
  (Server-Code, `--help`, Tests, `docs/*.md`, SKILLS.md, HANDOFF.md); die systemd-Unit nutzt 8765 schon.
  MCP-Eintrag in Claude Code beim Umstieg neu setzen: `claude mcp add -s local --transport http baruch
  http://127.0.0.1:8765/mcp`. `grep -rn` nach dem alten Port muss danach leer sein (außer Historie).
- ✓ **MCP-Import (Entscheidung 2026-11-05):** `import_typst` wird aus dem MCP entfernt. Ein Import legt aktuelle
  Revisionen unter dem Namen des Menschen an – das darf ein Assistent nicht (AGENTS.md: Assistenten nur
  Vorschläge). Import nur über die Kommandozeile (`baruch-server import`) und später über die Oberfläche.
- ✓ **Vorlage im Spiegel (Entscheidung 2026-11-05):** Der Spiegel legt die Werkvorlage als `vorlage.typ` ab;
  `main.typ` beginnt mit dem Vorlagentext (Set-/Show-Regeln gelten so für alles Eingebundene), jede
  Abschnittsdatei beginnt mit `#import "../vorlage.typ": *` (damit `#let`-Funktionen wie `#szene`/`#arbeit`
  dort gelten). Der Spiegel ist eine Projektion; die Revision in SQLite bleibt ohne diese Zeile. Prüfen mit
  einem installierten `typst compile main.typ` (typst als Werkzeug installieren oder eingebettet setzen).
- ✓ **slug:** an einer Wortgrenze kürzen (höchstens 40 Zeichen), Kollisionen weiter mit `-2`.
- ✓ **Beispielwerk umbenennen:** `sample::work()` bekam einen eigenen Titel mit dem Vorsatz „Beispielwerk:“ (seit der Nachbesserung R24 „Beispielwerk: Leise Abhängigkeit (Auszug)“, ohne Bezug auf ein echtes Buch), damit es sich vom echten
  Buch unterscheidet; wird nur in eine leere Datenbank gesät.
- ✓ **Kleine Befunde aus B1.5 (HANDOFF.md):** FIFO-Pfad im Import (`File::open` nach `is_file`/`spawn_blocking`),
  Parser-Grenzen (mehrzeilige `#block[…]`, Raw, Strings, auskommentiertes `#set document`, `//` hinter
  Überschriften), Fehlermeldung „nicht beides“, Zusicherungen ohne Begründung.
- ✓ **Werktitel eindeutig (Entscheidung 2026-11-05, erledigt in Runde 4 mit Migration 0005):** Vorher war nur `work.slug` UNIQUE, `title` nicht – die Datenbank
  des Nutzers hatte zweimal denselben Buchtitel (Werk 1 = Beispielwerk, Werk 2 =
  echtes Buch). Neu: Titel je Bibliothek eindeutig (getrimmt, ohne Groß-/Kleinschreibung; `UNIQUE` auf einem
  normalisierten Ausdruck bzw. Spalte `title_key`), Import mit doppeltem Titel wird abgelehnt (Hinweis auf das
  vorhandene Werk, `--title "…"` zum Ausweichen). Migration für bestehende Doppel: das gesäte Beispielwerk heißt
  danach den Titel des Beispielwerks, weitere Doppel bekommen „ (2)“, „ (3)“ … – nur Titel ändern sich,
  slug und Spiegel bleiben. Die Werkwahl zeigt zusätzlich Autor und Anzahl Abschnitte.
- ✓ (Runde 4, Migration 0006) Typst-Maskierungen in Überschriften werden nicht aufgelöst: Gliederung zeigt „Anna\'s Laube“ statt
    „Anna's Laube“ (Titel aus der Überschriftszeile ohne Auflösung von `\'`, `\"`, `\#` usw.); der
    Revisionstext bleibt unverändert, nur Anzeige/slug/Commit-Betreff.
- ✓ (Runde 4, `load.rs`, `outline.set_numbering`) Kapitelnummern in der Gliederung arabisch („1“, „2“), die Vorlage setzt römisch („I“, „II“) – Nummerierung
    nach der Werkvorlage (`#set heading(numbering: …)`) bzw. im Gliederungsmanager einstellbar.
- ✓ (Runde 5) **MCP `key` wird Pflicht:** `propose_revision` und `add_post` (auch jeder Verweis in `refs`)
    adressieren Gliederungspunkte nur noch über den festen Schlüssel; `node` (die veränderliche Stelle `seq`)
    oder ein fehlender `key` → -32602. Ebenso `highlight.cite {highlight, key}` und `/literatur/act` mit `cite`
    (ohne `key` → 422); `/literatur` und `/quelle` tragen den Schlüssel selbst. Links `/?node=` bleiben Anzeige.
- ✓ (Runde 5, Migration 0007) **Werk-ids werden nie wiederverwendet** (`AUTOINCREMENT`); slugs gelöschter
    Werke bleiben **dauerhaft** reserviert (`work_retired`; PM-Entscheidung, weil die Datenbank den
    Archivordner nicht kennt).
- ✓ (Runde 5) **Saat:** Teile des Beispielwerks ohne Text (`""`, wie `outline.add` bei Teilen). Nach dem
    Löschen aller Werke wird **nicht** neu gesät, auch nicht nach einem Neustart (`store_meta.seeded`); nur
    eine wirklich neue Datenbank bekommt das Beispielwerk.
- ✓ (Runde 5) **Verschieben:** Ein Gliederungspunkt behält beim Verschieben seine Ebene (Kapitel bleibt Kapitel), außer
    der Nutzer stuft ausdrücklich um.
- ✓ (Runde 5, `heading_numbering` erkennt `numbering("…")` in Funktionen) **Römische Kapitelnummern fehlten:** Die Vorlage des Buchs setzt `#set heading(numbering: (..n) =>
    …numbering("I", ..n)…)` über eine eigene Funktion; die Erkennung greift dabei nicht, die Gliederung zeigt
    arabisch. Erkennung erweitern oder Nummerierung je Werk im Gliederungsmanager einstellbar machen.
