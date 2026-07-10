# PLAN — resource-link SSRF blocks same-host MCP-server artifacts

## Context

When an external MCP server (RCPA/DSCC, separate containers on the same host) returns a tool
result carrying an artifact as a `resource_link` (e.g.
`http://172.21.0.1:9005/results/.../de_ad_control_limma.csv`), ziee's server-side ingest
(`persist_links` in `src-app/server/src/modules/mcp/resource_link.rs`) rejects the fetch by SSRF
policy and the chat shows "Failed to show file"; artifact chaining between MCP tools appears broken.

Root cause: the external-fetch branch chooses `PUBLIC_HTTP_OR_HTTPS` in release (the only opt-in is
`cfg!(debug_assertions)`-gated → dead code in the release container) → the RFC1918 docker-gateway
host `172.21.0.1` is rejected → ingest never runs → no `file_id` is stamped → the frontend falls
back to the raw non-`/api/` URL, which `useResourceLinkContent` refuses ("Failed to show file").

Fix (two complementary changes, both in the external-fetch branch): a scoped same-host trust that
permits the private fetch when the link host matches any enabled accessible MCP server's registered
host, plus a release-honored `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1` global opt-in. Both use the
existing `OutboundUrlPolicy::MCP_USER` (allows loopback/private, still blocks IMDS/link-local).
Making ingest succeed stamps `file_id` back onto the link → the file card renders via the
authenticated `/api/files/{id}` path; the LLM-facing raw URI is intentionally left unchanged (so
external→external container chaining keeps working).

## Items

- **ITEM-1**: Add pure, testable helpers to `resource_link.rs`: `host_of(url) -> Option<String>`
  (lowercased URL host), `trusted_hosts_from_urls(urls) -> Vec<String>` (the shared dedup/lowercase
  derivation used by all three call sites), and `choose_fetch_policy(link_uri, trusted_hosts,
  debug_loopback, env_private) -> FetchPolicyKind` where `FetchPolicyKind ∈ { Public, PrivateScoped,
  PrivateGlobal, DevLocal }`, plus a mapping `kind -> (OutboundUrlPolicy, follow_redirects: bool)`:
  `Public→(PUBLIC_HTTP_OR_HTTPS,true)`, `PrivateScoped→(MCP_USER,false)`,
  `PrivateGlobal→(MCP_USER,true)`, `DevLocal→(DEV_LOCAL,true)`. Precedence inside
  `choose_fetch_policy`: debug_loopback → env_private → host-match → public.
- **ITEM-2**: Add a non-`cfg`-gated env reader `resource_link_allow_private_env() -> bool` for
  `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1`, and rewrite the external-branch policy/client
  construction in `persist_links` (`resource_link.rs` ~L382-424) to use `choose_fetch_policy` +
  the redirect toggle: `follow_redirects=false` → build via
  `validated_client_builder(policy).redirect(reqwest::redirect::Policy::none()).build()`; else
  `build_validated_client(policy)`. Keep `validate_outbound_url` on the initial URL with the chosen
  policy. Preserve the existing debug `MCP_RESOURCE_LINK_ALLOW_LOOPBACK` seam.
- **ITEM-3**: Add a `trusted_hosts: &[String]` parameter to `persist_links` and thread it into the
  policy decision. Update the two chat call sites (`chat_extension/mcp.rs` approval + auto-exec) to
  build `trusted_hosts` via `trusted_hosts_from_servers(accessible_servers.iter().map(|s|
  (s.is_system, s.url.as_deref())))` — the helper EXCLUDES `is_system`/built-in servers so their
  loopback `url` never grants same-host trust (loopback-SSRF guard).
- **ITEM-4**: Update the workflow call site (`workflow/dispatch.rs`) to build `trusted_hosts` via the
  same `trusted_hosts_from_servers` helper — but ONLY for non-built-in emitters (skip the
  `Repos.mcp.list_accessible(ctx.user_id, 1, 1000, None, Some(true), None)` query entirely when the
  emitter is built-in, since built-in links never consult `trusted_hosts`).
- **ITEM-5**: Update the 7 existing `persist_links` call sites in
  `tests/mcp/resource_link_test.rs` for the new `trusted_hosts` parameter, and update the
  module/function doc comments in `resource_link.rs` to describe the trusted-host allowance, the
  env opt-in, and the redirect rule.
- **ITEM-6**: Document the operator-facing env var `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` in the
  resource_link section of `CLAUDE.md` (release-honored, off by default, still blocks IMDS).

## Files to touch

- `src-app/server/src/modules/mcp/resource_link.rs` — new helpers, `persist_links` signature +
  external-branch rewrite, doc updates, unit tests.
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — 2 call sites (build + pass trusted_hosts).
- `src-app/server/src/modules/workflow/dispatch.rs` — 1 call site (fetch accessible servers + pass).
- `src-app/server/tests/mcp/resource_link_test.rs` — update existing calls + add new integration tests.
- `CLAUDE.md` — env-var note.

## Patterns to follow

- **Trusted-private policy** — mirror `web_search/providers/searxng.rs` `SEARXNG_POLICY` (admin-
  trusted host allowed private) but REUSE the existing `OutboundUrlPolicy::MCP_USER` const rather
  than defining a new literal.
- **Redirect-disabled validated client** — mirror `llm_provider/handlers/discover.rs`
  (`.redirect(Policy::none())` layered onto a validated client builder).
- **Policy-decision helper shape + unit tests** — mirror `web_search/fetch.rs::fetch_policy` and
  `lit_search/connectors/mod.rs::connector_policy` (a small function returning a policy, unit-tested).
- **Integration harness** — mirror the existing `tests/mcp/resource_link_test.rs` (loopback mock
  file server via a spawned axum/hyper listener, `#[tokio::test]`, direct `ziee::persist_links` call).
