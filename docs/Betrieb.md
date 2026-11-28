# Operation

**The description for operators is on the website:** <https://baruch.bernau.family/betrieb/> (in German) – the
source is `site/content/de/betrieb.md` (since Round 37). It covers the data folder, backup and restore, permanent
operation and updating, the three levels of reachability, operation behind a reverse proxy with the examples
for nginx and Caddy and the mandatory test, login and accounts, `/mcp` with a token, outgoing connections and
TLS to pgvector.

**Rule for developers:** "updating the operation docs" means changing `site/content/de/betrieb.md` in the same
commit. This file here contains **no steps for operators**, only what nobody needs there: the state of
verification with the tests, open points and technical notes. Nothing is in both files.

Installation and packages: `docs/Verteilung.md`. Usage: the help under `site/content/de/hilfe/` (entry point
`docs/Bedienung.md`). Structure of the protective layers: `docs/Architektur.md`, section "Protection of the interfaces".

## Where is what

| Topic | on the website |
|---|---|
| The most important things first | `/betrieb/#das-wichtigste` |
| Data folder, permissions, keyring | `/betrieb/#datenordner` |
| Backup, restore, taking a work along | `/betrieb/#sicherung`, `#wiederherstellen`, `#werk-mitnehmen` |
| Permanent operation, updating, keyring without a desktop | `/betrieb/#dauerbetrieb`, `#schluesselbund` |
| Reachability in three levels, rejected host names | `/betrieb/#erreichbarkeit`, `#rechnernamen` |
| Behind a reverse proxy: allowing names, rules, nginx, Caddy, test | `/betrieb/#reverse-proxy`, `#namen-erlauben`, `#regeln`, `#nginx`, `#caddy`, `#probe` |
| Login and accounts, forgotten password, test with accounts | `/betrieb/#konten`, `#passwort-vergessen`, `#probe-konten` |
| `/mcp` with a token | `/betrieb/#mcp-token` |
| Outgoing connections, TLS to pgvector | `/betrieb/#verbindungen`, `#pgvector-tls` |
| What is different behind a proxy | `/betrieb/#proxy-unterschiede` |
| What has not been tried (for operators) | `/betrieb/#nicht-erprobt` |

## State of verification

Verified (tests `crates/baruch-server/tests/round24_betrieb.rs`, unit tests in `guard.rs`, a server of our own
with `curl`):

- Without `--allow-host` a name stays 421; with an allowed name `Host: name` and `Host: name:8443` pass,
  another name, a name with spaces or a wildcard does not.
- A POST with `Origin: https://name` and `Host: name` passes the check; with a rewritten `Host` 403.
- `/mcp` under the allowed name from loopback: 401 without a token, 200 with a token; under `127.0.0.1` as
  before without a token.
- Invalid names end `serve` with exit 2 and a message, both as an argument and as an environment variable.
- The page for MCP access shows address and command with a token under the allowed name.
- Accounts (Round 35): login, session and locks with tests against the server.

Not verified, hence an assumption (the list for operators is under `/betrieb/#nicht-erprobt`; here the
technical side of it):

- the examples for nginx and Caddy on a running proxy, including certificate, basic auth, client certificate,
  HTTP/2 to the outside and live answers through the proxy;
- everything about `PGSSLROOTCERT`, `PGSSLCERT`, `PGSSLKEY` against a PostgreSQL server with TLS (finding only
  from the source code, see below);
- restoring a backup;
- operation under macOS and Windows behind a proxy;
- accounts on a real proxy with TLS and in a real browser (manual checklist: `docs/Architektur.md`,
  "Round 35 – Accounts and roles").

## Open

- Baruch cannot detect a proxy that rewrites `Host` and sets no proxy header (see the
  tests on the website).
- Baruch has no TLS of its own; only the session cookie carries `Secure`.
- There is no two-factor login, no reset by e-mail and no support for several authors.
- The TLS hint of the pgvector connection does not know the environment variables.
- The hint texts in the program (`src/main.rs`, help for `--allow-host`; `templates/settings/mcp.html`) and
  `docs/MCP.md` still name `docs/Betrieb.md`; this file leads from there to the website. Whether they name
  the address `/betrieb/` directly in future is up to the user.

## Technical notes

What the website says in everyday language, here with the names from the program.

### Data folder and backup

- The backup writes the database with `VACUUM INTO` (a consistent state despite running operation); `-wal` and
  `-shm` are the companion files of SQLite.
- The tokens for `/mcp` are stored as SHA-256 hashes in the database.
- Secrets: keyring service `baruch`; switch `--secrets keyring|memory` or `BARUCH_SECRETS`.
- New data folders are created with 0700; `baruch.sqlite*` and `works/` are created with the umask of the service.
- „Sicherung angelegt“ (backup created) is a one-time note in the running process, not a value in the database.
- The example for the service is in the repository under `packaging/systemd/` (the handling of the
  service is described there as well), in the package under `/usr/share/doc/baruch/examples/baruch.service`.
- The page „Über Baruch“ (About Baruch) is `/ueber`.

### Host names and proxy

- The check of writing requests is `guard::check`: `Origin` against `Host`, plus `Sec-Fetch-Site`; reading
  requests from foreign pages as a subresource are rejected, framing is forbidden.
- `--allow-host` only applies to the command `serve`. Names like `0x7f.1` (last part a number in a different
  notation) and `baruch.example.org.evil.example` (only the exact name counts) are rejected as well.
- A request without a `Host` header does not count as local once names are allowed.
- Baruch evaluates `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` and `Forwarded` neither for the
  peer nor for the scheme; the scheme `https` shown on the page for MCP access is an assumption.
- From a peer outside loopback Baruch lets only `/mcp` through (403 for everything else).

### Accounts

- Decisions of Round 35: 0.2 (with accounts the login also applies on loopback; exception only as a switch
  of the author) and 0.3 (direct access from other machines stays locked).
- Session cookie `baruch_session`: `HttpOnly`, `SameSite=Strict`, through a name from `--allow-host`
  additionally `Secure`; only a hash of the session key is stored. Baruch sets the remaining cookies of the user
  interface (`baruch_*`, settings only) without `Secure`.
- The login page and the setup page follow the cookie `baruch_theme` (light/dark), without JavaScript.
- The throttle answers with 429 and `Retry-After`; it counts per account and – if the request does not come
  through the proxy – per address.
- Passwords: Argon2id verifier in the table `account`.
- `/einrichten` answers with 403 through the name of the proxy or with a proxy header; if accounts already
  exist, with 303.
- The login applies to pages, forms and `/rpc`; the only place of the permission check for editors (Lektoren) is
  `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`).

### TLS to pgvector

- The finding on the website comes from the source code of sqlx 0.8.6 (`sqlx-postgres/src/connection/tls.rs`,
  `sqlx-core/src/net/tls/tls_rustls.rs`), **read, not verified against a server**. Since Round 36
  Baruch runs with sqlx 0.9.1; the place in `sqlx-postgres-0.9.1/src/connection/tls.rs` was re-read on
  2026-11-27 and behaves the same (`accept_invalid_certs` applies to everything except `verify-ca` and
  `verify-full`, `accept_invalid_hostnames` to everything except `verify-full`).
- The backlog carried the assumption that with `PGSSLROOTCERT` sqlx also verifies under `require` (this is how
  libpq behaves: if a root file is present, it verifies under `require` as under `verify-ca`). According to the
  source code this does not hold for sqlx.
- Baruch does not read `PGPASSWORD` and `~/.pgpass`; the password only comes from the keyring.
