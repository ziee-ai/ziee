# Chunk B5 — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 15 findings across 9 angles incl.
`equivalence`, `schema-neutrality`, `permission-routing`, `session-fanout`,
`self-echo`, `caps-pruning`, `generics-bounds`, `build-hygiene`,
`singleton-lifecycle`, `test-fidelity`, `eventbus-split`, `security`) reconciled
against every diff hunk (`AUDIT_COVERAGE.tsv` — 10 hunks, ≥3 angles each). No
finding had actionable (open) status; the build-adjacent items were handled
DURING implementation (the drift-convergence loop), not deferred:

- **generics-bounds / E0599** (`ziee-framework::sync::registry`): the first-pass
  `impl<P: Principal> Default` couldn't call `Self::new()` (defined on
  `impl<P: Principal + Send + 'static>`). Aligned the `Default` bound to match.
  Re-verified: `cargo check -p ziee-framework` exit 0.

- **build / unused-imports** (three sites, T-6/T-7/T-8): moving `Audience`'s
  constructors out of ziee made `PermissionList` dead on three paths. Removed the
  event.rs import (with the moved code), dropped the mod-level `permissions::`
  re-export, and `#[allow(unused_imports)]`-guarded the bin-tree `types.rs`
  re-export (still live for `crate::PermissionList` + a `#[cfg(test)]` call site).
  Re-verified: `cargo check -p ziee` (lib+bin) + `-p ziee-desktop` + `cd sdk &&
  cargo check --workspace` all exit 0, zero new warnings.

All other findings are `verified`/`info`: the moved machinery is byte-equivalent
modulo the genericization transforms (T-1..T-4); the wire/schema types stayed
concrete in ziee (E8 golden IDENTICAL on BOTH surfaces — `types.ts` byte-identical,
`openapi.json` canonically-equal — then restored); the singleton + concrete
principal are app-side (B4/B3 seam); every emit site + the `SyncEntity` enum are
unchanged; the 16 routing/constructor tests moved and pass; EventBus stays
app-side by decision.

Re-audit of the post-fix diff surfaced no new issues.

**New confirmed findings:** 0
