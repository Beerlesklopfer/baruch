# Baustein Runde 37 – Aufgabe bilder

## CHANGELOG

Für „Unveröffentlicht“, Abschnitt „Hilfe auf der Webseite (Runde 37)“:

- Menü **Baruch ▾ → Hilfe** öffnet die Hilfe auf der Webseite in einem neuen Fenster (gewöhnlicher Link, auch ohne
  JavaScript und für Lektoren; der Rechner braucht dafür das Internet – Baruch selbst ruft nichts ab). Die Seite
  „Über Baruch“ nennt die Adresse der Hilfe unter der Webseite.
- Werkzeug `site/tools/bildschirmfotos.py` neu gefasst: Die Bilder der Hilfe kommen aus der Liste
  `site/data/hilfebilder.toml` (94 Namen, je Bild Adresse, Fenster, Ausschnitt, Schritte, erwartete Texte, Stufe);
  die sieben Bilder der Seite „Funktionen“ entstehen weiter mit. Neu: `--abgleich` (verwendete Namen gegen die
  Liste, fehlende Dateien, Größe gegen 20 MB), `--nur`, `--sword`, `--breite`, `--ohne-palette`; `--pruefen` baut
  das Beispielwerk ganz auf und belegt jede Aufnahme (200, Inhalt, erwartete Texte, Wirkung der Schritte).
- Ein Vermittler im Skript (vor dem Wegwerf-Server) ergänzt Cookies und Sitzung, öffnet `<details>`, fügt Klick- und
  Mess-Skript ein und liefert Antworten von Formularen aus; je Aufnahme ein eigenes Firefox-Profil, dunkle Fassungen
  für Einträge mit `dunkel = true`, Nachbearbeitung mit ImageMagick (Ausschnitt, Breite, 256 Farben).
- Beispielwerk „Ein Jahr auf Mövenoog“ erweitert (Fußnote, Blockzitat, Liste, Bibelstelle); alles Weitere legt das
  Skript über RPC, MCP und Formulare des eigenen Servers an (zweite Fassung, gezeichnete Bilder, Register, Glossar,
  Buchangaben, Einfälle, Beiträge, Vorschlag, Gliederungsantrag, Faden, Quelle, Archiv, Konten, Befunde).

## Backlog/Offen

- **Fotos fehlen noch** (Aufgabe des Integrators, `--fotos` nach dem vollen Lauf). Bis dahin lassen die Seiten die
  Bilder aus. Was erst die Fotos zeigen, steht in `site/tools/README.md` („Nicht belegt“): Wirkung der Klicks
  (`einfaelle-festhalten`, `einstellungen-popup`), der geschätzte feste Ausschnitt `band-eingeklappt`, ob Anker den
  Block ins Bild rücken, die Gesamtgröße.
- **Versehentlicher Firefox-Lauf der Aufgabe bilder:** Eine Probe von `--fotos` sollte mit einer Attrappe laufen;
  die Attrappe war nicht ausführbar, das Skript fand darum den echten Firefox und machte sieben Aufnahmen (headless,
  eigene Wegwerf-Profile, Ziel unter `target/`, nichts eingecheckt, Dateien gelöscht) – während des ersten vollen
  Testlaufs. Der Lauf scheiterte aus anderem Grund (zwei Zusicherungen zur Über-Seite) und wurde ganz wiederholt.
- In der dunklen Arbeitsseite steht rechts neben dem Abschnitts-Chat ein heller Streifen (Rollbalken des Chats);
  gesehen in der versehentlichen Aufnahme, nicht untersucht (Stylesheet der Oberfläche, nicht Besitz dieser Aufgabe).
- **Bilder mit Bibelmodul:** `bibel-nachschlagen` braucht `--sword ORDNER` mit echten oder erzeugten Modulen;
  geprüft ist nur, dass die Seite mit einem leeren Ordner 200 liefert. Ohne Module zeigen `bibel-zitate-pruefen`
  („ungeprüft“) und `bibel-einstellungen` (keine Module) den ehrlichen Leerstand. Ein Modul aus frei erfundenem Text
  ließe sich mit `baruch_sword::fixture` erzeugen (SKILLS.md, „Handprüfung mit SWORD-Testbibel“) – das wäre ein
  kleines Beispielprogramm außerhalb des Besitzes dieser Aufgabe.
- **Vertrag nennt „95 Namen“, die Tabelle §6 und S1 haben 94.** Liste, Test (`NAMES`) und README zählen 94; es
  fehlt kein Name der Tabelle.
- **Zweig:** `team/bilder` gab es schon (Runde 21c, Stand 02fe1e6, in development enthalten). Die Arbeit liegt
  darum auf `team/bilder-r37`; der alte Zweig ist unberührt.
- `verweise-seite` zeigt „Keine kaputten Verweise“ (ein kaputter Verweis bräche den Satz des Beispielwerks);
  `assistenten-websuche` zeigt die Belege-Seite eines Beitrags ohne Websuche; `einstellungen-mcp` zeigt die Seite
  ohne Token. Wünscht die Hilfe dort mehr, braucht es einen Nachbau (Websuche) bzw. eine Entscheidung (Token-Anfang
  im Bild).
- `installation-erster-start`: Das Bild zeigt das mitgelieferte Beispielwerk im selben Wegwerf-Ordner; in der
  (zugeklappten) Werkauswahl steht auch das Mövenoog-Werk. Ein wirklich frischer Ordner bräuchte einen zweiten
  Wegwerf-Server.
- Die sieben Bilder der Seite „Funktionen“ zeigen nach neuen Fotos mehr Inhalt (Beiträge im Chat, Bild im Werk).

## SKILLS

- **Befunde ohne Modell gibt es nur über ein Lektor-Konto:** `finding.add` und `revision.propose` sind den Lektoren
  vorbehalten (-32000 für alle anderen). Für Fotos also: Konten zuletzt anlegen, als Lektor anmelden, Befunde
  schreiben; die Bilder mit Befunden zeigen dann die angemeldete Autorin (Sitzung über den Vermittler).
- **Beiträge und Vorschläge einer Assistenten-Rolle ohne Modell:** MCP am eigenen Server von 127.0.0.1 ohne Token,
  `add_post` bzw. `propose_revision` mit `persona: "lektor"` → Autor `@lektor`. `thread.post` und MCP `chat` rufen
  kein Modell, solange die Nachricht keinen Assistenten erwähnt (`answers` und `errors` bleiben leer).
- **Wegwerf-Server und Bibelmodule:** Ohne gespeicherte Liste liest Baruch `$SWORD_PATH`, `~/.sword` und
  `/usr/share/sword` – also die Module des Nutzers. Für Fotos gleich nach dem Start `bible.set_dirs` auf einen
  leeren Ordner (mit `mods.d`) setzen.
- **Vorschauen des Einlesens sind teils Seiten, teils Antworten:** `POST /werke/import/word/vorschau` antwortet mit
  303 auf die Seite des Entwurfs, `POST /austausch/vorschau` (Dateifeld `datei`, nicht `file`) und
  `POST /werke/import/quelle/vorschau` mit 200. Der Bericht nach `POST /werke/import` erscheint nur, wenn es etwas
  zu berichten gibt (etwa ein fehlendes Bild in einer Markdown-Datei), sonst 303 auf das Werk.
- **Vermittler für Fotos als Teil des Werkzeugs** (`site/tools/bildschirmfotos.py`, Klasse `Relay`): Rezept je
  Aufnahme über `_bild=<Aufnahme>` in der Adresse; Sitzung nur zum Server hin; `Set-Cookie` für die übrigen Cookies
  auch an den Browser; Host/Origin/Referer umschreiben (Gegenprobe: dieselbe Anfrage direkt → 403). Ohne Firefox
  prüfbar ist alles außer der Wirkung von Klick und Messung.
- **Dunkel im HTML prüfbar:** Mit Cookie `baruch_theme=dark` trägt `<html>` `data-theme="dark"`.
- **Worktree-Schutz und Shell:** Zusammengesetzte Befehle mit Hier-Dokument lehnt der Schutz als „zu komplex“ ab;
  Hilfsskripte mit dem Schreib-Werkzeug ins Scratchpad legen (Name mit Aufgaben-ID) und einzeln aufrufen.
- **Attrappe statt Firefox:** Wer `--fotos` ohne Firefox proben will, prüft vorher mit `command -v firefox`, dass
  die Attrappe im Pfad vorn steht **und ausführbar ist** – `shutil.which` überspringt nicht ausführbare Dateien und
  findet dann den echten Browser.
- **Vorlagen maskieren Schrägstriche:** minijinja schreibt eingesetzte Adressen als `https:&#x2f;&#x2f;…`; Tests,
  die einen Link mit eingesetzter Adresse suchen, ersetzen vorher `&#x2f;` durch `/`.
- **`| head` an einem Python-Werkzeug:** `BrokenPipeError` abfangen, sonst endet `--liste | head` mit einer
  Fehlermeldung.

## Nutzerfragen

1. **Bilder mit Bibelmodul:** Soll `bibel-nachschlagen` (und damit auch `bibel-zitate-pruefen` mit geprüftem
   Wortlaut) mit einem Modul aus frei erfundenem Text entstehen (kleines Beispielprogramm nötig) oder mit einem
   gemeinfreien Modul aus dem Ordner des Nutzers (`--sword ORDNER`, dann steht dessen Name im Bild)?
2. **MCP-Zugang im Bild:** Die Seite wird ohne Token fotografiert. Soll ein Token „Beispiel“ in der Liste stehen
   (dann ist sein Anfang, wie in der Oberfläche üblich, lesbar – das Token stirbt mit dem Wegwerf-Ordner)?
3. **Bilder der Seite „Funktionen“:** mit erneuern (mehr Inhalt im Beispielwerk) oder die sieben vorhandenen
   behalten?
