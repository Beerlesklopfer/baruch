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
Prompt – etwa Lektor, Exeget, Belege oder Hörer.

1. **Einstellungen → Verbindungen:** das Modell-Backend eintragen – ein OpenAI-kompatibler Dienst auf dem
   eigenen Rechner oder Ollama. Schlüssel liegen im Schlüsselbund des Systems, nie in Dateien.
2. **Einstellungen → Assistenten:** je Persona Modell und Verbindung wählen.
3. Im Chat am Abschnitt ruft `@lektor` oder „Lektor, …“ eine Persona; `@alle` ruft alle.

Ein Anbieter außerhalb des eigenen Netzes muss ausdrücklich freigegeben werden – je Verbindung und je Persona.
Ohne Freigabe geht keine Anfrage hinaus.

## Baruch als MCP-Server

Baruch ist ein MCP-Server unter `POST /mcp` (Streamable HTTP). Mit Claude Code auf demselben Rechner:

```sh
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Über MCP lesen Programme Werke, Gliederung, Fassungen, Einfälle, Befunde und Bibeltexte, schreiben Vorschläge,
Beiträge und Einfälle, stellen Anträge und schlagen Glossarbegriffe vor. Alle Werkzeuge stehen in
{{< doku "docs/MCP.md" "docs/MCP.md" >}}; dieselben Funktionen gibt es als JSON-RPC unter `POST /rpc`
({{< doku "docs/RPC.md" "docs/RPC.md" >}}).

## Zugang und Token

- **Vom eigenen Rechner** (127.0.0.1 bzw. ::1) geht `/mcp` ohne Token.
- **Von anderen Rechnern** nur mit Token: **Einstellungen → MCP-Zugang → Token erzeugen**. Baruch zeigt den
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
