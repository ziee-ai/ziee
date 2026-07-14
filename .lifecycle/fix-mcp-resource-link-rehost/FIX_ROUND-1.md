# FIX_ROUND-1

## Fixes applied (in response to the Phase-6 LEDGER)
1. **Shared helper extraction** (resolves error-handling#1, coverage-gaps chat#2, coverage-gaps
   workflow#3): extracted `resource_link::result_link_trusted_hosts(emitter_is_built_in, user_id)` —
   the built-in short-circuit + accessor query + error handling in ONE place, called by all 3
   `persist_links` sites. A lookup error now logs a `warn!` before degrading to the strict PUBLIC
   policy (was a silent `unwrap_or_default`).
2. **Helper test** (resolves the glue coverage gap): added assertions in
   `accessor_returns_system_host_and_omits_builtin` — external emitter → the registered system host;
   built-in emitter → empty set — exercising the branch logic shared by all 3 call sites.
3. **Unit-test comment corrected** (resolves test-reality#6): the pure-filter test no longer
   overclaims; the real is_system→is_built_in guard is documented as the accessor integration test.
4. **Loopback tradeoff documented** (security#2 / perms-authz#3): DEC-3 records the admin-system
   loopback same-host-trust tradeoff explicitly; the misleading "127.0.0.1 can't gain trust" call-site
   comments were corrected.

Accepted-not-fixed (recorded in LEDGER with rationale): perf#5 (per-result query — negligible,
external-only), and the security/perms loopback tradeoff (consistent with the pre-existing same-host
model; admin-only; IMDS blocked).

## Re-audit (round 2 — 3 fresh blind agents over the post-fix diff)
- **correctness+patterns+concurrency+perf:** clean (0 findings) — helper behavior-identical to the
  3 inline blocks modulo the intended fix + warn; all sites pass correct `(is_built_in, user_id)`;
  fail-closed; no borrow/await issue.
- **security+sql+api-contract+error-handling:** fail-closed confirmed; SQL parameterized + scoped;
  no URL leak; IMDS-safe. Re-raised the loopback tradeoff (**duplicate** of the round-1
  accepted-tradeoff — no new logic issue) and a **docstring-accuracy** nit → **FIXED** (added a
  SECURITY NOTE to `trusted_hosts_from_servers` stating only built-in loopback hosts are excluded).
- **tests+regression:** all guidance substrings verified against source; helper assertions
  non-tautological + revert-catching; ingest test gates on the accessor output. One **low** residual:
  the literal call-site arg wiring isn't exercised end-to-end → **ACCEPTED** (trivial 2-arg
  pass-through, pre-existing glue pattern, covered by the mandatory Phase-8 live container repro).

## Convergence
All actionable findings are fixed (helper+warn, comment, docstring) or explicitly dispositioned in
LEDGER.jsonl (accepted tradeoffs + the low glue-wiring residual covered by the Phase-8 live repro).
The round-2 re-audit surfaced no NEW confirmed logic defect — only the docstring nit (fixed) and
duplicates/residuals of already-recorded items.

**New confirmed findings:** 0
