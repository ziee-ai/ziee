# DRIFT-1 — implementation vs plan

Reconciled the implemented diff against PLAN.md item-by-item. The lib + integration test binary
both compile clean (`cargo test --test integration_tests --no-run` → Finished, no warnings on the
changed files).

- **DRIFT-1.1** — verdict: none — ITEM-1 implemented exactly: `trusted_hosts_from_servers` filters on
  `is_built_in`; doc + in-`persist_links` NOTE rewritten. No divergence.
- **DRIFT-1.2** — verdict: none — ITEM-2 implemented as a lean `SELECT DISTINCT s.is_built_in, s.url`
  accessor with the same accessibility predicate as `list_accessible_mcp_servers` + `enabled=true`;
  returns hosts only. Matches plan (no `assemble_mcp_server` / secret decryption).
- **DRIFT-1.3** — verdict: none — ITEM-3 implemented at all 3 sites, gated `if server.is_built_in {
  Vec::new() } else { … }` (chat) / preserved `if is_built_in` short-circuit (workflow); stale
  comments replaced. Both chat sites are byte-identical (applied via one replace_all).
- **DRIFT-1.4** — verdict: none — ITEM-4/ITEM-5 guidance strings broadened for the tool-returned-URL
  case; existing asserted substrings preserved; the 4 `mcp.rs` tests + the `handlers.rs` description
  test extended in lockstep (A5 non-shrinkage respected — no TEST-ID removed).
- **DRIFT-1.5** — verdict: none — ITEM-6 remains DESCOPED (approved); no sandbox egress code added,
  as planned.
- **DRIFT-1.6** — verdict: resolved — during Phase 3 the standalone comment-only "ITEM-4 (NOTE
  update)" was folded into ITEM-1 and the guidance/sandbox items renumbered (ITEM-4/5/6). This was a
  plan amendment recorded then (gates re-run green), not a Phase-5 implementation drift; the shipped
  code matches the amended plan.

**Unresolved drifts:** 0
