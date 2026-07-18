# Chunk `sdk-standalone-fixes` — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 12 findings across 12 angles incl.
`security` + `equivalence` + `schema-fingerprint` + `standalone-apply` +
`full-crate-audit` + `wiring-correctness` + `cross-crate-resolution` +
`back-compat` + `migration-integrity` + `desktop-parity` + `build-hygiene` +
`boundary`) reconciled against every diff hunk (`AUDIT_COVERAGE.tsv` — 10 hunks,
≥2 angles each). FIX_ROUND count of deferred defects = 0.

Two items were handled DURING implementation (drift-convergence loop), not
deferred:

- **N-3 test — OpenAPI paths read from the wrong doc.** The first pass asserted
  `build_api_router`'s returned `api_doc.paths` (which carries only the
  `bearerAuth` security scheme, not routes) → `expect("openapi has paths")`
  panicked. Fixed by reading paths from the `OpenApi` that `finish_api` populates.
  Re-run: both N-3 tests green.

- **Gate false-positive on the benign DROP NOTICE.** `standalone_apply_gate.sh`
  first grepped `does not exist` unconditionally and flagged the
  `DROP DATABASE IF EXISTS` NOTICE ("database … does not exist, skipping") as a
  failure. Fixed by excluding `does not exist, skipping`. Re-run: all three
  crates PASS; negative-control (leak reintroduced) FAILs ziee-notification only.

No further fix rounds required — the audit angles all landed `verified` on the
first blind pass after these two convergence fixes.
