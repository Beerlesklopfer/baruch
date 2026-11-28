---
title: "MCP and assistants"
description: "Set up assistants with an AI model of your own, connect MultiGPT and connect Baruch with AI programs such as Claude Code – step by step. Assistants suggest, the human decides."
inhalt: true
---

Baruch works without AI. Whoever wants to use assistants sets them up once; this page leads through the steps.
How to work with them afterwards is described in the help under
[Assistants, chat and findings](/en/hilfe/assistenten/).

## The rule {#die-regel}

Assistants **change no book text** – whether they are built into Baruch or access it from outside as a separate AI
program. They write proposals (revisions of their own next to your text), posts in the chat, ideas and requests.
Accepting, changing, deleting, switching and importing stay with the human. Whatever rebuilds the outline becomes
a request that the human approves. Every action is recorded with its origin in the Git mirror of the work – the
automatic record of all changes as ordinary text files.

## The assistants {#die-assistenten}

An assistant is a **role** with a task of its own: it has a name, a handle to address it (for example `@lektor`),
an AI model, a colour and a text that explains its task to it. In Baruch these roles are also called "personas".
A new installation brings six ready assistants:

| Assistant | Task |
|---|---|
| Editor (`@lektor`) | structure, clarity, style; writes proposals |
| Exegete (`@exeget`) | text, context, main message |
| Scripture (`@schrift`) | back up Bible references and wording |
| Sources (`@belege`) | check sources and footnotes |
| Nathan (`@nathan`) | reviewer: names findings only, never approves |
| Listener (`@hoerer`) | reads as the audience |

The names and handles of the ready assistants are German (`Lektor`, `Exeget`, `Schrift`, `Belege`, `Nathan`,
`Hörer`); you can rename them in the settings.

The tasks are prepared; only the connection to a model is missing. You can add assistants of your own and switch
existing ones off.

## Set up assistants {#assistenten-einrichten}

Baruch ships **no AI model**. It talks to a **model service**: a program that provides language models – best on
your own computer or in your own network. Baruch knows three kinds:

- **Eliezer** – that is the name in Baruch of the first connection, which is built in. The preset address is
  `http://127.0.0.1:1234`, that is a model service on your own computer.
- **OpenAI-compatible** – any service that speaks the widespread interface of OpenAI (many programs for local
  models do).
- **Ollama** – a widespread program for local models.

Which program you take as the model service and which model you load there is up to you; Baruch installs none of
it.

### 1. Enter the address {#modell-adresse}

Open **Baruch ▾ → Settings → Connections**.

- If your model service runs at `http://127.0.0.1:1234`, there is nothing to do: that is the preset of
  **Eliezer (model backend)**. Otherwise change the address there and press **Save**.
- For another service, under **New model connection** give it a name, choose the **Kind** (Eliezer,
  OpenAI-compatible or Ollama), enter the **Address** and create it.

Addresses begin with `http://` or `https://` and contain neither user names nor keys.

{{< hilfebild name="einstellungen-verbindungen" alt="Settings, page Connections: the connection Eliezer (model backend) with its address, the field for the key, Save and Check connection, below them MultiGPT and the databases of the semantic search." titel="Settings → Connections: enter the address, store the key, check." >}}

### 2. Store a key if the service requires one {#modell-schluessel}

Many services on your own computer need no key. If yours requires one, enter it in the field **Key** of the
connection and save.

- The key goes into the **keyring** of the system, the password store of the operating system – never into a file
  and never into the database.
- The field is empty again afterwards; next to it only **stored** or **missing** is shown. Baruch never shows a
  key again.
- A key applies only to the address it was entered for. If you change the address later, enter it again – Baruch
  does not send it to a new target by itself.

### 3. Check the connection {#modell-pruefen}

**Check connection** asks the service once for its list of models (at most ten seconds). Afterwards the
connection shows **✓ checked** with the models reported or **✗ failed** with the reason – for example no
connection or a refused key. For Eliezer the message also names which assistants have a model set that the
service does not know.

The **Quick test** below sends the fixed task "Reply only with: OK" to a model of your choice and shows the answer
and the duration. No book text goes out. If the service must load the model first, it takes correspondingly
longer.

### 4. Choose a model per assistant {#modell-waehlen}

**Settings → Assistants** shows a tab per assistant. There:

1. choose the **Connection** (preset: Eliezer),
2. enter the **Model** – after a successful check the field suggests the models of the service; one model for all
   is enough,
3. if needed adjust **Context (tokens)**: how much text the model gets at once. A "token" here is a piece of a
   word, the unit of measurement of the models – not to be confused with the token for access further below;
4. **Save**.

{{< hilfebild name="einstellungen-assistenten" alt="Settings, page Assistants: at the top the choice of the editor, below it a tab per assistant with name, handle, colour, model, context, connection and the text of the role." titel="Settings → Assistants: connection and model per role." >}}

All fields of this page are described in the help under
[Settings → Assistants](/en/hilfe/einstellungen/#assistenten).

### 5. Try it out {#modell-probe}

Open a section and address `@lektor` in the chat of the section, for example "@lektor, what holds up in this
section?". The answer appears as a post in the chat. `@alle` calls all assistants that are switched on. More in
the [help on the chat](/en/hilfe/assistenten/#chat).

### Providers outside your own network {#extern}

Baruch distinguishes local and **external** connections. Your own computer and addresses in your own network
count as local; everything else counts as external, as long as you do not explicitly decide otherwise. Baruch
sends book text to an external connection only if you clear it **per assistant** (Settings → Assistants, checkbox
**Send book text to an external connection**). Without clearance no request with book text goes out.

## Set up MultiGPT {#multigpt-einrichten}

**MultiGPT** is a separate, self-hosted service for searching document collections and the web. Baruch does not
need it; whoever has it gets three things in addition: the semantic search (the search by meaning instead of by
wording) in its collections, the full text of its documents in the literature and – only for the assistant
for sources (`@belege`) and only on request – research on the web.

**No address is preset** for MultiGPT. Without an address Baruch never asks the service.

1. **Settings → Connections → MultiGPT:** enter the address of your service. The grey text in the field
   (`https://multigpt.example/mcp/`) only shows the form; it is never saved and never asked.
2. Enter the **token** of the service – the key word that your MultiGPT issues for access. Like all keys it goes
   into the keyring. **Save**.
3. **Check connection.** On success Baruch names the service, the number of its tools and its collections.
4. At the bottom of the same page, under **RAG paths of the work** (RAG is the technical term for the semantic
   search): tick MultiGPT, choose the collections to be searched and save. Without a chosen collection the way
   stays off.
5. Only if you want the **web search**: Settings → Assistants, tab `Belege` (sources), switch on **research on
   the web**. The preset is off. The search query then contains the beginning of the section or your question –
   that is unpublished text that goes through MultiGPT to a search engine.

Comparing whole works with MultiGPT and the sources are described in the help under
[Bible, literature and sources](/en/hilfe/bibel-quellen/#quellen); the fields of the page are under
[Settings → Connections](/en/hilfe/einstellungen/#verbindungen).

**Checked and not checked:** Baruch is tested against a reproduced MultiGPT service that speaks the same
interface. Which rights a token needs with your service depends on how it is set up; if one is missing, Baruch
says so in the message.

## Connecting AI programs with Baruch {#mcp-clients}

**MCP** (Model Context Protocol) is an open interface through which AI programs such as Claude Code work directly
with your own work – reading, checking, making proposals – without copying texts back and forth. Such a program
is called an MCP client. The rule above applies to these programs too: they suggest, the human decides.

### On the same computer {#mcp-lokal}

Baruch offers the interface at the address `http://127.0.0.1:8765/mcp`. With Claude Code in the terminal:

```sh
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

From your own computer this needs no token. Afterwards ask Claude Code, for example, "Show me the outline of my
book in Baruch" – when connecting, the program lists all tools with their explanations.

Through MCP, programs read works, outline, revisions, ideas, findings and Bible texts, write proposals, posts and
ideas, make requests and suggest glossary terms. Which programs have connected since Baruch started is shown by
[Settings → MCP access](/en/hilfe/einstellungen/#mcp-zugang).

{{< hilfebild name="einstellungen-mcp" alt="Settings, page MCP access: the address for programs, the ready command for an assistant program, the note that no access key (token) has been created yet, the form Create token and the list of programs connected since the start." titel="Settings → MCP access: address, command and tokens." >}}

The connection with Claude Code is **checked**. Other MCP clients (for example Claude Desktop) should work with the
same address, provided they master the transport "Streamable HTTP" – that is **not checked**.

### From another computer in your own network {#mcp-netz}

From other computers Baruch cannot be reached at all at first. Two ways:

- **Safer: an SSH tunnel.** Then Baruch stays on `127.0.0.1`, the connection is encrypted, and no token is
  needed.
- **Directly, only in a trusted network:**
  1. Start Baruch with the address of the computer in the home network, for example
     `baruch-server --listen 192.168.1.20:8765`.
  2. **Settings → MCP access:** enter a name (for example that of the other computer) and press **Create
     token**. The **token** is a long, random key word for exactly this access. Baruch shows it **exactly
     once**; whoever loses it creates a new one.
  3. On the other computer – through the **IP address**, not through a host name:

     ```sh
     claude mcp add -s local --transport http baruch http://192.168.1.20:8765/mcp \
       --header "Authorization: Bearer <Token>"
     ```

  From other computers only this interface can be reached this way, nothing else. **Caution:** on this way
  nothing is encrypted – token and texts travel readably through the network.

### Over the Internet {#mcp-internet}

Only behind a web server in front with encryption (reverse proxy), see [Operation](/en/betrieb/#reverse-proxy).
There the interface always requires a token:

```sh
claude mcp add -s local --transport http baruch https://baruch.example.org/mcp \
  --header "Authorization: Bearer <Token>"
```

### Managing tokens {#tokens}

Posts and proposals of an access with a token appear under the name `@mcp:<name of the token>` – so it stays
visible which program wrote what. A token can be **revoked** under Settings → MCP access at any time and is then
no longer valid at once.

### For your own programs {#eigene-programme}

The same functions exist for your own programs as a programming interface (JSON-RPC). Both interfaces are
described in the repository: {{< doku "docs/MCP.md" "MCP with all tools" >}} and {{< doku "docs/RPC.md" "RPC" >}}.
