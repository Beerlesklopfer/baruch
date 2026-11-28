---
title: "Operation"
description: "For everyone who runs Baruch permanently: data folder, backup and restore, updating, reachability in the network with a server in front, accounts and access for AI programs."
inhalt: true
---

This page is meant for operators: for everyone who keeps Baruch running permanently, backs it up, moves it to
another computer or makes it reachable in the network under a name of their own. How to write with Baruch is
described in the [help](/en/hilfe/).

Every step says whether it is **checked** or only **assumed**. What nobody has tried on a real setup yet is
collected in the section [What is not tried](#nicht-erprobt).

## The most important first {#das-wichtigste}

- **Without accounts Baruch has no sign-in.** Whoever reaches the interface reads and changes everything: works,
  revisions, settings, connections. That is why, after the installation, Baruch can only be reached from your own
  computer, at `http://127.0.0.1:8765/`.
- **With accounts Baruch itself requires a sign-in** – for all pages and forms. Programs that access the
  interface for AI programs (MCP, see [below](#mcp-token)) do not sign in with an account but with a **token**: a
  long, random key word that Baruch creates.
- **Whoever wants to reach Baruch over the network puts a reverse proxy in front of it.** That is a web server
  (for example nginx or Caddy) that accepts the requests from the network and passes them on to Baruch. It must
  speak **TLS**, that is encrypt the connection (the "https" in the address) – otherwise password and session
  would travel readably over the line. Without accounts it must also **protect the access**; with accounts this
  additional protection is optional but remains a sensible second layer.
- **Baruch never asks the net for new versions on its own.** Updates come through the package manager or by
  hand.

## Data folder {#datenordner}

| System | Folder |
|---|---|
| Linux | `~/.local/share/baruch` (or `$XDG_DATA_HOME/baruch`) |
| macOS | `~/Library/Application Support/baruch` |
| Windows | `%LOCALAPPDATA%\baruch` (Local, not Roaming) |

Another folder: `baruch-server --data FOLDER` or the environment variable `BARUCH_DATA`. The folder contains:

- `baruch.sqlite` – the database and thus **the source of all data**. While Baruch is running, two helper files
  with the endings `-wal` and `-shm` lie next to it.
- `works/<name>/` – per work the **Git mirror**: an automatic record of all changes as ordinary text files that
  can be read with the version control program Git and even without Baruch. Only read it, change nothing in it:
  the database determines the state, and at start Baruch brings the mirrors up to date. The mirror also holds what
  was lost when an outline item was deleted (the current revision, the other revisions, the posts) – an item
  deleted by mistake can be restored from it by hand.
- `lancedb/` – the search index of the semantic search (the search by meaning instead of by wording). It can be
  rebuilt at any time.

**Permissions:** if Baruch creates the folder itself, under Linux and macOS only you may enter it (permissions
0700). Baruch never changes an existing folder. If its permissions are more open, a note under **Settings →
Data** says so; the remedy by hand: `chmod 700 <data folder>`.

**Keys, tokens and passwords** of the connections are not in the data folder but in the **keyring** of the
system, the password store of the operating system. This applies when Baruch runs with the usual data folder or
is started with `--secrets keyring` (also `BARUCH_SECRETS=keyring`). With a data folder of your own and without
this option they stay only in memory and are gone after quitting. Which applies, Baruch says at start in the line
`Geheimnisse: …` (secrets). Of the tokens for AI programs, the database holds only a check value, never the token
itself.

No package creates anything in the data folder, and no uninstall deletes anything there.

{{< hilfebild name="einstellungen-daten" alt="Settings, page Data: path of the data folder, name of the instance and the button Back up now with the checkbox for confirmation." titel="Settings → Data shows the data folder and creates backups." >}}

## Backup {#sicherung}

**Settings → Data → Back up now** (tick the checkbox) creates a folder `baruch.sicherung-YYYY-MM-DD-HHMMSS`
**next to** the data folder – under Linux, for example, `~/.local/share/baruch.sicherung-2026-11-24-213000`. It
contains the database as a consistent state, the mirrors and everything else. Nothing is overwritten. If there
are already ten backups next to it, Baruch refuses further ones until old ones are moved or deleted – Baruch
itself deletes none. The message **Backup created (next to the data folder).** appears once; after a restart it
is no longer shown, the backup itself stays. How it works from the writer's point of view is described in the
[help on backups](/en/hilfe/austausch-sicherung/#sicherung).

- **Before every new version** create a backup. On the first start of a new version Baruch converts the database
  to the new state by itself; going back to the old version is then only possible through the backup.
- **Put backups somewhere else.** They are on the same disk as the data; only a copy in another place helps
  against a failure of the disk (backup program, another computer).
- **Copy the data folder itself** only when Baruch is not running: the database writes into several files during
  operation, a copy made in the middle can be inconsistent.
- **The keyring is not part of the backup.** After moving to another computer, enter the keys of the connections
  again ([Settings → Connections](/en/hilfe/einstellungen/#verbindungen)).
- **Keep backups locked away.** With accounts, the database contains the check values of the passwords and of the
  running sign-ins – no passwords in plain text, but nothing for other eyes either.

### Restoring {#wiederherstellen}

**Assumed, not tried:** this way is described like this but has not yet been played through in a real
emergency. Hence step 2 – keep the old folder.

1. Quit Baruch ([Help: Quitting Baruch](/en/hilfe/erste-schritte/#beenden)).
2. Rename the data folder (for example `baruch` to `baruch.alt`), do not delete it.
3. Copy the backup to its place and rename it to `baruch`.
4. Start Baruch.

A backup can be inspected beforehand without touching the running data – on a **copy**, because Baruch writes
into the folder it opens at start:

```sh
cp -a ~/.local/share/baruch.sicherung-2026-11-24-213000 ~/baruch-probe
baruch-server --data ~/baruch-probe --listen 127.0.0.1:8766
```

Then open `http://127.0.0.1:8766/`. Keys stay only in memory in this case.

### Taking a single work along {#werk-mitnehmen}

The exchange file carries a single work from one computer to another – with all revisions, ideas, posts and
images, but without keys:

```sh
baruch-server export ID --baruch FILE.baruch
```

`ID` is the number of the work. The same works in the interface; see
[Help: Exchange](/en/hilfe/austausch-sicherung/). Accounts and shares for editors belong to the installation, not
to the work: they are neither in the exchange file nor in the Git mirror.

## Permanent operation and updating {#dauerbetrieb}

The Debian package sets up no service. To keep Baruch running permanently, take the bundled example of a service
of the logged-in user (systemd):

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

Baruch runs as a service **of the user**, not as a service of the system – only this way does it reach the
user's keyring and data folder.

**Updating:** create a backup, install the new version (`sudo apt upgrade` or the new file from the release page,
see [Installation](/en/installation/#aktualisieren)), restart the service
(`systemctl --user restart baruch.service`). The page **Baruch ▾ → About Baruch** shows the running version and
links to the release page; this click is the only "request" for new versions.

### Keyring on a server without a desktop {#schluesselbund}

Under Linux Baruch talks to the keyring through the "Secret Service" (gnome-keyring, KWallet, KeePassXC). On a
server without a graphical interface it often does not run. Then:

- Writing, typesetting and everything that needs no connection keeps working. When reading or saving a key,
  Baruch reports **no access to the keyring (locked or not reachable)** – never with the key itself in the text.
- **There are deliberately no environment variables for keys**; keys do not belong in service files or process
  lists.
- **Ways:** run a Secret Service even without a desktop (for example gnome-keyring or KeePassXC in the session of
  the service user) – or start Baruch with `--secrets memory`. Then the keys stay only in memory and must be
  entered again after every restart.

## Reachability: three levels {#erreichbarkeit}

| Level | Start | Reachable | Protection |
|---|---|---|---|
| 1. Only your own computer (this is how Baruch is installed) | `baruch-server` | `http://127.0.0.1:8765/`, `localhost`, `[::1]` | the computer and your own user account |
| 2. Only AI programs from your own network | `baruch-server --listen 192.168.1.20:8765` | from other computers only the address `/mcp`, under the IP address | token; **no encryption** – only in a trusted network |
| 3. Behind a reverse proxy | `baruch-server --allow-host NAME` | everything under `https://NAME/` | TLS in the proxy; sign-in in Baruch (accounts) or access protection in the proxy; `/mcp` with a token |

Level 2 is described in [MCP and assistants](/en/mcp/#mcp-clients). The rest of this page is level 3.

### Why Baruch otherwise refuses host names {#rechnernamen}

Without `--allow-host` Baruch answers only under an IP address or `localhost`; under any other name the error
response **421** comes back. This protects against an attack called DNS rebinding: a foreign web page can let a
name of its own point to `127.0.0.1` and so get the browser to address Baruch under this name. An IP address
cannot be bent. An allowed name lifts this protection for exactly this name – that is why it is named
explicitly.

## Behind a reverse proxy {#reverse-proxy}

### 1. Allow names {#namen-erlauben}

```sh
baruch-server --allow-host baruch.example.org
baruch-server --allow-host baruch.example.org --allow-host writing.example.net
BARUCH_ALLOWED_HOSTS=baruch.example.org,writing.example.net baruch-server
```

`--allow-host` may appear several times; `BARUCH_ALLOWED_HOSTS` separates with commas or spaces. Both together
give the union. In the service file: `Environment=BARUCH_ALLOWED_HOSTS=baruch.example.org`.

A valid name (**checked**):

- 1 to 253 characters; the parts between the dots 1 to 63 characters from `a–z`, `0–9` and `-` (not at the
  edge). Capital letters are read as lower case. A name without a dot (`baruch` in your own network) is valid.
- **Without a port** – the port in the request is arbitrary (`baruch.example.org` and `baruch.example.org:8443`
  both pass).
- **No wildcard** (`*.example.org`), every name on its own. Exactly the name is allowed: `x.baruch.example.org`
  stays refused.
- **No IP address** and not `localhost` – they apply anyway. Names whose last part is a number (`127.1`) are
  refused too, because browsers read them as an IP address.
- Non-ASCII letters only in the ASCII form (`xn--…`).

An invalid name ends the start with a message that names it (`Ungültiger Rechnername „…“ (--allow-host): …`,
invalid host name). At start Baruch names the allowed names.

### 2. Baruch stays local, the proxy runs on the same computer {#baruch-bleibt-lokal}

Baruch keeps listening on `127.0.0.1:8765`; only the proxy opens a port to the outside. So no way leads past the
proxy. The proxy must run **on the same computer**: if a request comes directly from another computer, Baruch
lets only the address `/mcp` through and refuses everything else (error 403), whatever the name.

### 3. Rules for the proxy {#regeln}

1. **Access protection.** Mandatory without accounts: password prompt of the proxy (Basic Auth), client
   certificate or a VPN in front of everything except `/mcp` – Baruch itself then checks nobody. With accounts
   Baruch checks itself; the protection in the proxy is then optional. Whoever uses both signs in twice.
2. **Pass the header `Host` through unchanged.** `Host` holds the name under which the browser called the page.
   Baruch compares it with the origin of the request on every save. If the proxy rewrites `Host` (with nginx that
   is the default!), Baruch refuses saving and all forms with the error 403 **request from a foreign page (Origin
   does not match Host)**. Worse: Baruch would then take every request for one from its own computer – see the
   [check](#probe).
3. **TLS ends in the proxy.** Baruch itself speaks only unencrypted HTTP. The headers with which a proxy
   describes the original request (`X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`, `Forwarded`) are
   **not evaluated by Baruch**. The only exception: if **On the own computer without sign-in** is switched on
   ([accounts](#konten)), a request with one of these headers (plus `X-Real-IP` and `Via`) does **not** count as
   your own computer. Setting them is therefore recommended.
4. **HTTP/1.1 to Baruch** (usual with nginx and Caddy).
5. **Cache nothing, rewrite nothing**, do not buffer long responses. (**Assumed:** answers of the assistants that
   appear word by word otherwise arrive late; not checked with a real proxy.)

### Example nginx {#nginx}

**Not tried on a running nginx.** What is checked is how Baruch answers the headers this configuration sends.

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name baruch.example.org;

    ssl_certificate     /etc/letsencrypt/live/baruch.example.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/baruch.example.org/privkey.pem;

    # Access protection: mandatory as long as no accounts are set up in Baruch (without accounts Baruch has no
    # sign-in). With accounts it can be dropped; TLS stays mandatory.
    auth_basic           "Baruch";
    auth_basic_user_file /etc/nginx/baruch.htpasswd;

    client_max_body_size 64m;            # images and imports; adjust to your own files (assumption)

    location / {
        proxy_pass         http://127.0.0.1:8765;
        proxy_http_version 1.1;
        proxy_set_header   Host $http_host;   # unchanged, with the port – not $proxy_host (the default)
        proxy_buffering    off;
        proxy_read_timeout 600s;              # model answers and typesetting take time
    }

    # /mcp is protected by the token (Authorization: Bearer …). Basic Auth uses the same header, both at once
    # do not work – hence the exception here.
    location = /mcp {
        auth_basic         off;
        proxy_pass         http://127.0.0.1:8765;
        proxy_http_version 1.1;
        proxy_set_header   Host $http_host;
        proxy_buffering    off;
        proxy_read_timeout 600s;
    }
}

server {
    listen 80;
    server_name baruch.example.org;
    return 301 https://$host$request_uri;
}
```

`$http_host` is the header as the browser sends it (with the port, if not 443). `$host` leaves the port out; on a
port other than 443 the origin of the request would then no longer match `Host`, and Baruch would refuse
saving.

### Example Caddy {#caddy}

**Not tried on a running Caddy either.** Caddy fetches the certificate itself and passes `Host` through unchanged
(assumption based on the documentation of Caddy 2).

```caddyfile
baruch.example.org {
    # Mandatory without accounts: access protection. /mcp is protected by the token.
    @interface not path /mcp
    basic_auth @interface {
        author HASH-FROM-caddy-hash-password
    }
    reverse_proxy 127.0.0.1:8765
}
```

In older versions of Caddy 2 `basic_auth` is still called `basicauth`.

### Check after setting up (mandatory) {#probe}

From **another** computer, `NAME` = the allowed name. Each command prints only the number of the response.

```sh
# 1. No interface without sign-in: 401 (from the proxy)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/

# 2. /mcp without a token: 401 (from Baruch). If 200 comes here, the proxy does not pass Host through – switch off at once!
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/mcp \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"ping"}'

# 3. With sign-in: 200; then save a section in the browser (no error 403)
curl -s -o /dev/null -w '%{http_code}\n' -u USER https://NAME/
```

On 2: if the proxy rewrites `Host` to `127.0.0.1:8765`, Baruch sees a request from its own computer under an IP
address – just like one from a program running there – and accepts `/mcp` **without a token**. If `/mcp` is then
also exempt from the access protection, this access would be open to the network. Baruch cannot detect this
case; the check is therefore mandatory.

With accounts the first check does not answer with 401 from the proxy but with a redirect to the sign-in page;
see the [check with accounts](#probe-konten).

On the computer itself, without a proxy, the same check shows the behaviour of Baruch (**checked**):

```sh
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: baruch.example.org' http://127.0.0.1:8765/     # 200
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: other.example.org'  http://127.0.0.1:8765/     # 421
```

## Sign-in and accounts {#konten}

Baruch knows two kinds of accounts: one **author** and any number of **editors**. How to set up accounts, create
editors and share works with them is described in the help under [Setting up accounts](/en/hilfe/konten/#einrichten)
and [Editors](/en/hilfe/konten/#lektoren). Here is what that means for operation.

{{< hilfebild name="konten-einrichten" alt="Page Set up accounts: fields for the name and the password of the first account, the author's." titel="The first account is created on your own computer." breite="640" >}}

- **Without accounts nothing changes.** No sign-in, no redirect. Then everything above applies: only your own
  computer, or a proxy with access protection.
- **The first account** is created by the author **on their own computer** (`http://127.0.0.1:8765/einrichten`
  or **Baruch ▾ → Set up accounts…**), never through the proxy: setting up is locked there. Otherwise the first
  person to reach the address could take over the writing workshop.
- **With accounts the sign-in applies on your own computer too.** The reason: a proxy on the same computer also
  connects from there. If it rewrites `Host`, it would look like your own computer – an exception would then be a
  complete bypass of the sign-in. Under **Settings → Accounts** the author can switch on **On the own computer
  without sign-in**. The exception applies only to requests from your own computer under an IP address or
  `localhost` (never under an allowed name) and only without the headers of a proxy (`Forwarded`,
  `X-Forwarded-For`, `X-Forwarded-Host`, `X-Forwarded-Proto`, `X-Real-IP`, `Via`). **Only switch it on if the
  proxy passes `Host` through unchanged** (check below) – ideally it also sets `X-Forwarded-For`.
- **Direct access from other computers stays locked.** An editor comes through the reverse proxy on the same
  computer (level 3), not through `--listen` in the network.
- **AI programs stay with tokens.** The sign-in applies to pages and forms; programs at the address `/mcp` need
  no account. That also means: a program on your own computer still reaches `/mcp` without a token and without
  sign-in and can write there what was always allowed on this way (proposals, posts, ideas, outline items,
  requests – never change or delete book text). Accounts do not protect this way.
- **Session:** after the sign-in the browser remembers the session in a cookie, a small note that it sends with
  every request. Scripts of a page cannot read it, foreign pages do not trigger it, and under the name of the
  proxy it travels only over encrypted connections. A session ends after 30 days without use, after 90 days at
  the latest, on signing out, when the account is locked and – for all other sessions of the account – when the
  password changes.
- **Links from outside:** if someone follows a link from an e-mail or another page, the browser does not send the
  session with this first jump. Baruch then shows the sign-in page; the link **Already signed in?** →
  **Continue** there leads to the target without signing in again.
- **Passing `Host` through unchanged** is doubly important with accounts: because of the check on saving, because
  of the protection of the session and because of the exception for your own computer.
- **Guessing passwords** is slowed down by Baruch itself: from the fourth failed attempt per account a waiting
  time grows up to at most 15 minutes; meanwhile the sign-in refuses (error 429) and names the waiting time.
  Behind the proxy Baruch only sees the proxy's address – a brake per visitor therefore belongs in the proxy (with
  nginx `limit_req`), if wanted.
- **The lock can lock the author out:** whoever knows the account name keeps the account in the waiting time with
  wrong sign-ins; meanwhile even the right password does not help. This gives no access to data. Remedy: choose an
  account name that cannot be guessed, set up the brake in the proxy – and the emergency way below.
- **Passwords** are stored by Baruch only as a check value from which the password cannot be calculated back –
  never in plain text and never in logs.
- **What an editor writes** (proposals, posts, findings) is stored with its origin `@lektorat:<name>` in the work
  and in the Git mirror.

{{< hilfebild name="konten-einstellungen" alt="Settings, page Accounts: the account of the author, an invented editor and the switch On the own computer without sign-in." titel="Settings → Accounts: author, editors and the exception for your own computer." >}}

### Forgotten password (emergency way) {#passwort-vergessen}

On the computer where Baruch runs, as the user who owns the service:

```sh
baruch-server passwort NAME [--data FOLDER]     # reads the new password as one line from the input
```

This sets the password of the account, ends its sessions and lifts a lock after failed attempts for this account:
signing in with the new password succeeds at once. A lock per visitor address (it exists only without a proxy)
expires by itself after at most 15 minutes without another failed attempt. The command needs no sign-in – whoever
can work on the computer as this user has the data folder anyway.

### Check with accounts {#probe-konten}

From **another** computer (`NAME` = the allowed name):

```sh
# 1. Without sign-in Baruch redirects to the sign-in page: 303 and the address …/anmelden?next=…
curl -s -o /dev/null -w '%{http_code} %{redirect_url}\n' https://NAME/

# 2. The programming interface /rpc without sign-in: 401
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/rpc -H 'Origin: https://NAME' \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'

# 3. Setting up through the proxy is locked: 303 (accounts exist already) or 403 (none yet)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/einrichten
```

If check 1 returns **200** with the workspace, Baruch takes the proxy for its own computer (**On the own computer
without sign-in** is switched on, and the proxy rewrites `Host` without setting `X-Forwarded-For`): switch the
switch off at once and correct the proxy.

## Access for AI programs: `/mcp` with a token {#mcp-token}

**MCP** (Model Context Protocol) is the interface through which AI programs such as Claude Code work with Baruch;
it is at the address `/mcp`. How to connect such a program is described under
[MCP and assistants](/en/mcp/#mcp-clients).

Under an additionally allowed name a request **never counts as one from your own computer**: `/mcp` always
requires a token there, even if the proxy runs on the same computer. Without a token: error 401 (**checked**).

1. In the browser, through the proxy, open [Settings → MCP access](/en/hilfe/einstellungen/#mcp-zugang) and
   create a token. Baruch shows it exactly once and stores only a check value.
2. Under the allowed name the page shows the matching address and the command right away:

   ```sh
   claude mcp add -s local --transport http baruch https://baruch.example.org/mcp \
     --header "Authorization: Bearer <Token>"
   ```

   Baruch assumes the "https" here: TLS ends in the proxy, Baruch itself cannot see it.
3. Posts and proposals of this access appear under `@mcp:<name of the token>`. Revoking takes effect at once.

Directly on the computer (`http://127.0.0.1:8765/mcp`) the access stays without a token. This way no book text
can be changed or deleted; everything can be read.

If the proxy protects everything with client certificates or is only reachable in a VPN, `/mcp` needs no
exception: then both protective layers apply at once.

## Connections to the outside {#verbindungen}

On its own, Baruch opens only connections that are entered under
[Settings → Connections](/en/hilfe/einstellungen/#verbindungen): to the model service for the assistants, to
MultiGPT (a service for document search and web search) and to a database for the semantic search. Only the
assistant for sources searches the web, only through MultiGPT and only if its switch allows it. No address is
preset for MultiGPT: without an entered address the way is "not set up" and is never asked. Setting it up is
described in [MCP and assistants](/en/mcp/#multigpt-einrichten).

### Encryption to the database of the semantic search (pgvector) {#pgvector-tls}

Whoever runs the semantic search with a PostgreSQL database and its extension pgvector chooses under Settings →
Connections how the connection is secured (`disable`, `prefer`, `require`, `verify-full`). For this, Baruch reads
**environment variables** of the service when connecting: `PGSSLROOTCERT` (the root certificate as a PEM file),
`PGSSLCERT` and `PGSSLKEY` (certificate and key of the client) and `PGOPTIONS`. `PGPASSWORD` and `~/.pgpass` are
not read – the password comes only from the keyring.

**Read from the source code of the database library used, not checked against a server:**

| Setting | Certificate checked | Name checked | `PGSSLROOTCERT` |
|---|---|---|---|
| `disable` | – (unencrypted) | – | no effect |
| `prefer` | no | no | no effect; falls back to plain text without TLS |
| `require` | **no** | no | **no effect** – the file is not even read |
| `verify-full` | yes | yes | additional root next to those of the system |

- **`require` encrypts but does not protect** against an attacker in the network who poses as the database:
  Baruch accepts any certificate in this case. That differs from the usual PostgreSQL programs, which also check
  with `require` when a root file is given.
- Whoever wants protection chooses **`verify-full`**. If the server has a certificate from its own certificate
  authority, the service gets its root through the environment:

  ```ini
  # ~/.config/systemd/user/baruch.service, section [Service]
  Environment=PGSSLROOTCERT=/etc/ssl/own-ca/root.pem
  ```

  With `verify-full` and an unreadable or invalid file the connection fails with a message (assumption based on
  the source code).
- `PGSSLCERT` and `PGSSLKEY` apply with every encrypted setting and only together; if only one of the two is set,
  every encrypted connection to the database fails (assumption based on the source code).
- The note on the page Connections (about a stored password with `prefer`, `disable` or `require`) does not know
  these environment variables; it judges only by the chosen setting.

## What is different behind a proxy – and what is not {#proxy-unterschiede}

- **What stays the same:** the protection on saving against foreign web pages, the ban on embedding Baruch in
  foreign pages, and all limits for files and imports.
- **Without accounts everyone is "the author".** Baruch then distinguishes no users. Whoever gets past the access
  protection acts in the interface as the author – also when accepting, deleting, in the settings and for the
  backup.
- **With accounts** Baruch distinguishes the author and editors. An editor sees only shared works and writes only
  proposals, posts and findings; everything else Baruch refuses (error 403).
- **The notes of the interface** (theme, column widths and the like) contain no secrets. Only the note of the
  session is specially protected.
- **The address of your own computer still applies:** under `http://127.0.0.1:8765/` Baruch behaves as without a
  proxy, including the access for AI programs without a token. With accounts this address also requires a sign-in
  as long as **On the own computer without sign-in** is switched off.
- **Quitting Baruch** works only on the computer where it runs – not through the proxy.

## What is not tried {#nicht-erprobt}

What is checked is the behaviour of Baruch itself: host names, the check on saving, `/mcp` with and without a
token, sign-in, session and locks – with automatic tests and checks on your own computer. **Not tried**, so an
assumption:

- the examples for nginx and Caddy on a running proxy, including certificate, password prompt, client certificate
  and answers of the assistants through the proxy;
- the sign-in with accounts on a real proxy with TLS and in a real browser;
- everything about `PGSSLROOTCERT`, `PGSSLCERT` and `PGSSLKEY` against a PostgreSQL server with TLS;
- restoring a backup;
- operation under macOS and Windows behind a proxy.

What does not exist: TLS of its own in Baruch, sign-in with a second factor, resetting the password by e-mail and
several authors. Baruch also cannot detect a proxy that rewrites `Host` and sets no header of its own – hence the
checks.
