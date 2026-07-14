# Chunk B3 — FIX round 1

Blind multi-angle audit (LEDGER.jsonl, 15 findings across 12 angles incl.
`equivalence` + `security`) reconciled against every diff hunk (AUDIT_COVERAGE.tsv,
13 hunks, ≥3 angles each). Two findings had actionable status; both were fixed
DURING implementation (the drift-convergence loop) and re-verified, not deferred:

- **lint / unused-imports** (`ziee openapi.rs`): re-exporting the three unused
  `PermissionError*` types tripped `-D unused-imports`. Fixed with
  `#[allow(unused_imports)]` (surface preservation). Re-verified: `cargo check -p ziee`
  exit 0, zero new warnings.
- **lint / dead-code** (`ziee RequireAdmin` alias): moved `#[allow(dead_code)]` from
  the (now `pub`, lint-exempt) framework struct onto ziee's re-export alias.
  Re-verified: zero new warnings.

All other findings are `verified`/`info` — the enforcement algorithm, the admin
group-load elision, the `is_active` guard, the 403 schema, and the JWT-layer
untouched-ness were confirmed equivalent (E8 golden green on BOTH surfaces). No
security angle downgraded authorization: fail-closed on missing resolver, `is_admin`
bypass call-site-only, ALL-of AND preserved, no auth check dropped.

Re-audit of the post-fix diff surfaced no new issues.

**New confirmed findings:** 0
