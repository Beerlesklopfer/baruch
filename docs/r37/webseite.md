# Baustein Runde 37 – Aufgabe `webseite`

Zweig `team/webseite`. Kein Rust geändert, kein cargo (Vertrag §3). Geprüft mit `hugo --minify --panicOnWarning
-s site`, `python3 scripts/site-check.py` (auch `--selbsttest`), `sh scripts/ci-selftest.sh` (84 Zusicherungen,
0 gescheitert) und `sh scripts/ci-fingerprint.sh`.

## CHANGELOG

Für „Unveröffentlicht“, Abschnitt „Hilfe auf der Webseite (Runde 37)“:

- **Betrieb auf der Webseite:** `/betrieb/` ist jetzt die vollständige Beschreibung für Betreiber – Datenordner,
  Sicherung und Wiederherstellen, Dauerbetrieb und Aktualisieren, Erreichbarkeit in drei Stufen, Reverse-Proxy
  mit nginx und Caddy samt Pflichtprobe, Anmeldung und Konten, `/mcp` mit Token, Verbindungen nach außen, TLS zu
  pgvector. Fachwörter sind beim ersten Vorkommen erklärt, Geprüftes und Angenommenes bleibt getrennt
  (Abschnitt „Was nicht erprobt ist“). `docs/Betrieb.md` ist nur noch Verweis, „Wo steht was“, Stand der
  Prüfung, Offenes und technische Anmerkungen.
- **Ersteinrichtung Schritt für Schritt:** Installation → „Nach der Installation“ (erster Start, Bibeln
  einrichten: woher Module kommen, wohin der Ordner gehört, wo man ihn einträgt, wie man die Bibel des Werks
  wählt; Aktualisieren). MCP und Assistenten → „Assistenten einrichten“ (Adresse, Schlüssel im Schlüsselbund,
  Verbindung prüfen, Modell je Assistent, ausprobieren), „MultiGPT einrichten“, „KI-Programme mit Baruch
  verbinden“ (eigener Rechner, eigenes Netz, Internet).
- **Startseite, Funktionen, Installation, MCP in Alltagssprache;** alle Verweise auf die alte Seite Bedienung und
  auf `docs/Bedienung.md` bzw. `docs/Betrieb.md` bei GitHub führen jetzt in die Hilfe bzw. auf `/betrieb/`. Jede
  Überschrift dieser Seiten hat einen festen Anker; die Zeile „Direkt zu“ der Installation zeigte vorher für
  macOS auf einen Anker, den es nicht gab.
- **Hilfe im Theme:** Startseite der Hilfe als Liste der Seiten mit Vorspann; jede Hilfeseite mit der Navigation
  des Abschnitts (aktuelle Seite markiert), einem Inhaltsverzeichnis aus den Überschriften und „vorige/nächste
  Seite“ – ohne JavaScript. „Hilfe“ im Hauptmenü ist auch auf den Unterseiten markiert. Stylesheet für die
  Bilder der Hilfe (nie breiter als die Spalte, Rahmen, Unterschrift), Tastenangaben, Tabellen und Navigation auf
  Telefonbreite (waagerecht rollbar), Druck. Eine Suche gibt es nicht: Sie ginge nur mit JavaScript oder über eine
  fremde Adresse.
- **Neuer Shortcode `taste`:** `{{< taste "Strg+S" "⌘S" >}}` ergibt „Strg+S (Mac: ⌘S)“.
- **Linkprüfung `scripts/site-check.py`** (nur Python-Standardbibliothek): tote interne Links und Anker, Bilder
  ohne alt-Text, JavaScript, fremde Adressen für Stil, Schrift oder Bild, Links auf `docs/Bedienung.md` und
  `docs/Betrieb.md` bei GitHub, GitHub-Links im Text der Hilfe, Weiterleitung `/bedienung/` → `/hilfe/`;
  `--selbsttest` prüft das Skript an einem erfundenen Mini-Baum (18 Zusicherungen). `scripts/ci-site.sh` ruft es
  nach dem Hugo-Bau auf; `scripts/ci-selftest.sh` hat dazu vier neue Zusicherungen.

## Backlog/Offen

- **Reihenfolge beim Zusammenführen (wichtig):** Auf dem Stand von `team/webseite` allein meldet
  `site-check.py` 28 tote Anker – ausschließlich die festen Anker der Hilfe aus Vertrag §5
  (`/hilfe/erste-schritte/#starten`, `#beenden`, `#neues-werk`, `/hilfe/einlesen/#word`,
  `/hilfe/menueband/#laptop-modus`, `/hilfe/assistenten/#chat`, `#befunde`,
  `/hilfe/bibel-quellen/#bibeln-waehlen`, `#quellen`, `/hilfe/buchsatz/#pdf`, `#epub`, `#umschlag`,
  `/hilfe/austausch-sicherung/#sicherung`, `/hilfe/einstellungen/#verbindungen`, `#assistenten`, `#mcp-zugang`,
  `/hilfe/konten/#einrichten`, `#lektoren`), die es erst mit `hilfe_text` gibt. Gegenprobe im Scratchpad: Kopie
  von `site/`, in der die Platzhalterseiten diese Anker tragen → Hugo ohne Warnung, `site-check.py` ohne Fund.
  **`scripts/ci-site.sh` (und damit die CI) scheitert also, bis `hilfe_text` zusammengeführt ist** und die Anker
  liefert. Fehlt nach dem Zusammenführen einer, nennt `site-check.py` ihn.
- **python3 in der CI:** `ci-site.sh` braucht jetzt `python3` (Variable `PYTHON`); `.github/**` gehört dem
  Integrator. Die Aufrufe stehen in `ci.yml` (Z. 126), `release.yml` (Z. 490) und `pages.yml` (Z. 80/82) – bitte
  nachsehen, dass die Jobs dort python3 haben (auf den üblichen Ubuntu-Läufern vorhanden; nicht geprüft).
- **Bild `bibel-einstellungen`:** Vertrag §6 nennt als Ort „Einstellungen → Werk“. Nach dem Code stehen
  Modulordner, Modultabelle und „Bibel des Werks“ unter **Einstellungen → Quellen** (`templates/settings/quellen.html`,
  Abschnitt „Bibeln (SWORD)“); unter Einstellungen → Werk steht nur „Kommentare und Lexika“. `installation.md`
  beschreibt den Weg nach dem Code, alt-Text und Unterschrift des Bildes ebenso – das Foto sollte die Seite
  Quellen zeigen.
- **Alt-Texte vor den Fotos geschrieben:** Die acht `hilfebild`-Aufrufe (`einstellungen-daten`,
  `konten-einrichten`, `konten-einstellungen` in Betrieb; `einstellungen-verbindungen`, `einstellungen-assistenten`,
  `einstellungen-mcp` in MCP; `installation-erster-start`, `bibel-einstellungen` in Installation) beschreiben,
  was das Bild nach §6 zeigen soll. Nach `--fotos` bitte gegen die Bilder lesen.
- **Sichtprüfung offen** (macht der Integrator): Hilfeseite hell, dunkel, Telefonbreite, Druck. Die Regeln stehen
  im CSS, gesehen hat sie niemand. Annahme im CSS vermerkt: Browser drucken die helle Fassung eines Bildes.
- **Keine Suche** in der Hilfe (ohne JavaScript und ohne fremde Adresse nicht einfach möglich). Ersatz:
  Seitenliste, Navigation des Abschnitts, Inhaltsverzeichnis je Seite.
- **Shortcode `taste`** kennt `hilfe_text` nicht (parallel entstanden); `tasten.md` kann ihn nachträglich
  verwenden. Ohne ihn bleiben Tastenangaben gewöhnlicher Text – richtig, nur ohne Tastenoptik.
- **Kein cargo-Lauf** in dieser Aufgabe (Vertrag: `webseite` baut nicht mit cargo). Die Zusicherungen der Tests,
  die meine Dateien lesen, sind von Hand nachvollzogen: `docs/Betrieb.md` ohne „Werk ▾“, „Ansicht ▾“,
  „Werkzeuge ▾“ und ohne die Suchwörter der Verbotslisten; `site/content/mcp.md` ohne die frühere Adresse;
  `baruch-typst compile` in `installation.md` (2×) und `funktionen.md` (1×); in `hugo.toml` nur der Kommentar
  und `[markup.tableOfContents]` geändert. Den vollen Lauf macht der Integrator (§8).
- **Nicht geprüft, so auch auf den Seiten vermerkt:** andere MCP-Clients als Claude Code; wo BibleTime und Xiphos
  ihre Module unter macOS und Windows ablegen; Aktualisieren über eine vorhandene Installation unter macOS und
  Windows; alles, was schon in `docs/Betrieb.md` als Annahme stand.
- **TLS zu pgvector:** Der Befund stammte aus sqlx 0.8.6; seit Runde 36 läuft 0.9.1. Die Stelle
  (`sqlx-postgres-0.9.1/src/connection/tls.rs`) ist nachgelesen und unverändert (`require` nimmt jedes Zertifikat
  an); steht so in `docs/Betrieb.md`. Gegen einen Server weiter ungeprüft.
- **Zweig:** Es gab schon einen Zweig `team/webseite` aus Runde 24 (285b58e, nicht Vorfahr von `development`,
  aber im Zweig `historie-bis-0.9.0` enthalten). Er wurde auf `development` neu gesetzt; verloren ist nichts.

## SKILLS

- **Feste Anker statt erzeugter:** Hugo bildet Anker aus der Überschrift (`#macos-nicht-signiert` gab es nie,
  weil die Überschrift „macOS“ hieß). Jede Überschrift bekommt `{#anker}`; `scripts/site-check.py` findet tote
  Anker nach dem Bau.
- **Parallel zugesagte Anker prüfen:** Verweist ein Zweig auf Anker, die ein anderer erst liefert, eine Kopie
  von `site/` im Scratchpad mit erfundenen Abschnitten samt diesen Ankern bauen und dort prüfen – so ist belegt,
  dass auf dem eigenen Stand nur das Zugesagte fehlt.
- **Hugo, Abschnittsvorlagen:** `layouts/hilfe/page.html` gilt nur für Seiten unter `content/hilfe/`;
  `layouts/section.html` für die Startseite des Abschnitts. Das Inhaltsverzeichnis nur zeigen, wenn es Einträge
  hat: Bedingung `in .TableOfContents "<li"` (auf den Platzhalterseiten ohne Überschrift erscheint so kein Kasten).
- **Menüeintrag auf Unterseiten markieren:** `IsMenuCurrent` trifft nur die Seite selbst; für die Unterseiten
  eines Abschnitts `$seite.IsDescendant .Page` am Menüeintrag (nur wenn `.Page.IsSection`).
- **Die Sandbox lehnt Heredocs mit bestimmtem Inhalt ab** („too complex to verify“): Dateien mit dem
  Schreibwerkzeug anlegen, längere Ersetzungen als Python-Skript im Scratchpad ablegen und ausführen.
- **`ci-selftest.sh` baut Probe-Seiten ohne Abschnitt Hilfe:** Prüfungen, die die echte Seite voraussetzen,
  brauchen einen Schalter (`site-check.py --ohne-hilfe`, von `ci-site.sh` gesetzt, wenn `content/hilfe` fehlt) –
  und eine Gegenprobe, dass die Prüfung mit dem Abschnitt wirklich greift.

## Nutzerfragen

1. **AGENTS.md** (Vertrag §1): Die Zeile „Bedienung (`docs/Bedienung.md`)“ sollte künftig lauten: „Bedienung
   (`site/content/hilfe/`, Einstieg `docs/Bedienung.md`), Betrieb (`site/content/betrieb.md`)“.
2. **Hinweistexte im Programm:** `src/main.rs` (Hilfe zu `--allow-host`, „Siehe docs/Betrieb.md“),
   `templates/settings/mcp.html` und `docs/MCP.md` nennen weiter `docs/Betrieb.md`. Die Datei führt jetzt auf
   die Webseite. Sollen diese Stellen künftig gleich `https://baruch.bernau.family/betrieb/` nennen?
3. **Wortlaut zu „Eliezer“:** `mcp.md` erklärt Eliezer als „die fest eingebaute erste Verbindung“ zu einem
   Modelldienst unter `http://127.0.0.1:1234` und nennt kein bestimmtes Programm dahinter. Ist das so gewollt,
   oder soll die Seite ein Programm empfehlen bzw. verlinken?
4. **MultiGPT:** `mcp.md` nennt MultiGPT „einen eigenen, selbst betriebenen Dienst für die Suche in
   Dokumentsammlungen und im Web“, ohne Bezugsquelle. Gibt es eine Adresse, auf die die Seite verweisen soll?
5. **Bilder mit Bibelmodul** (aus dem Vertrag): `bibel-nachschlagen` braucht `--sword ORDNER`; die Seiten der
   Aufgabe `webseite` verwenden es nicht.
6. **`site/README.md`, Impressum:** Der Satz, das Impressum sei ein Platzhalter (`platzhalter: true`), stimmte
   nicht mehr (die Seite trägt die Angaben und kein Kennzeichen); er ist berichtigt. `impressum.md` selbst ist
   unverändert.
