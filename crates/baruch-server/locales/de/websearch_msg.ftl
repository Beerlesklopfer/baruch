# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the web search of @belege (src/websearch/**): evidence page, adopting into the library, notes of a
# search run, refused addresses (Round 39a, task messages_2).

# Evidence page /websuche/belege.
websearch_msg.page.adopted = Der Beleg ist in der Autorenbibliothek: Titel, Zitat und Hervorhebung.
websearch_msg.page.not_proven = Nur belegte Zitate kommen in die Bibliothek: Dieser Wortlaut steht nicht auf der abgerufenen Seite (oder es gibt kein Zitat). Nichts angelegt.
websearch_msg.page.already = Dieser Beleg ist schon in der Bibliothek.
websearch_msg.page.unknown = Diesen Beleg gibt es nicht.
websearch_msg.page.unknown_action = Unbekannte Aktion; nichts geändert.
websearch_msg.page.failed = Das hat nicht geklappt; nichts angelegt.
websearch_msg.page.post_missing = post fehlt (Beitrag).
websearch_msg.page.error = Fehler: { $detail }
websearch_msg.page.template_error = Fehler: Vorlage: { $detail }

# websearch.* over /rpc and adopting (src/websearch/mod.rs, evidence.rs).
websearch_msg.rpc.number_expected = { $name }: Zahl erwartet
websearch_msg.rpc.missing = { $name } fehlt
websearch_msg.rpc.on = on: true oder false erwartet
websearch_msg.evidence.store = Speicher: { $detail }
websearch_msg.evidence.unknown = Beleg { $id } gibt es nicht.
websearch_msg.evidence.not_proven = Beleg { $id } ist nicht belegt; in die Bibliothek kommen nur Zitate, deren Wortlaut auf der abgerufenen Seite steht.
websearch_msg.evidence.no_quote_verdict = Beleg { $id } ist ohne Zitat; in die Bibliothek kommen nur Zitate, deren Wortlaut auf der abgerufenen Seite steht.
websearch_msg.evidence.adopted = Beleg { $id } ist schon übernommen (Titel { $title }).
websearch_msg.evidence.no_quote = Beleg { $id } hat kein Zitat.

# Retrieval (src/websearch/fetch.rs): reasons why an address is not retrieved.
websearch_msg.fetch.scheme = nur http- und https-Adressen werden abgerufen
websearch_msg.fetch.userinfo = Adresse enthält Benutzerangaben
websearch_msg.fetch.host = Adresse ohne Rechner
websearch_msg.fetch.address = Adresse im eigenen Netz (Loopback, privat oder Link-Local)
websearch_msg.fetch.name = Rechnername des eigenen Netzes (localhost, .local, .internal, .home.arpa oder ohne Punkt)
websearch_msg.fetch.no_url_param = { $tool } nennt im inputSchema keinen Parameter für die Adresse ({ $keys }).
websearch_msg.fetch.too_big = Seite ist größer als { $max } MiB ({ $size } KiB)
websearch_msg.fetch.no_text = { $url }: die Seite enthält keinen Text

# Notes of a search run (src/websearch/search.rs).
websearch_msg.search.no_query_param = { $tool } nennt im inputSchema keinen Parameter für die Anfrage ({ $keys }).
websearch_msg.search.unknown_param = { $tool } verlangt den Parameter „{ $name }“, den Baruch nicht kennt.
websearch_msg.search.unreadable = { $tool }: Antwort nicht lesbar (weder Trefferliste noch Text).
websearch_msg.search.no_query = keine Suchanfrage (Frage und Abschnitt sind leer)
websearch_msg.search.sources_unreadable = Hinweis: bevorzugte Quellen nicht lesbar ({ $detail })
websearch_msg.search.timeout = Zeitlimit von { $seconds } s für die ganze Websuche erreicht; { $pages } Seite(n) bis dahin abgerufen
websearch_msg.search.missing_tool = MultiGPT bietet das Werkzeug { $tool } nicht an (tools/list).
websearch_msg.search.site_failed = Hinweis: Suche „site:{ $host }“ ausgefallen ({ $detail })
websearch_msg.search.general_failed = Hinweis: allgemeine Suche ausgefallen ({ $detail })
websearch_msg.search.not_fetched = Hinweis: { $url } nicht abgerufen ({ $detail })
websearch_msg.search.skipped = Hinweis: { $url } übersprungen ({ $detail })
