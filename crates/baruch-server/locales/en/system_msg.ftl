# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the system modules (guard, lib, rpc, about, shutdown, instance, models): refusals of the guards,
# error pages, model connections (Round 39a, task messages_2).

# Guards (src/guard.rs): refusals of foreign pages and foreign machines.
system_msg.guard.foreign_read = Refused: request from a foreign page (reading)
system_msg.guard.refused = Refused: { $detail }
system_msg.guard.remote = Refused: from other computers only /mcp with a token.
system_msg.guard.host = Refused: Baruch only answers under an IP address, localhost or a host name allowed at start (--allow-host, operation behind a reverse proxy), not under a foreign name (Host header, protection against DNS rebinding).
system_msg.guard.fetch_site = request from a foreign page (Sec-Fetch-Site)
system_msg.guard.origin_unreadable = Origin not readable
system_msg.guard.origin_no_host = Origin without Host
system_msg.guard.origin_mismatch = request from a foreign page (Origin does not match Host)

# Workspace page and error pages (src/lib.rs).
system_msg.page.error = Error: { $detail }
system_msg.page.unknown_work = unknown work
system_msg.page.delete_confirm = Please confirm the deletion.

# JSON-RPC answers with a user-facing text (src/rpc.rs, src/instance.rs).
system_msg.rpc.markdown_converted = Markdown headings converted.
system_msg.rpc.markdown_failed = Markdown headings not converted: { $detail }
system_msg.rpc.delete_confirm = Please confirm the deletion (confirm).
system_msg.rpc.page_title = { $title } – Baruch
system_msg.rpc.instance_name = name: text expected

# About Baruch (src/about.rs).
system_msg.about.appimage_place = usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt in the image (unpack with --appimage-extract)
system_msg.about.windows_place = THIRD-PARTY-LICENSES.txt in the installation folder (default C:\Program Files\Baruch)

# Quitting Baruch (src/shutdown.rs).
system_msg.shutdown.done = Baruch has quit – this window can be closed
system_msg.shutdown.template_error = Error: template { $name }: { $detail }

# Model connections (src/models.rs).
system_msg.models.not_local = The address is not recognisably on this computer or in the own network, but the connection is kept as local: book text goes there without a release per persona. If the service lies outside, tick “external”.
system_msg.models.eliezer_unreadable = Connection eliezer not readable: { $detail }
system_msg.models.unknown_kind = Unknown kind “{ $kind }”.
system_msg.models.no_address = No address entered for { $name } (Settings → Connections).
system_msg.models.keyring_unreadable = Keyring not readable ({ $detail }); nothing sent.
system_msg.models.key_other_target = The key of the connection { $name } applies to another target; not sent. Enter the key anew for the present address (Settings → Connections).
system_msg.models.persona_connection = Connection of the persona not readable: { $detail }
system_msg.models.connection_unreadable = Model connection not readable: { $detail }
system_msg.models.not_released = The connection “{ $name }” is external; it is not released for { $who } (Settings → Assistants). Nothing was sent.
system_msg.models.no_persona = this task without a persona
system_msg.models.stale_list = { $name } does not answer ({ $detail }); list from the expired cache (as of { $stamp }).
system_msg.models.context_lower = Setting { $setting }, { $name } reports { $reported }; { $reported } tokens are in effect.
system_msg.models.context_higher = Setting { $setting }, { $name } reports { $reported }; the setting stays in effect.
system_msg.models.unknown_at_eliezer = Unknown at Eliezer: { $list }.
system_msg.models.alias = Alias: { $list } (Baruch resolves the old name when sending).
system_msg.models.number_expected = { $name }: number expected
system_msg.models.id_missing = { $name } missing (id of the model connection)
system_msg.models.kind = kind: eliezer, openai or ollama.
system_msg.models.url_missing = url missing.
system_msg.models.external = external: true or false.
system_msg.models.secret_empty = secret: empty; nothing changed.
system_msg.models.secret_long = secret: at most 4096 characters on one line.
system_msg.models.keyring = Keyring: { $detail }
system_msg.models.confirm = confirm: true expected
system_msg.models.params_object = Parameters expected as an object
system_msg.models.model_name = model: model name (1–200 characters, one line) expected.
system_msg.models.persona_missing = persona missing (e.g. “@lektor”)
