---
title: "Baruch"
claim: "Freie Schreibwerkstatt für Bücher und Predigten"
lead: "Baruch ist eine Schreibumgebung für Predigten, Vorträge und Bücher – vom ersten Einfall über die Bibelarbeit bis zum druckfertigen Buch, ohne die Kontrolle an eine KI abzugeben. Jede Fassung bleibt erhalten, Assistenten lesen mit und schlagen vor, das letzte Wort behält der Mensch."
description: "Freie Schreibwerkstatt für Bücher und Predigten: Fassungen, Buchsatz mit Typst und Assistenten, die nur vorschlagen. AGPL-3.0, läuft auf dem eigenen Rechner."
---

> Benannt nach Baruch, dem Schreiber Jeremias, der die verbrannte Rolle neu und erweitert niederschrieb
> (Jer 36,32). Fassungen sind der Kern.

## Schreiben, prüfen, setzen {#schreiben-pruefen-setzen}

- **Schreiben:** Predigt oder Buch als Gliederung aus Teilen, Kapiteln und Abschnitten. Der Text erscheint so,
  wie er gedruckt wird, und jede Fassung bleibt erhalten – zurück zur gestrigen geht mit einem Klick.
- **Prüfen:** Bibelstellen und Zitate werden mit der gewählten Bibelübersetzung abgeglichen. Ein Lektor, ein
  Exeget oder ein kritischer Hörer lesen auf Wunsch mit und schlagen Verbesserungen vor; für Predigten zeigt
  ein Zeitwächter die Sprechdauer.
- **Setzen:** Aus demselben Text entstehen das druckfertige Buch-PDF mit Umschlag, ein E-Book und
  Stichwortzettel für die Kanzel – ohne zusätzliches Programm.

Mehr unter [Funktionen](/funktionen/).

## Frei {#frei}

Baruch steht unter der **GNU Affero General Public License 3.0** (oder später) und ist bewusst ein freier
Gegenentwurf zu bezahlten Schreibwerkzeugen. Der Quellcode liegt offen; wer Baruch verändert und anderen über
ein Netz anbietet, gibt seine Änderungen ebenso frei. Mehr dazu unter [Lizenz](/lizenz/).

## Lokal {#lokal}

Baruch läuft auf dem eigenen Rechner und wird im Browser bedient; standardmäßig ist es nur dort erreichbar
(`http://127.0.0.1:8765/`). Die Daten liegen im eigenen Benutzerordner: eine Datenbankdatei und je Werk ein
**Git-Spiegel** – eine automatische Sicherung aller Entwürfe als gewöhnliche Textdateien, mit jeder Änderung
nachvollziehbar und auch ohne Baruch lesbar und setzbar. Schlüssel und Token liegen im Schlüsselbund des Systems, nie in
Dateien. Baruch fragt nie von sich aus im Netz nach neuen Versionen; Modelle bei fremden Anbietern gibt es nur
nach ausdrücklicher Freigabe.

## Der Mensch entscheidet {#der-mensch-entscheidet}

Assistenten schreiben **Vorschläge**, Beiträge, Einfälle und Anträge. Übernehmen, Ändern und Löschen am Buchtext
bleiben allein beim Menschen – im Browser ebenso wie über die Schnittstellen für andere Programme. Jeder Abschnitt hat beliebig viele
Fassungen, genau eine ist die aktuelle, und umschalten geht jederzeit. Jede Änderung wird im Spiegel des Werks
festgehalten, mit Herkunft: Mensch, Assistent oder ein angebundenes KI-Programm. Was nicht geht, sagt Baruch offen; nichts wird im
Hintergrund unbemerkt gekürzt, weggelassen oder dazugedichtet – auch nicht, wenn ein Text für ein KI-Modell zu
lang ist.

## Ohne JavaScript bedienbar {#ohne-javascript}

Jede Seite von Baruch funktioniert mit Links und Formularen. JavaScript bringt Komfort – Ziehen, Tastenkürzel,
Fenster zum Aufklappen, sofortige Aktualisierung –, ist aber nie Voraussetzung. Auch diese Webseite kommt ohne JavaScript, Cookies,
Zähler und fremde Schriften aus.

## Weiter {#weiter}

- [Funktionen](/funktionen/): was Baruch kann, mit Bildern
- [Installation](/installation/): Pakete für Debian und Ubuntu, AppImage, macOS und Windows – und die
  [ersten Schritte danach](/installation/#nach-der-installation)
- [Hilfe](/hilfe/): Baruch bedienen, Schritt für Schritt und mit Bildern – vom
  [ersten Start](/hilfe/erste-schritte/#starten) bis zum [fertigen Buch](/hilfe/buchsatz/#pdf)
- [Betrieb](/betrieb/): Datenordner, Sicherung, Dauerbetrieb und der Weg ins Netz über einen vorgeschalteten
  Webserver
- [MCP und Assistenten](/mcp/): Assistenten mit einem KI-Modell einrichten, KI-Programme wie Claude Code
  mit Baruch verbinden
