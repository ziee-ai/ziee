# Chunk BG-2 — FIX round 1

Blind multi-angle audit (LEDGER.jsonl — 15 findings across 9 angles incl.
`equivalence` + two `security` findings covering the SSRF guard and the at-rest
crypto) reconciled against every diff hunk (AUDIT_COVERAGE.tsv — 14 hunks, ≥3
angles each). One finding surfaced DURING the drift-convergence loop and was
resolved + re-verified before the gate; none deferred:

- **build / edition mismatch** (`secret.rs` + `secrets.rs`): the moved code used
  edition-2024 let-chains (`if let … && let …`), which fail under the edition-2021
  SDK workspace (`error: let chains are only allowed in Rust 2024 or later`, 3
  sites). Fixed by desugaring each to the exactly-equivalent nested `if let`
  (TRANSFORMS D3) — same short-circuit order, same branches, same side effects.
  Re-verified: `cd sdk && cargo check --workspace` exit 0.

No other finding required a code change:
- **security (SSRF)** — `url_validator` moved byte-identically; all policy
  constants + IP-block arms + redirect/DNS re-validation intact.
- **security (crypto)** — `secret` crypto unchanged but for the AppError source +
  storage_key path repoints (both same underlying value/type).
- **build-db** — grep confirmed zero compile-time `query!` in `secret.rs`;
  framework stayed build-DB-free (workspace check needs no DB).
- **wire/types** — `SecretView` has no `JsonSchema` impl, so no schemars ident
  moved crates; E8 golden byte-identical (types.ts) + canonical (openapi.json) on
  BOTH surfaces.
- **feature-unification** — reqwest matched the server base decl; `cargo check -p
  ziee -p ziee-desktop` exit 0.

A second blind pass over the full `git diff` hunks (SDK submodule + ziee side, all
14 in AUDIT_COVERAGE) surfaced no additional equivalence/security/type/boundary
divergence.

**New confirmed findings: 0**
