# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the system modules (guard, lib, rpc, about, shutdown, instance, models): refusals of the guards,
# error pages, model connections (Round 39a, task messages_2).

# Guards (src/guard.rs): refusals of foreign pages and foreign machines.
system_msg.guard.foreign_read = Abgelehnt: Anfrage von einer fremden Seite (lesend)
system_msg.guard.refused = Abgelehnt: { $detail }
system_msg.guard.remote = Abgelehnt: Von anderen Rechnern nur /mcp mit Token.
system_msg.guard.host = Abgelehnt: Baruch antwortet nur unter einer IP-Adresse, localhost oder einem beim Start erlaubten Rechnernamen (--allow-host, Betrieb hinter einem Reverse-Proxy), nicht unter einem fremden Namen (Host-Kopfzeile, Schutz vor DNS-Rebinding).
system_msg.guard.fetch_site = Anfrage von einer fremden Seite (Sec-Fetch-Site)
system_msg.guard.origin_unreadable = Origin nicht lesbar
system_msg.guard.origin_no_host = Origin ohne Host
system_msg.guard.origin_mismatch = Anfrage von einer fremden Seite (Origin passt nicht zu Host)

# Workspace page and error pages (src/lib.rs).
system_msg.page.error = Fehler: { $detail }
system_msg.page.unknown_work = unbekanntes Werk
system_msg.page.delete_confirm = Löschen bitte bestätigen.

# JSON-RPC answers with a user-facing text (src/rpc.rs, src/instance.rs).
system_msg.rpc.markdown_converted = Markdown-Überschriften umgewandelt.
system_msg.rpc.markdown_failed = Markdown-Überschriften nicht umgewandelt: { $detail }
system_msg.rpc.delete_confirm = Löschen bitte bestätigen (confirm).
system_msg.rpc.page_title = { $title } – Baruch
system_msg.rpc.instance_name = name: Text erwartet

# About Baruch (src/about.rs).
system_msg.about.appimage_place = usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt im Abbild (entpacken mit --appimage-extract)
system_msg.about.windows_place = THIRD-PARTY-LICENSES.txt im Installationsordner (Vorgabe C:\Program Files\Baruch)

# Quitting Baruch (src/shutdown.rs).
system_msg.shutdown.done = Baruch ist beendet – dieses Fenster kann geschlossen werden
system_msg.shutdown.template_error = Fehler: Vorlage { $name }: { $detail }

# Model connections (src/models.rs).
system_msg.models.not_local = Die Adresse liegt nicht erkennbar auf diesem Rechner oder im eigenen Netz, die Verbindung ist aber als lokal geführt: Buchtext geht ohne Freigabe je Persona dorthin. Liegt der Dienst außerhalb, „extern“ ankreuzen.
system_msg.models.eliezer_unreadable = Verbindung eliezer nicht lesbar: { $detail }
system_msg.models.unknown_kind = Unbekannte Art „{ $kind }“.
system_msg.models.no_address = Keine Adresse für { $name } eingetragen (Einstellungen → Verbindungen).
system_msg.models.keyring_unreadable = Schlüsselbund nicht lesbar ({ $detail }); nichts gesendet.
system_msg.models.key_other_target = Schlüssel der Verbindung { $name } gilt für ein anderes Ziel; nicht gesendet. Für die jetzige Adresse den Schlüssel neu eintragen (Einstellungen → Verbindungen).
system_msg.models.persona_connection = Verbindung der Persona nicht lesbar: { $detail }
system_msg.models.connection_unreadable = Modell-Verbindung nicht lesbar: { $detail }
system_msg.models.not_released = Die Verbindung „{ $name }“ ist extern; für { $who } ist sie nicht freigegeben (Einstellungen → Assistenten). Es wurde nichts gesendet.
system_msg.models.no_persona = diesen Auftrag ohne Persona
system_msg.models.stale_list = { $name } antwortet nicht ({ $detail }); Liste aus dem abgelaufenen Zwischenspeicher (Stand { $stamp }).
system_msg.models.context_lower = Einstellung { $setting }, { $name } meldet { $reported }; wirksam sind { $reported } Tokens.
system_msg.models.context_higher = Einstellung { $setting }, { $name } meldet { $reported }; wirksam bleibt die Einstellung.
system_msg.models.unknown_at_eliezer = Unbekannt bei Eliezer: { $list }.
system_msg.models.alias = Alias: { $list } (Baruch löst den alten Namen beim Senden auf).
system_msg.models.number_expected = { $name }: Zahl erwartet
system_msg.models.id_missing = { $name } fehlt (id der Modell-Verbindung)
system_msg.models.kind = kind: eliezer, openai oder ollama.
system_msg.models.url_missing = url fehlt.
system_msg.models.external = external: true oder false.
system_msg.models.secret_empty = secret: leer; nichts geändert.
system_msg.models.secret_long = secret: höchstens 4096 Zeichen in einer Zeile.
system_msg.models.keyring = Schlüsselbund: { $detail }
system_msg.models.confirm = confirm: true erwartet
system_msg.models.params_object = Parameter als Objekt erwartet
system_msg.models.model_name = model: Modellname (1–200 Zeichen, eine Zeile) erwartet.
system_msg.models.persona_missing = persona fehlt (z. B. „@lektor“)
