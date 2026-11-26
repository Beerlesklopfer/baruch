# Runde 35 – Baustein „rollen“ (Rollen je Werk, zentrale Rechteprüfung, Herkunft)

Baustein für den Integrator: Abschnitte für `docs/RPC.md`, `docs/Architektur.md` und `docs/MCP.md`. Belegt am Code des
Zweigs `team/rollen`; Tests in `crates/baruch-store/tests/migration_0054.rs`,
`crates/baruch-server/tests/round35_rollen.rs` und `crates/baruch-server/tests/round35_schreibwege.rs`.

## 1 Aufbau (für `docs/Architektur.md`)

- **Kontoart global, Rolle je Werk.** `roles::role_in(state, who, work)`: volle Rechte (`Who::Open`,
  `Who::LocalAuthor`, Autor-Konto) → `Role::Author` in jedem Werk; Lektor-Konto → `Role::Lektor` nur mit Zeile in
  `work_member` (Migration 0054); sonst `None` (Werk unsichtbar).
- **Eine Stelle der Rechteprüfung:** das Modul `roles` (`crates/baruch-server/src/roles/mod.rs`).
  - `roles::check_request` – jede Route, aufgerufen aus `auth::gate`. Lektor: nur Treffer in `LEKTOR_ROUTES`
    (Methode + Muster), sonst 403 „Abgelehnt: …“ mit Kopfzeile `x-baruch-abgelehnt: rolle`. HEAD zählt als GET;
    andere Verben als GET, HEAD, POST sind für Lektoren gesperrt.
  - `roles::check_rpc` – jede RPC-Methode, aufgerufen aus `rpc::call` vor jedem Dispatch (auch für unbekannte
    Namen), also für `/rpc` (einzeln und Batch), `/act` und `/lektorat/act`. Lektor: nur Methoden aus `LEKTOR_RPC`,
    sonst `-32003` „Das darf nur der Autor: <methode>“.
  - Alles, was nicht in den Tabellen steht, ist für Lektoren gesperrt – auch Routen und Methoden künftiger Runden.
    Die Formulare der Module (`/beitraege/act`, `/werke/befunde/act`, `/faden/act` …) rufen ihr Modul direkt und
    stehen darum nicht in `LEKTOR_ROUTES`.
- **Werk-Bindung.** Das Werk steht im Pfad (`{id}`), in der Adresse (`?work=`) oder im RPC-Parameter `work`.
  - Ein nicht freigegebenes Werk wird mit derselben Meldung abgelehnt wie ein unbekanntes („unbekanntes Werk N“;
    403 bzw. `-32003`) – ein Lektor erfährt nicht, welche Werke es gibt.
  - Fehlt das Werk, gilt **nie** das Vorgabewerk der Instanz: RPC setzt das kleinste freigegebene Werk ein; GET-Seiten
    mit `WorkFrom::Query` leiten mit 303 auf dieselbe Adresse mit `work=<kleinstes freigegebenes>` um; ohne
    freigegebenes Werk 303 auf `/werke` (RPC: `-32003` „Noch kein Werk freigegeben.“).
  - Ein `work` in der Adresse bzw. in den Parametern wird bei **jeder** erlaubten Route und Methode geprüft, auch wenn
    sie selbst kein Werk braucht (Werkwahl im Menüband). Die Adresse wird gelesen wie die Seiten sie lesen
    (prozentdekodiert; ein kodierter Name wie `wor%6B` zählt mit, mehrere `work` müssen alle freigegeben sein).
  - Ids (Beitrag, Befund, Faden, Bild, fester Schlüssel eines Abschnitts) sucht der Store immer im genannten Werk;
    eine id eines fremden Werks trifft unter dem freigegebenen nichts (Test
    `ids_of_a_foreign_work_yield_nothing_under_the_granted_work`).
- **Nichts wird gemerkt.** Jede Prüfung liest die Freigaben neu (`Store::member_works`, liefert nur für aktive
  Lektor-Konten etwas). Entzug der Freigabe und Sperre des Kontos wirken bei der nächsten Anfrage.
- **Sichtbarkeit.** `roles::visible_works` liefert dem Lektor nur freigegebene Werke (`work.list`, `/werke`,
  Werkwahl); `rpc::library_view` liefert Lektoren eine leere Literaturliste (keine Titel, keine Suche, keine Treffer,
  kein Bilderkasten) – für `ui.render` und die Seite `/`.
- **Herkunft.** Ein Lektor schreibt unter `@lektorat:<slug>` (`roles::author_name`), alle anderen weiter als „Autor“.
  - Vorschlag: `Store::add_proposal` mit Persona = Handle, Modell leer; Seitenzweig `refs/revisions/<key>/<n>`
    (`mirror::side_refs_after_proposal`) wie bei MCP-Vorschlägen, Trailer `Persona: @lektorat:<slug>`.
  - Beitrag: `post.author` = Handle; wie beim Autor kein Commit beim Anlegen. Bearbeiten/Löschen eigener Beiträge:
    ein Commit (`mirror::describe_post`), Autor des Commits ist der Handle, Committer „Autor“.
  - Befund: `finding.persona` = Handle; ein Commit „Befund <id>: angelegt von @lektorat:<slug>“
    (`mirror::describe_finding_added`, neu), Autor „<Anzeigename> (<Handle>)“
    `<lektorat-slug>@lektorat.baruch.invalid`, Trailer `Baruch-Finding`, `Baruch-Action: finding-add`,
    `Baruch-Node` (falls Abschnitt), `Baruch-Origin: lektor`, `Baruch-Lektor`.
- **Freigaben sind Instanzdaten:** Tabelle `work_member` (0054), nicht im Austauschformat, kein Commit im Spiegel;
  der Zeitpunkt steht in `granted`. Löschen eines Werks oder Kontos entfernt seine Freigaben (ON DELETE CASCADE).
- **Store (`baruch-store/src/members.rs`):** `member_list(work)`, `member_grant(work, account, role)` (nur
  Lektor-Konten, nur vorhandene Werke, idempotent – der Zeitpunkt der ersten Freigabe bleibt),
  `member_revoke(work, account)`, `member_works(account)` (aufsteigend; leer für gesperrte, unbekannte und
  Autor-Konten).

## 2 Erlaubte Routen für Lektoren (`roles::LEKTOR_ROUTES`)

| Methode | Muster | Werk aus | Begründung |
|---|---|---|---|
| GET | `/` | Adresse | Arbeitsseite: Abschnitt lesen, Vorschlag, Chat |
| GET | `/werke` | – | Werkliste, gefiltert über `roles::visible_works` |
| GET | `/beitraege` | Adresse | ganze Chat-Historie eines Abschnitts (ohne JavaScript) |
| GET | `/beitrag` | Adresse | ein Beitrag: kopieren, zitieren, eigenen bearbeiten |
| GET | `/faeden` | Adresse | Fäden lesen |
| GET | `/faden` | Adresse | einen Faden lesen (schreiben nicht: `/faden/act` gesperrt) |
| GET | `/suche` | Adresse | im Werk suchen (ersetzen nicht) |
| GET | `/gliederung` | Adresse | Gliederung lesen (ändern nicht: `/gliederung/act` gesperrt) |
| GET | `/werke/{id}/befunde` | Pfad | Befundseite, dort auch „Befund schreiben“ |
| GET | `/werke/{id}/fassungen/verlauf` | Pfad | Verlauf der Fortschreibungen lesen |
| GET | `/werke/{id}/glossar` | Pfad | Glossar lesen |
| GET | `/werke/{id}/register` | Pfad | Register lesen |
| GET | `/werke/{id}/bilder/{name}/datei` | Pfad | Bilder im Text der Arbeitsseite |
| GET | `/werke/{id}/bilder/{name}/vorschau` | Pfad | Vorschaubilder im Text |
| GET | `/ueber`, `/ueber/lizenz` | – | Über Baruch, Lizenz (AGPL: Hinweis auf den Quelltext) |
| GET | `/konto` | – | eigenes Konto (Passwort ändern, Abmelden) |
| GET | `/anmelden` | – | Anmeldeseite (nach dem Abmelden) |
| GET | `/einstellungen/darstellung` | Adresse | Darstellung im eigenen Browser (Cookie); alle anderen Einstellungsseiten gesperrt. **Abweichung:** mit Werk aus der Adresse statt „ohne Werk“ – ohne `work` zeigte das Menüband der Seite das Vorgabewerk der Instanz (Titel eines fremden Werks) |
| GET | `/static/app.css`, `/static/favicon.svg`, `/static/js/{name}` | – | Stil, Symbol, Widgets |
| POST | `/rpc` | – | JSON-RPC; jede Methode prüft `roles::check_rpc` |
| POST | `/lektorat/act` | – | Formularweg ohne JavaScript; ruft `rpc::call`, also dieselbe Prüfung |
| POST | `/abmelden` | – | Abmelden |
| POST | `/konto/passwort` | – | eigenes Passwort ändern |
| POST | `/einstellungen/darstellung/act` | – | Darstellung speichern (nur Cookies im eigenen Browser) |

Nicht erlaubt und darum 403: jeder Upload, `/act`, jedes Formular eines Moduls, jeder Export (PDF, EPUB, Word,
Austausch, Mindmap-Datei), Import, alle übrigen Einstellungen, Beenden, Literatur, Einfälle, Bilderseiten außer Datei
und Vorschau. `/mcp` bleibt bei Tokens und läuft nicht durch die Rollenprüfung.

## 3 Erlaubte RPC-Methoden für Lektoren (`roles::LEKTOR_RPC`)

„Werk“ = `work` aus den Parametern; fehlt es, wird das kleinste freigegebene eingesetzt.

| Methode | Werk | Art | Begründung |
|---|---|---|---|
| `work.get`, `node.get` | Werk | lesend | Werk und Abschnitt lesen |
| `revision.tree`, `revision.amends`, `revision.fold_get` | Werk | lesend | Fassungen, ihr Verlauf, Ausblendung |
| `ui.render` | Werk | lesend | Arbeitsseite ohne Neuladen (Literatur für Lektoren leer) |
| `menu.badges` | Werk | lesend | Zähler im Menüband |
| `post.page`, `post.since`, `post.versions` | Werk | lesend | Chat des Abschnitts |
| `finding.list`, `finding.get`, `finding.matrix` | Werk | lesend | Befunde |
| `thread.list`, `thread.get`, `thread.since`, `thread.post_versions` | Werk | lesend | Fäden lesen |
| `glossary.list`, `glossary.lookup`, `index.entries`, `index.overview` | Werk | lesend | Glossar und Register |
| `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` | Werk | lesend | Editor für den Vorschlag |
| `ribbon.formats` | Werk | lesend | Formate der Vorlage im Menüband |
| `work.sheet` | Werk | lesend | **ergänzt:** Satzbild des Werks für den Editor (nur lesen; `work.sheet_set` bleibt gesperrt) |
| `search.find` | Werk | lesend | **ergänzt:** Suche im Werk (Seite `/suche` und Suchleiste); `search.preview/replace/undo` bleiben gesperrt |
| `work.list` | – | lesend | gefiltert auf freigegebene Werke |
| `account.me`, `account.set_password`, `account.logout_all` | – | eigenes Konto | Konto des Lektors (Baustein „konten“) |
| `revision.propose` | Werk | schreibend | Vorschlag einer Fassung |
| `post.add`, `post.edit`, `post.delete` | Werk | schreibend | Beitrag; bearbeiten und löschen nur eigene |
| `finding.add` | Werk | schreibend | Befund bzw. „Anmerkung zum Buch“ |

Abweichungen vom Start des Vertrags (§2.2), gemeldet (dazu die Route `/einstellungen/darstellung` in §2):

1. `wysiwyg.*` und `ribbon.formats` stehen mit Werk-Bindung statt „ohne Werk“. Grund: `rpc::dispatch` gibt ihnen ohne
   `work` das Vorgabewerk der Instanz (`wysiwyg.parse` liest dessen Bildtexte, `wysiwyg.style` dessen Satzbild,
   `ribbon.formats` die Formate dessen Vorlage) – das wäre für einen Lektor ein fremdes Werk. Folge: Ein Lektor ohne
   freigegebenes Werk bekommt für sie `-32003`.
2. Ergänzt wurden nur die beiden rein lesenden Methoden `work.sheet` und `search.find` (Begründung in der Tabelle).
   Keine davon fasst ein Modell, eine Verbindung oder das Dateisystem an. Nicht ergänzt (offen für den Integrator bzw.
   „oberflaeche_konten“, falls die Arbeitsseite sie für Lektoren aufruft): `bible.marks` und `scripture.check` (lesen
   SWORD-Module aus dem Dateisystem), `timing.count`, `xref.targets`, `outline.check`, `index.registers`,
   `work.typeset_get`.

## 4 Neue RPC-Methoden (für `docs/RPC.md`)

Fehlercodes neu in Runde 35: `-32002` (`UNAUTHENTICATED`: es gibt Konten und die Anfrage trägt keine gültige Sitzung;
HTTP 401 an `/rpc`) und `-32003` (`FORBIDDEN`: die Rolle darf die Methode bzw. das Werk nicht; HTTP 200 an `/rpc`,
403 an `/lektorat/act`).

| Methode | Parameter | Antwort / Fehler |
|---|---|---|
| `revision.propose` | `{work?, key \| node, body, note?}` | `{work, node, key, rev, status: "proposal", author}`; nur Lektor (andere: -32000 mit Hinweis auf `revision.save`). Vorschlag unter `@lektorat:<slug>`, nie aktuelle Fassung; Markdown-Überschriften wie bei `revision.save`. Leerer Text, über 1 MiB, `note` über 2000 Zeichen → -32602; Text gleich der aktuellen Fassung → -32000; unbekannter Abschnitt → -32602 bzw. -32000 (Schlüssel fort). Kein Commit am Hauptzweig, Seitenzweig `refs/revisions/<key>/<rev>`. |
| `finding.add` | `{work?, key?, kind: "hint" \| "critical" \| "praise", body, quote?}` | `{finding, commit? \| mirror?}` (`finding` wie in `finding.get`, `persona` = Handle, `status: "open"`, ohne Beitrag); ohne `key` „Anmerkung zum Buch“ (Befund ohne Abschnitt). Nur Lektor (andere: -32000). Leerer Text, unbekannte Art, Abschnitt nicht im Werk → -32602. Genau ein Commit im Spiegel. |
| `member.list` | `{work}` | `{work, members: [{account, name, display, handle, role, granted, active}], candidates: [{account, name, display, handle}]}`; `candidates` = aktive Lektor-Konten ohne Freigabe. Nur volle Rechte (Lektor: -32003). `work` ist Pflicht (kein Vorgabewerk). |
| `member.grant` | `{work, account, role?: "lektor"}` | `{work, account, role, granted}`; idempotent. Unbekanntes Werk oder Konto, andere Rolle → -32602; Autor-Konto → -32000. Nur volle Rechte. Kein Commit. |
| `member.revoke` | `{work, account}` | `{work, account, revoked}` (`false`, wenn es keine Freigabe gab). Nur volle Rechte. Kein Commit. |

Geändert:

- `post.add`: Autor des Beitrags ist `roles::author_name` – für Lektoren der Handle, sonst „Autor“ wie bisher.
- `post.edit`, `post.delete`: Lektoren nur eigene Beiträge (Autor des Beitrags = Handle); fremde und die des Autors
  → -32000 mit der Meldung „Nur eigene Beiträge lassen sich bearbeiten: …“. Der Autor verwaltet weiter jeden Beitrag.
- `post.page`, Seiten `/beitraege` und `/beitrag`: `own` ist für Lektoren „unter dem eigenen Handle“.
- `work.list`: für Lektoren nur freigegebene Werke.
- `ui.render`: für Lektoren ohne Literatur.
- Jede Methode außerhalb von `LEKTOR_RPC` (auch unbekannte Namen): für Lektoren -32003.

## 5 Routen (für `docs/RPC.md` „Formulare“)

- `POST /lektorat/act` (hinter `guard::same_origin`): Felder `method`, `work`, `key`, `node`, `id`, `body`, `note`,
  `kind`, `quote`, `confirm` (`1` | `on` | `true`), `back`. Ruft `rpc::call` – dieselbe Rechteprüfung wie `/rpc`.
  Angenommen werden nur Methoden aus `LEKTOR_RPC` (auch vom Autor: sonst 403). Fehlt `work`, gilt für Lektoren das
  kleinste freigegebene Werk. Danach 303: nach `revision.propose` auf `/?work=W&key=K&rev=N`, sonst auf den Abschnitt
  `/?work=W&key=K`; mit `back=befunde` (und nach `finding.add` ohne `key`) auf `/werke/W/befunde`; mit `back=post`
  auf `/beitrag?work=W&id=N` (nach `post.delete` auf den Abschnitt). Fehler: 403 „Abgelehnt: …“ mit
  `x-baruch-abgelehnt: rolle` (`-32003`), 401 (`-32002`), 500 (intern), sonst 422 „Fehler: …“.
- `POST /werke/{id}/mitwirkende/act` (nur volle Rechte): `action` = `grant` | `revoke`, `account`. 303 auf
  `/werke/{id}/mitwirkende?ok=freigegeben` bzw. `?ok=entzogen`; `?fehler=unbekanntes_konto` (Konto fehlt, unbekannt
  oder keine Zahl) bzw. `?fehler=kein_lektor` (Freigabe für ein Autor-Konto). Unbekannte Aktion 400, unbekanntes Werk
  422.

## 6 MCP (für `docs/MCP.md`)

Kein neues Werkzeug. `/mcp` bleibt bei Tokens und läuft als `Who::Open` an der Rollenprüfung vorbei; MCP-Clients
schreiben weiter als `@mcp:<name>`. Ein Lektor ist kein MCP-Client: Commits seiner Befunde tragen bewusst keinen
Trailer `Persona`, sondern `Baruch-Origin: lektor` und `Baruch-Lektor`.

## 7 Annahmen und offene Punkte

- **Cookie `baruch_work` bei der Umleitung (Vertrag §2.1):** `check_request` bekommt nach der festen S1-Signatur
  keine Kopfzeilen und kann das Cookie nicht lesen. Umgeleitet wird darum immer auf das kleinste freigegebene Werk.
  Soll das Cookie zählen, braucht `check_request` die Kopfzeilen (Änderung an S1/`auth::gate`).
- **Kein Zähler für offene Vorschläge:** MCP begrenzt offene Vorschläge je Client; für Lektoren (Menschen) gibt es
  keine solche Grenze, nur die Größen (1 MiB Text, 2000 Zeichen Notiz).
- **Lektoren und der Faden-Chat:** lesen ja, schreiben nein (kein Modellauftrag, Vertrag §0.7).
- **`/lektorat/act` für den Autor:** nimmt nur Methoden aus `LEKTOR_RPC`; `revision.propose`/`finding.add` lehnt der
  Server für ihn mit Hinweis ab. Der Autor nutzt wie bisher `/act` und die Formulare der Module.
- **Seiten ohne eigenes Werk** (`/ueber`, `/konto`, `/anmelden`): Welches Werk ihr Menüband zeigt,
  entscheidet die jeweilige Seite (S1: `visible_works`); ein fremdes `?work=` lehnt die Rollenprüfung ab. `/ueber`
  ist geprüft (nennt kein fremdes Werk); `/konto` und `/anmelden` kommen von „oberflaeche_konten“ und sind dort zu
  prüfen. `/einstellungen/darstellung` zeigte ohne `work` das Vorgabewerk im Menüband und steht darum mit Werk aus
  der Adresse in der Tabelle (Umleitung auf das freigegebene Werk; ein Lektor ohne Freigabe landet auf `/werke`).

## 8 Handprüfung (nach dem Zusammenführen, mit echten Konten)

1. Als Autor ein Lektor-Konto anlegen, unter „Mitwirkende“ eines Werks freigeben.
2. Als Lektor anmelden: Werkliste zeigt nur dieses Werk; `/` führt auf `/?work=<Werk>`.
3. Abschnitt öffnen, „Vorschlagen“: Der Vorschlag erscheint mit `@lektorat:<name>`, die aktuelle Fassung bleibt.
4. Beitrag schreiben, bearbeiten, löschen; einen Beitrag des Autors bearbeiten wollen → Meldung „Nur eigene …“.
5. Befundseite: „Befund schreiben“ und „Anmerkung zum Buch“; Status-Knöpfe fehlen bzw. antworten 403.
6. Adresse von Hand auf ein anderes Werk ändern (`?work=`, `/werke/<n>/befunde`) → 403 „unbekanntes Werk“.
7. `/einstellungen/daten`, `/act`, ein Export → 403.
8. Als Autor den Vorschlag übernehmen; Freigabe entziehen → der Lektor bekommt bei der nächsten Anfrage 403.
9. Im Spiegel (`git log`, `git for-each-ref refs/revisions`) Herkunft des Vorschlags und des Befunds nachsehen.
