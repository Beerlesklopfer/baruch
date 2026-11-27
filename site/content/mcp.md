---
title: "MCP und Assistenten"
description: "Assistenten mit einem eigenen KI-Modell einrichten, MultiGPT anbinden und Baruch mit KI-Programmen wie Claude Code verbinden – Schritt für Schritt. Assistenten schlagen vor, der Mensch entscheidet."
inhalt: true
---

Baruch kommt ohne KI aus. Wer Assistenten nutzen will, richtet sie einmal ein; diese Seite führt durch die
Schritte. Wie man danach mit ihnen arbeitet, steht in der Hilfe unter
[Assistenten, Chat und Befunde](/hilfe/assistenten/).

## Die Regel {#die-regel}

Assistenten **ändern keinen Buchtext** – gleich, ob sie in Baruch eingebaut sind oder als fremdes KI-Programm
von außen zugreifen. Sie schreiben Vorschläge (eigene Fassungen neben Ihrem Text), Beiträge im Chat, Einfälle
und Anträge. Übernehmen, Ändern, Löschen, Umschalten und Einlesen bleiben dem Menschen. Was die Gliederung
umbaut, wird ein Antrag, dem der Mensch zustimmt. Jede Aktion steht mit ihrer Herkunft im Git-Spiegel des
Werks – der automatischen Mitschrift aller Änderungen als gewöhnliche Textdateien.

## Die Assistenten {#die-assistenten}

Ein Assistent ist eine **Rolle** mit eigenem Auftrag: Er hat einen Namen, ein Kürzel zum Ansprechen (etwa
`@lektor`), ein KI-Modell, eine Farbe und einen Text, der ihm seine Aufgabe erklärt. In Baruch heißen diese
Rollen auch „Personas“. Eine neue Installation bringt sechs fertige Assistenten mit:

| Assistent | Aufgabe |
|---|---|
| Lektor | Aufbau, Klarheit, Stil; schreibt Vorschläge |
| Exeget | Text, Zusammenhang, Hauptaussage |
| Schrift | Bibelstellen und Wortlaut belegen |
| Belege | Quellen und Fußnoten prüfen |
| Nathan | Prüfer: nennt nur Befunde, gibt nie frei |
| Hörer | liest als Zielgruppe |

Die Aufgaben sind vorbereitet; es fehlt nur die Verbindung zu einem Modell. Eigene Assistenten lassen sich dazu
anlegen, vorhandene abschalten.

## Assistenten einrichten {#assistenten-einrichten}

Baruch bringt **kein KI-Modell** mit. Es spricht mit einem **Modelldienst**: einem Programm, das Sprachmodelle
bereitstellt – am besten auf dem eigenen Rechner oder im eigenen Netz. Drei Arten kennt Baruch:

- **Eliezer** – so heißt in Baruch die fest eingebaute erste Verbindung. Voreingestellt ist die Adresse
  `http://127.0.0.1:1234`, also ein Modelldienst auf dem eigenen Rechner.
- **OpenAI-kompatibel** – jeder Dienst, der die verbreitete Schnittstelle von OpenAI spricht (viele Programme
  für lokale Modelle tun das).
- **Ollama** – ein verbreitetes Programm für lokale Modelle.

Welches Programm Sie als Modelldienst nehmen und welches Modell Sie dort laden, entscheiden Sie selbst; Baruch
installiert nichts davon.

### 1. Adresse eintragen {#modell-adresse}

**Baruch ▾ → Einstellungen → Verbindungen** öffnen.

- Läuft Ihr Modelldienst unter `http://127.0.0.1:1234`, ist nichts zu tun: Das ist die Voreinstellung bei
  „Eliezer (Modell-Backend)“. Sonst dort die Adresse ändern und **Speichern**.
- Für einen weiteren Dienst unter **Neue Modell-Verbindung** einen Namen vergeben, die **Art** wählen
  (Eliezer, OpenAI-kompatibel oder Ollama), die **Adresse** eintragen und anlegen.

Adressen beginnen mit `http://` oder `https://` und enthalten weder Benutzernamen noch Schlüssel.

{{< hilfebild name="einstellungen-verbindungen" alt="Einstellungen, Seite Verbindungen: die Verbindung „Eliezer (Modell-Backend)“ mit Adresse, dem Feld für den Schlüssel, „Speichern“ und „Verbindung prüfen“, darunter MultiGPT und die Datenbanken der Bedeutungssuche." titel="Einstellungen → Verbindungen: Adresse eintragen, Schlüssel hinterlegen, prüfen." >}}

### 2. Schlüssel hinterlegen, falls der Dienst einen verlangt {#modell-schluessel}

Viele Dienste auf dem eigenen Rechner brauchen keinen Schlüssel. Verlangt Ihrer einen, tragen Sie ihn im Feld
**Schlüssel** der Verbindung ein und speichern.

- Der Schlüssel landet im **Schlüsselbund** des Systems, dem Passwortspeicher des Betriebssystems – nie in
  einer Datei und nie in der Datenbank.
- Das Feld ist danach wieder leer; daneben steht nur **hinterlegt** oder **fehlt**. Baruch zeigt einen
  Schlüssel nie wieder an.
- Ein Schlüssel gilt nur für die Adresse, für die er eingetragen wurde. Ändern Sie später die Adresse, tragen
  Sie ihn neu ein – Baruch schickt ihn nicht von selbst an ein neues Ziel.

### 3. Verbindung prüfen {#modell-pruefen}

**Verbindung prüfen** fragt den Dienst einmal nach seiner Liste der Modelle (höchstens zehn Sekunden). Danach
steht bei der Verbindung „✓ geprüft“ mit den gemeldeten Modellen oder „✗ fehlgeschlagen“ mit dem Grund – etwa
keine Verbindung oder ein abgelehnter Schlüssel. Bei Eliezer nennt die Meldung außerdem, welche Assistenten ein
Modell eingestellt haben, das der Dienst nicht kennt.

Der **Kurztest** darunter schickt den festen Auftrag „Antworte nur mit: OK“ an ein Modell Ihrer Wahl und zeigt
Antwort und Dauer. Buchtext geht dabei nicht hinaus. Muss der Dienst das Modell erst laden, dauert es
entsprechend.

### 4. Modell je Assistent wählen {#modell-waehlen}

**Einstellungen → Assistenten** zeigt je Assistent einen Reiter. Dort:

1. die **Verbindung** wählen (voreingestellt: Eliezer),
2. das **Modell** eintragen – nach einer erfolgreichen Prüfung schlägt das Feld die Modelle des Dienstes vor;
   ein Modell für alle genügt,
3. bei Bedarf **Kontext (Tokens)** anpassen: wie viel Text das Modell auf einmal bekommt. Ein „Token“ ist hier
   ein Wortstück, die Maßeinheit der Modelle – nicht zu verwechseln mit dem Token für den Zugang weiter unten;
4. **Speichern**.

{{< hilfebild name="einstellungen-assistenten" alt="Einstellungen, Seite Assistenten: oben die Wahl des Lektors, darunter je Assistent ein Reiter mit Name, Kürzel, Farbe, Modell, Kontext, Verbindung und dem Text der Rolle." titel="Einstellungen → Assistenten: je Rolle Verbindung und Modell." >}}

Alle Felder dieser Seite beschreibt die Hilfe unter
[Einstellungen → Assistenten](/hilfe/einstellungen/#assistenten).

### 5. Ausprobieren {#modell-probe}

Einen Abschnitt öffnen und im Chat am Abschnitt `@lektor` ansprechen, etwa „@lektor, was trägt dieser
Abschnitt?“. Die Antwort erscheint als Beitrag im Chat. `@alle` ruft alle eingeschalteten Assistenten. Mehr
dazu in der [Hilfe zum Chat](/hilfe/assistenten/#chat).

### Anbieter außerhalb des eigenen Netzes {#extern}

Baruch unterscheidet lokale und **externe** Verbindungen. Als lokal gelten der eigene Rechner und Adressen im
eigenen Netz; alles andere gilt als extern, solange Sie es nicht ausdrücklich anders festlegen. An eine externe
Verbindung schickt Baruch Buchtext nur, wenn Sie es **je Assistent** freigeben (Einstellungen → Assistenten,
Kästchen „Buchtext an eine externe Verbindung schicken“). Ohne Freigabe geht keine Anfrage mit Buchtext hinaus.

## MultiGPT einrichten {#multigpt-einrichten}

**MultiGPT** ist ein eigener, selbst betriebener Dienst für die Suche in Dokumentsammlungen und im Web. Baruch
braucht ihn nicht; wer ihn hat, bekommt dreierlei dazu: die Bedeutungssuche (die Suche nach dem Sinn statt nach
dem Wortlaut) in seinen Sammlungen, den Volltext seiner Dokumente in der Literatur und – nur für den Assistenten
„Belege“ und nur auf Wunsch – die Recherche im Web.

Für MultiGPT ist **keine Adresse voreingestellt**. Ohne Adresse fragt Baruch den Dienst nie an.

1. **Einstellungen → Verbindungen → MultiGPT:** die Adresse Ihres Dienstes eintragen. Der graue Text im Feld
   (`https://multigpt.example/mcp/`) zeigt nur die Form; er wird nie gespeichert und nie angefragt.
2. Den **Token** des Dienstes eintragen – das Schlüsselwort, das Ihr MultiGPT für den Zugang ausgibt. Er kommt
   wie alle Schlüssel in den Schlüsselbund. **Speichern**.
3. **Verbindung prüfen.** Bei Erfolg nennt Baruch den Dienst, die Zahl seiner Werkzeuge und seine Sammlungen,
   etwa „Verbindung steht: multigpt 1.2, 4 Werkzeuge, 2 Sammlungen.“
4. Unten auf derselben Seite, unter **RAG-Wege des Werks** (RAG ist das Fachwort für die Bedeutungssuche):
   MultiGPT ankreuzen, die Sammlungen wählen, die durchsucht werden sollen, und speichern. Ohne gewählte
   Sammlung bleibt der Weg aus.
5. Nur wer die **Websuche** will: Einstellungen → Assistenten, Reiter „Belege“, „im Web recherchieren“
   einschalten. Voreingestellt ist aus. Die Suchanfrage enthält dann den Anfang des Abschnitts oder Ihre Frage –
   also unveröffentlichten Text, der über MultiGPT an eine Suchmaschine geht.

Den Abgleich ganzer Werke mit MultiGPT und die Quellen beschreibt die Hilfe unter
[Bibel, Literatur und Quellen](/hilfe/bibel-quellen/#quellen); die Felder der Seite stehen unter
[Einstellungen → Verbindungen](/hilfe/einstellungen/#verbindungen).

**Geprüft und nicht geprüft:** Baruch ist gegen einen nachgebauten MultiGPT-Dienst getestet, der dieselbe
Schnittstelle spricht. Welche Rechte ein Token bei Ihrem Dienst braucht, hängt von dessen Einrichtung ab;
fehlt eines, sagt Baruch es in der Meldung.

## KI-Programme mit Baruch verbinden {#mcp-clients}

**MCP** (Model Context Protocol) ist eine offene Schnittstelle, über die KI-Programme wie Claude Code direkt
mit dem eigenen Werk arbeiten – lesen, prüfen, Vorschläge machen –, ohne Texte hin und her zu kopieren. Ein
solches Programm heißt MCP-Client. Auch für diese Programme gilt die Regel oben: Sie schlagen vor, der Mensch
entscheidet.

### Auf demselben Rechner {#mcp-lokal}

Baruch bietet die Schnittstelle unter der Adresse `http://127.0.0.1:8765/mcp` an. Mit Claude Code im Terminal:

```sh
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Vom eigenen Rechner aus braucht das keinen Token. Danach in Claude Code etwa „Zeig mir die Gliederung meines
Buchs in Baruch“ – das Programm listet beim Verbinden alle Werkzeuge mit ihren Erklärungen auf.

Über MCP lesen Programme Werke, Gliederung, Fassungen, Einfälle, Befunde und Bibeltexte, schreiben Vorschläge,
Beiträge und Einfälle, stellen Anträge und schlagen Glossarbegriffe vor. Welche Programme sich seit dem Start
von Baruch verbunden haben, zeigt [Einstellungen → MCP-Zugang](/hilfe/einstellungen/#mcp-zugang).

{{< hilfebild name="einstellungen-mcp" alt="Einstellungen, Seite MCP-Zugang: die Adresse für Programme, der fertige Befehl für ein Assistenten-Programm, der Vermerk, dass noch kein Zugangsschlüssel (Token) erzeugt ist, das Formular „Token erzeugen“ und die Liste der seit dem Start verbundenen Programme." titel="Einstellungen → MCP-Zugang: Adresse, Befehl und Tokens." >}}

**Geprüft** ist die Verbindung mit Claude Code. Andere MCP-Clients (etwa Claude Desktop) sollten mit derselben
Adresse arbeiten, sofern sie die Übertragungsart „Streamable HTTP“ beherrschen – das ist **nicht geprüft**.

### Von einem anderen Rechner im eigenen Netz {#mcp-netz}

Von anderen Rechnern ist Baruch zunächst gar nicht erreichbar. Zwei Wege:

- **Sicherer: ein SSH-Tunnel.** Dann bleibt Baruch auf `127.0.0.1`, die Verbindung ist verschlüsselt, und es
  braucht keinen Token.
- **Direkt, nur im vertrauenswürdigen Netz:**
  1. Baruch mit der Adresse des Rechners im Heimnetz starten, etwa
     `baruch-server --listen 192.168.1.20:8765`.
  2. **Einstellungen → MCP-Zugang:** einen Namen eingeben (etwa den des anderen Rechners) und **Token
     erzeugen**. Der **Token** ist ein langes, zufälliges Schlüsselwort für genau diesen Zugang. Baruch zeigt
     ihn **genau einmal**; wer ihn verliert, erzeugt einen neuen.
  3. Auf dem anderen Rechner – über die **IP-Adresse**, nicht über einen Rechnernamen:

     ```sh
     claude mcp add -s local --transport http baruch http://192.168.1.20:8765/mcp \
       --header "Authorization: Bearer <Token>"
     ```

  Von anderen Rechnern ist so nur diese Schnittstelle erreichbar, nichts sonst. **Achtung:** Auf diesem Weg ist
  nichts verschlüsselt – Token und Texte gehen lesbar durchs Netz.

### Über das Internet {#mcp-internet}

Nur hinter einem vorgeschalteten Webserver mit Verschlüsselung (Reverse-Proxy), siehe
[Betrieb](/betrieb/#reverse-proxy). Dort verlangt die Schnittstelle immer einen Token:

```sh
claude mcp add -s local --transport http baruch https://baruch.example.org/mcp \
  --header "Authorization: Bearer <Token>"
```

### Tokens verwalten {#tokens}

Beiträge und Vorschläge eines Zugangs mit Token stehen unter dem Namen `@mcp:<Name des Tokens>` – so bleibt
sichtbar, welches Programm was geschrieben hat. Ein Token lässt sich unter Einstellungen → MCP-Zugang jederzeit
**widerrufen** und gilt dann sofort nicht mehr.

### Für eigene Programme {#eigene-programme}

Dieselben Funktionen gibt es für eigene Programme als Programmierschnittstelle (JSON-RPC). Beide
Schnittstellen sind im Repository beschrieben: {{< doku "docs/MCP.md" "MCP mit allen Werkzeugen" >}} und
{{< doku "docs/RPC.md" "RPC" >}}.
