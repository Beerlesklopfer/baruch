---
title: "MCP und Assistenten"
description: "Personas mit eigenem Modell einrichten und Baruch als MCP-Server mit Claude Code oder anderen Clients verbinden – Assistenten schlagen vor, der Mensch entscheidet."
---

## Die Regel

Assistenten – ob Persona in Baruch oder Programm über MCP – **ändern keinen Buchtext**. Sie schreiben
Vorschläge (eigene Fassungen), Beiträge im Chat, Einfälle und Anträge. Übernehmen, Ändern, Löschen, Umschalten
und Importieren bleiben dem Menschen. Was die Gliederung umbaut, wird ein Antrag, dem der Mensch zustimmt. Jede
Aktion steht mit ihrer Herkunft im Git-Spiegel des Werks.

## Personas

Eine Persona hat eine Rolle, ein Modell, eine Verbindung, eine Kontextlänge, eine Farbe und einen eigenen
Prompt. Eine neue Installation bringt sechs fertige Personas mit:

| Persona | Rolle |
|---|---|
| Lektor | Aufbau, Klarheit, Stil; schreibt Vorschläge |
| Exeget | Text, Zusammenhang, Hauptaussage |
| Schrift | Bibelstellen und Wortlaut belegen |
| Belege | Quellen und Fußnoten prüfen |
| Nathan | Prüfer: nennt nur Befunde, gibt nie frei |
| Hörer | liest als Zielgruppe |

Rollen und Prompts sind vorbereitet; es fehlt nur die Verbindung zu einem Modell. Eigene Personas lassen sich
dazu anlegen, vorhandene abschalten.

1. **Einstellungen → Verbindungen:** das Modell-Backend eintragen – ein OpenAI-kompatibler Dienst auf dem
   eigenen Rechner oder Ollama. Schlüssel liegen im Schlüsselbund des Systems, nie in Dateien.
2. **Einstellungen → Assistenten:** je Persona Verbindung und Modell wählen (ein Modell für alle genügt).
3. Im Chat am Abschnitt ruft `@lektor` oder „Lektor, …“ eine Persona; `@alle` ruft alle.

Ein Anbieter außerhalb des eigenen Netzes muss ausdrücklich freigegeben werden – je Verbindung und je Persona.
Ohne Freigabe geht keine Anfrage hinaus.

## Baruch als MCP-Server

**MCP** (Model Context Protocol) ist eine offene Schnittstelle, über die KI-Programme wie Claude Code oder
Claude Desktop direkt mit dem eigenen Werk arbeiten können – lesen, prüfen, Vorschläge machen –, ohne Texte hin
und her zu kopieren. Auch für sie gilt die Regel oben: Sie schlagen vor, der Mensch entscheidet.

Technisch ist Baruch ein MCP-Server unter `POST /mcp` (Streamable HTTP). Mit Claude Code auf demselben Rechner:

```sh
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Über MCP lesen Programme Werke, Gliederung, Fassungen, Einfälle, Befunde und Bibeltexte, schreiben Vorschläge,
Beiträge und Einfälle, stellen Anträge und schlagen Glossarbegriffe vor. Jedes Werkzeug beschreibt sich selbst:
Ein MCP-Client zeigt nach dem Verbinden die vollständige Liste mit Erklärungen an. Für eigene Programme gibt es
dieselben Funktionen auch als JSON-RPC unter `POST /rpc`.

## Zugang und Token

- **Vom eigenen Rechner** (127.0.0.1 bzw. ::1) geht `/mcp` ohne Token.
- **Von anderen Rechnern im eigenen Netz** nur mit Token: **Einstellungen → MCP-Zugang → Token erzeugen**.
  Standardmäßig ist Baruch von anderen Rechnern aus gar nicht erreichbar (Verbindung abgelehnt). Dafür Baruch
  mit der Adresse des Rechners im Heimnetz starten, etwa `baruch-server --listen 192.168.1.20:8765`, und den
  Client auf `http://192.168.1.20:8765/mcp` mit dem Token richten – über die IP-Adresse, nicht über einen
  Rechnernamen (sonst antwortet Baruch zum Schutz mit 421). Sicherer ist ein SSH-Tunnel: Dann bleibt Baruch auf
  `127.0.0.1`, und der Zugang braucht keinen Token. Baruch zeigt den
  Token genau einmal und speichert nur seinen Hash. Von anderen Rechnern ist nur `/mcp` erreichbar, nichts
  sonst.
- **Hinter einem Reverse-Proxy** (erlaubter Rechnername, siehe [Betrieb](/betrieb/)) verlangt `/mcp` immer
  einen Token:

```sh
claude mcp add -s local --transport http baruch https://baruch.example.org/mcp \
  --header "Authorization: Bearer <Token>"
```

Beiträge und Vorschläge eines Zugangs mit Token stehen unter dem Autor `@mcp:<Name des Tokens>`. Ein Token lässt
sich jederzeit widerrufen und gilt dann sofort nicht mehr.

Ohne Proxy spricht Baruch nur HTTP ohne Verschlüsselung: Token und Texte gehen dann im Klartext durchs Netz.
Das taugt nur im eigenen, vertrauenswürdigen Netz – oder über einen Tunnel.
