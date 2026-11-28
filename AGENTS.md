# Baruch – rules for agents

Free writing workshop (AGPL-3.0) for books and sermons. Plan: `docs/Plan.md`.

- Language: the primary language is English for code, comments, commits, developer docs, MCP and RPC. The user
  interface is German and English through catalogs from Round 39 (until then German). Each work has its own
  work language (`de`|`en`); book output and persona answers follow it. Identifiers are English.
  Exceptions that stay German: the history files (`docs/Backlog.md`, `docs/Fahrplan.md`, `HANDOFF.md`,
  `CHANGELOG.md`), `docs/schema/`, the user help under `site/content/de/` (English from Round 39b) and every text of
  the user interface until the catalogs of Round 39a.
- Never commit, publish or print tokens or keys; keys belong in the keyring (`keyring`).
- Only the user sets version numbers.
- Push only after an explicit "ja" (yes) from the user.
- Power (2026-11-24, two failures of the power supply under full load): always bind building and testing to the same
  eight threads and limit test parallelism, e.g.
  `RUST_TEST_THREADS=4 nice -n 10 ionice -c3 taskset -c 0-7 cargo test --workspace -j 4`. `-j 4` only limits
  building; the tests would otherwise run on all 32 threads. Because every run is bound to the same cores `0-7`,
  the total load stays capped even with parallel agents. Never run several full test runs at the same time.
- Machine load: always build and test with `nice`/`ionice` and `-j 4`. Agents may work in parallel
  (each in its own worktree, branch `team/<id>`), but each builds into its **own** target/ directory (`target/`
  inside the worktree) – no shared `CARGO_TARGET_DIR`: the workspace crates get the same artifact hashes in all
  worktrees, cargo then takes the build of another worktree for current and tests foreign code
  (proven in round B1.5). Never run Firefox in parallel with test runs.
- Branches (development/main): work and commits happen on `development`; the fast-forward to `main` triggers the CI
  and is done only by the user.
- Do not guess: evidence (output, code, docs) before claims.
- Stubs: planned tools (MCP, RPC) exist immediately as a stub. A stub answers honestly ("not implemented yet", with
  a note on what is missing) and carries the mark `// STUB: <what is missing> (<stage>)` in the code; the
  description of the tool names it as well. Open stubs: `grep -rn "STUB:" crates`.
- Roadmap: the PM maintains `docs/Fahrplan.md` (Mermaid Gantt and table) at the end of every round – enter the
  finished round with start/end from `git log` and its commits, re-estimate running/planned rounds from
  `docs/Backlog.md`, check the Gantt for valid syntax before the commit (no colons in task names).
- Docs grow with the code (under `docs/`): developer entry point for usage (`docs/Bedienung.md`), structure
  (`docs/Architektur.md`), interfaces (`docs/RPC.md`, `docs/MCP.md` with every tool, stubs marked).
  Every change to the user interface, RPC or MCP updates the docs in the same commit.
- Help: user help lives in `site/content/de/hilfe/`, operation in `site/content/de/betrieb.md`; `docs/Bedienung.md`
  and `docs/Betrieb.md` are developer entry points.
- Revisions: each section has exactly one current revision (a mark that can be switched at any time); only the
  author changes or deletes, logged in the mirror; assistants and editors (Lektoren) only make suggestions. Without
  accounts, the human at their own machine is the author.
  Exception outline (2026-11-06): assistants may also create outline items (MCP `outline_add`, origin in the
  mirror); renaming, moving, changing the level, number on/off and deleting only after the human has confirmed a
  request.
  Addition (2026-11-07): a new part under which existing chapters would move is not mere creation but a request
  that the human actively approves (popup; without JavaScript a notice bar and a form); only a part without
  following chapters stays creation.
- Accounts and roles (2026-11-26, Round 35): without accounts Baruch stays open as before. With accounts, pages,
  forms and `/rpc` require a login; `/mcp` stays with tokens. Editors (`@lektorat:<name>`) see only works shared
  with them and write only suggestions, posts and findings. For editors every route and every RPC method is
  locked until it is allowed in `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`) – the only place of the permission check.
- Editor persona by setting (Einstellungen → Assistenten, i.e. Settings → Assistants): „fest <Persona>“ (fixed
  persona) or „automatisch nach Kontext“ (automatic by context: the active persona with the largest context; an
  inactive fixed persona falls back to this rule). Trial since 2026-11-06: fixed @lektor (all personas on one model).
- User interface: `baruch-server` (axum + minijinja) serves pages and fragments, the core is reachable through
  JSON-RPC (`POST /rpc`); widgets in TypeScript (`crates/baruch-server/web/src`, `data-widget`), compiled by
  `build.rs`. Everything must stay usable without JavaScript. No Iced.
