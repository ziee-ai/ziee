# TESTS — resource-link SSRF fix

No permission is introduced (no `X::use/read/manage`, no migration grant) → no `[negative-perm]`
e2e required. No UI workspace touched → no `tier: e2e` required. Backend-only: unit + integration.

Unit tests exploit that `validate_outbound_url` short-circuits **IP literals without DNS**, so real
RFC1918 / IMDS addresses can be asserted deterministically offline.

## Unit (in `resource_link.rs` `#[cfg(test)]`)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/resource_link.rs` — asserts: `host_of` lowercases + extracts host and ignores port/scheme-case — `host_of("http://172.21.0.1:9004/mcp") == host_of("HTTP://172.21.0.1:9005/x") == Some("172.21.0.1")`; `host_of("ziee:///abs")`/malformed → host per url crate (no panic).
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/resource_link.rs` — asserts: `choose_fetch_policy` precedence — trusted-host private link (host ∈ trusted_hosts, debug=false, env=false) → `PrivateScoped`; untrusted private + env=false → `Public`; untrusted private + env=true → `PrivateGlobal`; trusted-host + env=true → `PrivateGlobal` (env precedence); debug=true → `DevLocal` (highest precedence).
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/resource_link.rs` — asserts: the kind→(policy, follow_redirects) mapping — `PrivateScoped→(MCP_USER,false)`, `PrivateGlobal→(MCP_USER,true)`, `Public→(PUBLIC_HTTP_OR_HTTPS,true)`, `DevLocal→(DEV_LOCAL,true)`.
- **TEST-4** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/server/src/modules/mcp/resource_link.rs` — asserts: end-to-end policy behavior on IP literals — `validate_outbound_url("http://172.21.0.1:9005/x", &MCP_USER)` = Ok; `…, &PUBLIC_HTTP_OR_HTTPS)` = Err (private blocked); `validate_outbound_url("http://169.254.169.254/latest", &MCP_USER)` = Err (IMDS/link-local blocked even under the trusted policy).
- **TEST-5** (tier: unit) [covers: ITEM-2, ITEM-6] file: `src-app/server/src/modules/mcp/resource_link.rs` — asserts: `resource_link_allow_private_env()` reads `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` — returns true only for `"1"`, false when unset/other (guarded with a serialized env-set/restore to avoid cross-test races).

## Integration (in `tests/mcp/resource_link_test.rs`, loopback mock artifact server)

- **TEST-6** (tier: integration) [covers: ITEM-2, ITEM-3, ITEM-5] file: `src-app/server/tests/mcp/resource_link_test.rs` — asserts: matched host — `persist_links` with `trusted_hosts=[mock loopback host]` and an external (non-built-in) `http://127.0.0.1:<port>/artifact.csv` link → ingest succeeds, `outcome.saved` has 1 artifact, and the link's `file_id`/`version`/`version_id` are stamped back (proves the trusted-host allowance wires through and the display-fix precondition holds).
- **TEST-7** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/mcp/resource_link_test.rs` — asserts: unmatched host + env off — same loopback link with `trusted_hosts=[]` and `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` unset → `PUBLIC_HTTP_OR_HTTPS` rejects the loopback host → nothing saved (`outcome.saved` empty), link keeps its original uri (no file_id stamped).
- **TEST-8** (tier: integration) [covers: ITEM-2] file: `src-app/server/tests/mcp/resource_link_test.rs` — asserts: env opt-in — same loopback link with `trusted_hosts=[]` but `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1` → ingested (proves the release-honored global opt-in, host-match not required).
- **TEST-9** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/mcp/resource_link_test.rs` — asserts: off-host redirect on the scoped path — mock at trusted host returns 302 → a different host → with redirects disabled the fetch does not follow, nothing is saved (proves an off-host redirect cannot inherit the private allowance).
- **TEST-10** (tier: unit) [covers: ITEM-3, ITEM-4] file: `src-app/server/src/modules/mcp/resource_link.rs` — asserts: `trusted_hosts_from_urls` — the shared trusted-host derivation used by BOTH the chat call sites (from `accessible_servers`) and the workflow call site (from `list_accessible().servers`) — skips `None` (stdio) + hostless URLs, lowercases, and dedups same-host different-port entries. This is the "builds trusted_hosts" logic of ITEM-3/ITEM-4 (the `list_accessible` fetch itself is a one-line wiring exercised by the suite compiling).

## Doc / non-code items

- **ITEM-5** doc-comment + test-call updates are exercised by TEST-6..TEST-9 compiling and passing
  against the new signature (a mis-threaded arg fails to compile). Covered.
- **ITEM-6** (CLAUDE.md note) is documentation; covered indirectly by TEST-5/TEST-8 which prove the
  documented env-var behavior is real.

## ITEM → TEST coverage map
- ITEM-1 → TEST-1, TEST-2, TEST-3, TEST-4
- ITEM-2 → TEST-4, TEST-5, TEST-6, TEST-7, TEST-8, TEST-9
- ITEM-3 → TEST-6, TEST-7, TEST-9
- ITEM-4 → TEST-10
- ITEM-5 → TEST-6..TEST-9 (compile against the new signature)
- ITEM-6 → TEST-5, TEST-8 (prove the documented env behavior)
