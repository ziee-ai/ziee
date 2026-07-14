# PLAN_AUDIT — plan audited against the codebase

Verified against the live tree in the worktree. The 3 production callers of
`trusted_hosts_from_servers` are confirmed at `chat_extension/mcp.rs:944`, `:2898`, and
`workflow/dispatch.rs:1309`. The redaction is confirmed at `repository.rs:639-643`
(`if server.is_system { server.url = None; }`). The org servers are confirmed seeded
`is_system=true, is_built_in=false` (deploy `seed.sql`).

## Breakage risk
- **ITEM-1** (flip filter to `is_built_in`): the tuple's first bool changes meaning from
  `is_system` to `is_built_in`. All 3 current callers pass `s.is_system`; if left unchanged they
  would pass `is_system` into a param now interpreted as `is_built_in` — a latent bug. MITIGATION:
  ITEM-3 swaps ALL 3 callers to the new accessor (which passes `is_built_in`) in the same change;
  the old direct-`trusted_hosts_from_servers(accessible_servers…)` call disappears from prod code.
  The helper stays `pub(crate)` and is still exercised by the new accessor + unit tests. No other
  caller exists (grep-confirmed).
- **ITEM-2** (new accessor): purely additive method; no existing signature changes. Uses the same
  accessibility predicate as `list_accessible_mcp_servers` — no behavior change to existing reads.
- **ITEM-3** (call-site swap): introduces an `.await` + `Result` where the derivation was
  synchronous. No borrow conflict — `Repos` global touches neither the mutably-borrowed
  `result.links` nor `accessible_servers`. `.unwrap_or_default()` preserves infallible behavior
  (a DB error → empty trust set → PUBLIC policy → same as today's failure mode, no panic).
- **ITEM-1** also folds the comment/NOTE correctness update (no code effect).
- **ITEM-4/ITEM-5** (guidance strings): pure string edits. The RISK is the substring-assertion
  tests — they must be updated in lockstep (enumerated in TESTS.md, kept non-shrinking).

## Pattern conformance
- ITEM-1 mirrors the existing `trusted_hosts_from_urls`/`trusted_hosts_from_servers` idiom (iterator
  over `(bool, Option<&str>)`, `host_of` extraction, sort+dedup). ✔
- ITEM-2 mirrors `McpRepository::list_accessible` + the free `sqlx::query!` fns; returns a plain
  `Vec<String>` (no new model type, matching how trust hosts are already plain strings). ✔
- ITEM-3 mirrors the existing workflow short-circuit `if is_built_in { Vec::new() } else { … }`
  (`dispatch.rs:1297`). ✔
- ITEM-5/6 mirror the existing guidance fns + their `#[cfg(test)]` substring tests. ✔
- Integration test mirrors existing `persist_links` tests + `start_fixed_response_mock` fixture and
  the `tests/mcp/` admin-API system-server + group-assign helpers. ✔

## Migration collisions
None. This branch adds **no migration** (highest existing is `…157`; unchanged). No DB schema
change — the new accessor only reads existing columns (`is_built_in`, `url`, `enabled`, plus the
join used by the existing accessibility predicate).

## OpenAPI regen
Not required. No request/response type, route, or OpenAPI-annotated schema changes. The new
accessor is internal (`Vec<String>`), never surfaced via a handler. No `openapi.json` /
`api-client/types.ts` regen for either UI workspace; the frontend gates do not apply.

## Per-item verdicts
- **ITEM-1** — verdict: PASS — mirrors existing trust-host helpers; latent-mismatch risk fully
  covered by ITEM-3 swapping all 3 callers in the same change; no new migration.
- **ITEM-2** — verdict: PASS — additive read-only accessor mirroring `list_accessible`; no schema
  change; hosts-only return closes the redaction gap without re-exposing URLs to clients.
- **ITEM-3** — verdict: PASS — mirrors the existing workflow short-circuit; no borrow conflict;
  `.unwrap_or_default()` keeps infallible behavior.
- **ITEM-4** — verdict: CONCERN — guidance string edits must update the 4 substring-assertion tests
  in `mcp.rs` in lockstep (enumerated in TESTS.md; A5 non-shrinkage respected). Not blocking.
- **ITEM-5** — verdict: CONCERN — must keep the `handlers.rs` description test's existing asserted
  substrings passing and its negative guard ("does NOT contain `already reachable exactly as
  given`") satisfied. Not blocking.
- **ITEM-6** — verdict: PASS — [DESCOPED] with an approved disposition in DECISIONS.md; the sandbox
  `--share-net` posture makes a hard egress filter out of scope and Part A removes the need.
