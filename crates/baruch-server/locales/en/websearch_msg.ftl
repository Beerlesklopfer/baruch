# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the web search of @belege (src/websearch/**): evidence page, adopting into the library, notes of a
# search run, refused addresses (Round 39a, task messages_2).

# Evidence page /websuche/belege.
websearch_msg.page.adopted = The evidence is in the author library: title, quotation and highlight.
websearch_msg.page.not_proven = Only proven quotations go into the library: this wording is not on the retrieved page (or there is no quotation). Nothing created.
websearch_msg.page.already = This evidence is already in the library.
websearch_msg.page.unknown = This evidence does not exist.
websearch_msg.page.unknown_action = Unknown action; nothing changed.
websearch_msg.page.failed = That did not work; nothing created.
websearch_msg.page.post_missing = post missing (post).
websearch_msg.page.error = Error: { $detail }
websearch_msg.page.template_error = Error: template: { $detail }

# websearch.* over /rpc and adopting (src/websearch/mod.rs, evidence.rs).
websearch_msg.rpc.number_expected = { $name }: number expected
websearch_msg.rpc.missing = { $name } missing
websearch_msg.rpc.on = on: true or false expected
websearch_msg.evidence.store = Storage: { $detail }
websearch_msg.evidence.unknown = Evidence { $id } does not exist.
websearch_msg.evidence.not_proven = Evidence { $id } is not proven; only quotations whose wording is on the retrieved page go into the library.
websearch_msg.evidence.no_quote_verdict = Evidence { $id } is without a quotation; only quotations whose wording is on the retrieved page go into the library.
websearch_msg.evidence.adopted = Evidence { $id } has already been adopted (title { $title }).
websearch_msg.evidence.no_quote = Evidence { $id } has no quotation.

# Retrieval (src/websearch/fetch.rs): reasons why an address is not retrieved.
websearch_msg.fetch.scheme = only http and https addresses are retrieved
websearch_msg.fetch.userinfo = address contains user information
websearch_msg.fetch.host = address without a host
websearch_msg.fetch.address = address in the own network (loopback, private or link-local)
websearch_msg.fetch.name = host name of the own network (localhost, .local, .internal, .home.arpa or without a dot)
websearch_msg.fetch.no_url_param = { $tool } names no parameter for the address in the inputSchema ({ $keys }).
websearch_msg.fetch.too_big = Page is larger than { $max } MiB ({ $size } KiB)
websearch_msg.fetch.no_text = { $url }: the page contains no text

# Notes of a search run (src/websearch/search.rs).
websearch_msg.search.no_query_param = { $tool } names no parameter for the query in the inputSchema ({ $keys }).
websearch_msg.search.unknown_param = { $tool } requires the parameter “{ $name }”, which Baruch does not know.
websearch_msg.search.unreadable = { $tool }: answer not readable (neither a hit list nor text).
websearch_msg.search.no_query = no search query (question and section are empty)
websearch_msg.search.sources_unreadable = Note: preferred sources not readable ({ $detail })
websearch_msg.search.timeout = Time limit of { $seconds } s for the whole web search reached; { $pages } page(s) retrieved until then
websearch_msg.search.missing_tool = MultiGPT does not offer the tool { $tool } (tools/list).
websearch_msg.search.site_failed = Note: search “site:{ $host }” failed ({ $detail })
websearch_msg.search.general_failed = Note: general search failed ({ $detail })
websearch_msg.search.not_fetched = Note: { $url } not retrieved ({ $detail })
websearch_msg.search.skipped = Note: { $url } skipped ({ $detail })
