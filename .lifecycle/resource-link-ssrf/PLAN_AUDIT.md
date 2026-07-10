# PLAN_AUDIT — resource-link SSRF fix (audited against the codebase)

## Breakage risk

- `persist_links` gains a new `trusted_hosts: &[String]` parameter. This is a breaking signature
  change for ALL callers: 3 production (2 chat, 1 workflow) + 7 test call sites. All are enumerated
  in ITEM-3/4/5 and updated in lockstep — verified there are exactly 3 production callers
  (`chat_extension/mcp.rs:711`, `:2560`; `workflow/dispatch.rs:1256`) and 7 test callers
  (`tests/mcp/resource_link_test.rs` lines 82,167,206,248,286,374,461). No other caller exists
  (`persist_links` is re-exported as `ziee::persist_links` via `lib.rs`; grep confirms no further
  use). Adding a trailing parameter with an explicit value at each site cannot silently mis-bind.
- The external-fetch branch rewrite (`resource_link.rs` ~L382-424) only changes the `else`
  (`!server_is_built_in`) arm. The built-in arm (loopback plain client) and the `ziee://` / is_saved
  arms are untouched → no regression to the trusted in-process paths.
- Default behavior is preserved: with empty `trusted_hosts`, env unset, and release build,
  `choose_fetch_policy` returns `Public` → `PUBLIC_HTTP_OR_HTTPS` exactly as today. The existing
  debug `MCP_RESOURCE_LINK_ALLOW_LOOPBACK` seam is preserved (→ `DevLocal`).

## Pattern conformance

- Reusing `OutboundUrlPolicy::MCP_USER` (url_validator.rs:108, `allow_localhost + allow_private`,
  `allow_link_local:false`) is the exact category the fix needs (private allowed, IMDS/link-local
  blocked). Confirmed present; mirrors the `SEARXNG_POLICY` intent without a bespoke literal.
- Redirect-disabled client via `validated_client_builder(policy).redirect(reqwest::redirect::Policy::none()).build()`
  mirrors `llm_provider/handlers/discover.rs:250-252` verbatim (confirmed `validated_client_builder`
  is `pub`, url_validator.rs:270). `build_validated_client` (line 259) used for the redirect-on paths.
- `choose_fetch_policy` / `host_of` follow the `web_search/fetch.rs::fetch_policy` +
  `lit_search/connectors/mod.rs::connector_policy` small-pure-function shape, unit-tested in-source.
- `Repos.mcp.list_accessible(user_id, 1, 1000, None, Some(true), None)` (repository.rs:612) is the
  established enumeration; the chat path already reaches the same data via
  `helpers::get_all_accessible_config` (helpers.rs:22).

## Migration collisions

None. This feature adds NO migration (no schema change) — highest existing is
`00000000000132`; nothing new. No collision possible.

## OpenAPI regen

Not required. No REST route / request / response type changes. `persist_links` is internal.
`openapi.json` and `api-client/types.ts` are untouched → no UI workspace touched → backend-only gates.

## Known limitation (documented, not a blocker)

`McpRepository::list_accessible` redacts `url` on **is_system** servers (repository.rs:632-635), so a
same-host external MCP server registered as a *system* server (admin-level, group-assigned) will not
have its host in `trusted_hosts` built from the accessible list. The common deployment registers such
servers as **user-owned** (`rcpa-user`/`dscc-user`), whose `url` is NOT redacted → scoped trust works.
For the system-server edge, the release env opt-in `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1` is the
catch-all. Captured as DEC-5.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure helpers mirroring `web_search/fetch.rs::fetch_policy`; reuses the
  existing `MCP_USER`/`DEV_LOCAL`/`PUBLIC_HTTP_OR_HTTPS` consts; no external dependency.
- **ITEM-2** — verdict: PASS — external-branch rewrite is localized to the `!server_is_built_in` arm;
  redirect-disabled builder mirrors discover.rs; env reader is a plain non-cfg-gated `std::env::var`.
- **ITEM-3** — verdict: CONCERN — `accessible_servers` is url-redacted for is_system servers
  (repository.rs:632). Mitigated: real deployment uses user-owned servers (not redacted); env opt-in
  covers the system-server edge. Documented in DEC-5 + code comment + CLAUDE.md. Not BLOCKED.
- **ITEM-4** — verdict: PASS — reuses the already-present `Repos` handle; `list_accessible` returns
  `.servers: Vec<McpServer>` with `.url`; same redaction caveat as ITEM-3 (DEC-5).
- **ITEM-5** — verdict: PASS — mechanical: append `&[]`/fixture host slice to 7 existing test calls;
  doc-comment update is non-functional.
- **ITEM-6** — verdict: PASS — doc-only addition to CLAUDE.md's resource_link section.
