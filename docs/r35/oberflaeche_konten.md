# Runde 35 – Baustein „oberflaeche_konten“

Seiten, Menüband und Arbeitsseite für Konten und Rollen (Vertrag §3). Der Integrator setzt daraus die Abschnitte in
`docs/Architektur.md` und `docs/RPC.md` ein. Bedienung steht schon in `docs/Bedienung.md` („Konten und Rollen“),
Betrieb in `docs/Betrieb.md` („Anmeldung und Konten“).

## Aufbau

| Datei | Inhalt |
|---|---|
| `src/konten_ui/mod.rs` | `login_page`/`login_html`, `setup_page`/`setup_html`, `safe_next`, `MESSAGES` mit `ok_message`/`error_message`, `apply`, Marken `KONTO_MARK`, `LEKTOR_OPEN/CLOSE`, `KONTEN_OPEN/CLOSE`, eigene minijinja-Umgebung (die 38.) |
| `src/konten_ui/pages.rs` | Routen `GET /anmelden`, `/einrichten`, `/konto`, `/werke/{id}/mitwirkende`; Inhalt der Einstellungsseite „Konten“ (`render_settings`); reine Darstellungsfunktionen `accounts_html`, `members_html`, `account_html` über Ansichtsdaten; `direct_from_own_machine` |
| `src/view.rs` | `values` liest `auth::current()`; für Lektoren eigene Vorlagen der drei Bereiche (`pane_templates`), `post.mine`, Beiträge ohne Befund-Karten, Herkunft `@lektorat:…` ohne leeres Modell |
| `src/consent.rs` | Hinweisleiste und Dialog nur bei vollen Rechten |
| `src/settings/mod.rs` | achte Seite `konten` in `PAGES`; Navigation des Rahmens für Lektoren nur „Darstellung“ |
| `templates/anmelden.html`, `einrichten.html` | eigenständige Seiten ohne Menüband und ohne Skript |
| `templates/konto.html`, `mitwirkende.html` | erweitern `base.html` (Reiter Start bzw. Buch) |
| `templates/settings/konten.html` | Inhalt der Einstellungsseite |
| `templates/fragments/lektor_tree.html`, `lektor_editor.html`, `lektor_assistants.html` | Arbeitsseite eines Lektors |
| `static/konten.css` | Regeln der Rollen und Gestaltung der neuen Seiten (nur Variablen aus `app.css`) |
| `web/src/settings.ts`, `menu.ts` | Lektor: „Einstellungen“ bleibt ein Link (kein `settings.render`), Pfeiltasten überspringen ausgeblendete Einträge |

### `konten_ui::apply(page, who, accounts_exist)`

Läuft in `consent::inject` nach `band::apply` über jede Seite mit der Marke `<!--BARUCH-KONTO-->` (sie steht in
`base.html` im Menü „Baruch ▾“ nach „Baruch beenden…“). Seiten ohne Marke bleiben Byte für Byte gleich.

| Zustand | Menüeintrag | `<body>` | Kennzeichen |
|---|---|---|---|
| ohne Konten | „Konten einrichten…“ (`/einrichten`) | unverändert | ` nur-autor"` → `"`, ` data-nur-autor` entfernt, Stücke `NUR-LEKTOR` und `NUR-KONTEN` herausgeschnitten |
| Autor-Konto | „Angemeldet: <Anzeigename> (Autor)“, „Mein Konto“, Formular „Abmelden“ (`POST /abmelden`) | `data-rolle="autor"` | `NUR-LEKTOR` herausgeschnitten, `NUR-KONTEN` bleibt (ohne Klammern) |
| Lektor-Konto | wie oben mit „(Lektor)“ | `data-rolle="lektor"` | `NUR-LEKTOR` und `NUR-KONTEN` bleiben (ohne Klammern) |
| `LocalAuthor` | „Autor (eigener Rechner, ohne Anmeldung)“, „Anmelden“ | `data-rolle="autor"` | wie Autor-Konto |
| `Anonymous` | „Anmelden“ | unverändert | wie Autor-Konto |

Damit ist eine Seite ohne Konten bis auf den einen Menüeintrag dieselbe wie vor Runde 35. Geprüft mit einem
Byte-Vergleich von 23 Seiten und `ui.render` vor und nach der Änderung (Ziffern genullt): Unterschiede nur der
Menüeintrag, der achte Eintrag „Konten“ in der Navigation der Einstellungen und die Anzeige einer Herkunft
`@lektorat:…` (vorher „Persona @lektorat:erika ()“, jetzt „@lektorat:erika“ – ohne Konten kommt sie nicht vor).

Regel für Vorlagen: `nur-autor` ist immer die **letzte** Klasse eines Elements (`class="btn nur-autor"`), sonst
findet `apply` sie ohne Konten nicht. `apply` nimmt die Kennzeichen nur aus **Tags** heraus, nie aus Text zwischen
den Tags – Buchtext im Textfeld oder Satzbild bleibt unberührt, auch wenn er zufällig dieselben Zeichen enthält. Gruppen des Menübands tragen statt der Klasse das Attribut `data-nur-autor`
(ihr Tag muss wörtlich `<details class="band-group"` … ` data-band-group open>` bleiben, `band::apply`).

### Was ein Lektor sieht

- **Arbeitsseite (serverseitig):** `view::page`/`view::panes` nehmen für Lektoren `fragments/lektor_*.html`. Darin:
  Formular `form#editor-form` an `POST /lektorat/act` mit `method=revision.propose`, `work`, `key`, `body`, `note`
  (mit JavaScript dieselbe Methode über `/rpc`, `data-rpc` in `workspace.ts`), Knopf „Vorschlagen“; rechts
  „Befund schreiben“ (`finding.add`: `work`, `key`, `kind`, `body`, `quote`), Beitrag (`post.add`: `work`, `key`,
  `body`), eigene Beiträge ändern/entfernen (`post.edit`: `id`, `body`; `post.delete`: `id`, `confirm`). Es fehlen
  im HTML: Speichern, Als neue Fassung, Übernehmen, Verwerfen, Umschalten („aktuell machen“, „Diese Fassung
  verwenden“), Löschen, Ausblenden, Personas und ihre Formulare, Lösungen, Verschieben/Umwandeln von Beiträgen,
  Bild einfügen, Markierung, Zitate prüfen, die Dialoge Literatur/Werk anlegen/Einfall, Reiter und Feld „Einfügen“,
  das Widget „outline“ (Umstellen der Gliederung) und das Widget „chat“ (siehe Annahmen). Befund-Blöcke in
  Beiträgen der Personas liest der Lektor als Text des Beitrags, ohne die Karte mit den Status-Knöpfen.
- **Übrige Seiten (CSS, ohne JavaScript):** `konten.css` blendet für `body[data-rolle="lektor"]` aus: `.nur-autor`,
  `[data-nur-autor]` und **jedes POST-Formular**, dessen `action` nicht `/lektorat/act`, `/abmelden`,
  `/konto/passwort` oder `/einstellungen/darstellung/act` ist (dieselbe Regel wie im Server: alles verboten, was nicht
  erlaubt ist – neue Formulare künftiger Runden sind für Lektoren von selbst verborgen). `.nur-lektor` ist nur dort
  sichtbar. Gekennzeichnet sind in `base.html`: Neues Werk/Import (3 Einträge), Austausch einlesen, Beenden, die
  Gruppen Einfälle, Werkzeuge, Quellen, Satz, Ausgabe, Umschlag und die Knöpfe Zitate prüfen, Querverweise, Werk
  bearbeiten, Anträge, Gliederungsschema, Bilder, Mitwirkende; in `works.html` die Gruppe „Werke“ und Ändern, Export,
  PDF, Löschen; auf der Befundseite die Formulare zu Status, Entscheidung und Lösungen (Makro `card(…, na)`).
- **Befundseite:** Formular „Befund schreiben: Anmerkung zum Buch“ (`finding.add` ohne `key`, `back=befunde`) als
  `nur-lektor`, nur für Lektoren im HTML.
- **Einstellungen:** In der Navigation steht für Lektoren nur „Darstellung“.
- **Zustimmung:** Hinweisleiste und Dialog (`consent`) nur bei vollen Rechten.

### Seiten

| Route | Verhalten |
|---|---|
| `GET /anmelden[?next=…][&ok=abgemeldet]` | ohne Konten 303 `/`; sonst Formular `POST /anmelden` (`name`, `password`, `next`), Link „Weiter“ auf `safe_next(next)`; mit Sitzung „Angemeldet als …“ |
| `GET /einrichten` | mit Konten 303 `/einstellungen/konten`; nicht vom eigenen Rechner 403 mit Erklärung (ohne Formular); sonst Formular `POST /einrichten` (`name`, `display`, `password`, `password2`) |
| `GET /konto[?ok=…][&fehler=…]` | Name, Rolle, Herkunft, Werke (`roles::visible_works`), `POST /konto/passwort` (`current`, `password`, `password2`), `POST /abmelden`; ohne Konten Erklärung; `LocalAuthor` Hinweis |
| `GET /werke/{id}/mitwirkende[?ok=…][&fehler=…]` | nur volle Rechte (sonst 403), unbekanntes Werk 404; Daten aus `rpc::call("member.list", {work})`; Formulare `POST /werke/{id}/mitwirkende/act` (`action=grant|revoke`, `account`) |
| `GET /einstellungen/konten[?ok=…][&fehler=…]` | Liste aus `Store::account_list`, Schalter aus `account.me` (`local_open`); Formulare `POST /einstellungen/konten/act` mit `action=create` (`name`, `display`, `password`, `password2`), `deactivate`/`activate` (`id`), `reset_password` (`id`, `password`, `password2`), `local_open` (`open=0|1`); ohne Konten Erklärung und „Einrichten“ |

`?ok=`/`?fehler=` zeigen nur den Satz aus `MESSAGES` (ein Erfolgscode gilt nicht als Fehler und umgekehrt, Unbekanntes
zeigt nichts). Alle Konto-Seiten antworten mit `Cache-Control: no-store`.

## Annahmen (bitte beim Zusammenführen prüfen)

1. **`guard::forwarded` gibt es in meinem Zweig noch nicht** (Besitzer konten). `konten_ui::pages` trägt darum die
   sechs Kopfzeilen aus §0.2 selbst (`FORWARD_HEADERS`, `direct_from_own_machine`). Der Integrator kann die Stelle
   auf `guard::forwarded` umstellen – die Regel ist dieselbe.
2. **`account.me`** liefert `local_open` (Vertrag §1.6); scheitert der Aufruf, zeigt die Seite „ausgeschaltet“.
3. **`member.list`** in der Form des Vertrags (§2.3); eine unlesbare Antwort zeigt die Seite als Fehler.
4. **`/einstellungen/konten/act`** bekommt genau die Felder des Vertrags – kein `work`. Nach der Aktion fehlt das
   Werk in der Adresse (es kommt dann aus dem Cookie `baruch_work`).
5. **`/lektorat/act`** leitet nach `revision.propose`, `post.*` und `finding.add` zurück auf den Abschnitt
   (`view::node_url`) bzw. mit `back=befunde` auf die Befundseite. Die Lektor-Formulare der Arbeitsseite schicken
   kein `back`.
6. **`revision.propose`** über `/rpc` antwortet mit `{key, rev, …}`; `workspace.ts` zeigt danach diese Fassung.
7. **Live-Chat:** Für Lektoren fehlt das Widget „chat“ (neue Beiträge erscheinen nach dem Neuladen): `post.since`
   liefert Beiträge mit den Werkzeugen des Autors (`post_entry`), die ein Lektor nicht nutzen kann.
8. **Seiten `/beitrag` und `/beitraege`:** Ein Lektor liest dort; seine eigenen Beiträge ändert und entfernt er nur auf
   der Arbeitsseite. Die Formulare des Autors sind dort über die CSS-Regel für POST-Formulare verborgen.
9. **Anzeigename bei der Herkunft:** Fassungen, Beiträge und Befunde zeigen `@lektorat:<slug>` (wie `@word:<name>`).
   Den Anzeigenamen dazu gibt es auf „Mein Konto“, „Mitwirkende“ und in den Einstellungen – auf der Arbeitsseite
   nicht (die Ansichtsdaten `LibraryView` kennen keine Konten; Besitzer `rpc.rs`).
10. **Links auf gesperrte Seiten** (z. B. Umschlagbild in der Werkliste, „Zitate prüfen“ in älteren Seiten) können für
    Lektoren stehen bleiben, wo sie nicht in `base.html` liegen; sie enden mit 403 „Abgelehnt: …“.

## Angepasste ältere Tests

| Datei | Warum |
|---|---|
| `tests/round9_settings.rs`, `tests/round9_integration.rs`, Unit-Test in `settings/mod.rs` | acht statt sieben Einstellungsseiten (Vertrag §3.1) |
| `tests/round24_pakete.rs` | hinter „Baruch beenden…“ folgt der Eintrag zum Konto (Vertrag §3.2) |
| `tests/round32_abnahme.rs` | die Einträge „Word (.docx) importieren…“ und „Baruch beenden…“ tragen in `base.html` die Klasse `nur-autor`; hinter der Marke `R32-PLATZ beenden` folgt die Marke für den Eintrag zum Konto |
| `tests/round33_laptop.rs` | `konten.css` steht in `STYLE` hinter `laptop.css` (S1); der Test verlangt jetzt, dass `laptop.css` hinter dem Grundgerüst steht und danach nur `konten.css` folgt (das die Zustände des Laptop-Modus nicht anfasst – geprüft in `round35_oberflaeche`) |
| `tests/round33_seiten.rs` | `konto.html` und `mitwirkende.html` in `TEMPLATES` |

## Tests

`tests/round35_oberflaeche.rs` (18 Tests) und die Unit-Tests in `konten_ui/mod.rs` und `konten_ui/pages.rs`. Die
Identität kommt über `common::with_who` (ganzer Router) bzw. `auth::scope` um die Seiten selbst – so hängen die Tests
nicht daran, ob `roles` die Route für Lektoren schon öffnet. Nicht geprüft: ein echter Browser, ein echter Proxy, das
Zusammenspiel mit den echten Schreibwegen (`/lektorat/act`, `/anmelden` …) – das ist Sache von `round35_abnahme`.

## Handprüfung für den Nutzer (echter Browser, Proxy mit TLS)

Vorher eine Sicherung anlegen oder mit einem eigenen Datenordner prüfen (`--data`).

**Ohne Konten**

1. Baruch öffnen: alles wie bisher? Im Menü „Baruch ▾“ steht neu „Konten einrichten…“.
2. Einstellungen → Konten: Erklärung und Knopf „Einrichten“.

**Einrichten (am Rechner selbst, `http://127.0.0.1:8765`)**

3. „Konten einrichten…“: Formular erscheint. Zwei verschiedene Passwörter → Meldung, Name bleibt stehen.
4. Konto anlegen → Einstellungen → Konten mit „Das Konto ist angelegt.“; Menü zeigt „Angemeldet: … (Autor)“.
5. Über den Proxy (`https://NAME/einrichten`) bzw. in einem privaten Fenster: Umleitung auf die Konten bzw. Anmeldung.
6. Passwortmanager des Browsers: bietet er beim Anlegen das Speichern und beim Anmelden das Ausfüllen an?

**Autor**

7. Abmelden → Anmeldeseite mit „Abgemeldet.“; falsches Passwort viermal → Wartezeit wird genannt.
8. Lektor anlegen (Einstellungen → Konten), Werk freigeben (Reiter Buch → Mitwirkende), wieder entziehen.
9. Sperren und Entsperren; Passwort zurücksetzen (aufklappbares Formular in der Zeile).
10. „Am eigenen Rechner ohne Anmeldung“ ein- und ausschalten: Menü zeigt „Autor (eigener Rechner, ohne Anmeldung)“.
    **Über den Proxy muss trotzdem die Anmeldeseite kommen.**
11. Dunkles Thema und Laptop-Modus: Anmeldeseite, Mein Konto, Konten, Mitwirkende lesbar, nichts läuft aus dem Fenster.

**Lektor (zweiter Browser oder privates Fenster, über den Proxy)**

12. Anmelden: nur das freigegebene Werk in der Werkwahl und unter „Alle Werke“; ohne Freigabe „Noch kein Werk
    freigegeben.“.
13. Arbeitsseite: kein „Speichern“, kein Reiter „Einfügen“, keine Assistenten rechts; im Menü „Baruch ▾“ nur Alle
    Werke, Einstellungen, Über Baruch, Mein Konto, Abmelden.
14. Text ändern → „Vorschlagen“ (mit Notiz): Der Vorschlag erscheint als neue Fassung „Vorschlag · @lektorat:…“.
    Dasselbe mit abgeschaltetem JavaScript.
15. Beitrag senden; eigenen Beitrag unter „Verwalten“ ändern und entfernen; beim Beitrag des Autors fehlt „Verwalten“.
16. „Befund schreiben“ rechts; auf der Seite Befunde „Anmerkung zum Buch schreiben“; dort keine Status-Knöpfe.
17. Adresse von Hand: `/werke/<id>/pdf`, `/einstellungen/werk`, `/literatur` → „Abgelehnt: …“ (403).
18. „Einstellungen“ führt auf Darstellung (kein Dialog); Thema umschalten wirkt.
19. Als Autor: Vorschlag des Lektors übernehmen bzw. verwerfen; im Git-Spiegel steht die Herkunft.
20. Link auf eine Baruch-Seite aus einer E-Mail anklicken, während man angemeldet ist: Anmeldeseite mit „Schon
    angemeldet? Weiter“ → der Klick führt ohne neue Anmeldung ans Ziel.

**Notweg**

21. `echo 'neues-langes-passwort' | baruch-server passwort <Name> --data <Ordner>`; danach anmelden.
