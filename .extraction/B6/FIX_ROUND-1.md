# Chunk B6 — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 13 findings across 11 angles incl.
`equivalence`, `purity`, `byte-identity`, `canonical-equality`,
`output-path-parameterization`, `driver-split`, `golden-test-split`,
`test-fidelity`, `api-surface`, `cross-crate-resolution`, `build-hygiene`,
`desktop-parity`, `boundary`) reconciled against every diff hunk
(`AUDIT_COVERAGE.tsv` — 7 hunks, ≥3 angles each). One build-adjacent item was
handled DURING implementation (the drift-convergence loop), not deferred:

- **build / unused-imports** (`ziee openapi/mod.rs` + `lib.rs`): the first pass
  re-exported the generator + tail transitively —
  `pub use ziee_framework::openapi::{emit_ts, finish_and_emit}` inside ziee's
  private `mod openapi`, re-exported again by `lib.rs`. `-D unused-imports` flagged
  the `emit_ts` module re-export (the transitive-`pub use` lint case). Fixed by
  re-exporting BOTH symbols **directly** at the ziee crate root from
  `ziee_framework::openapi::*` and referencing the generator by its full path in
  the moved test. Re-verified: `cargo check -p ziee` + `-p ziee-desktop` +
  `cd sdk && cargo check --workspace` all exit 0, zero NEW warnings.
- **desktop dead code**: routing desktop through `ziee::finish_and_emit` made
  `use std::fs` dead and the `mut` on `api_doc` unnecessary; both removed.

All other findings are `verified`: the generator moved byte-for-byte (proved by
the byte-identical regen of BOTH `types.ts` surfaces — the chunk's core
requirement); `openapi.json` canonically-equal on both (E8 REFINEMENT); output
paths parameterized (files land in the same locations); the app-specific driver
head stayed app-side; the golden test split into an SDK generator-correctness
fixture test + ziee's retained per-app regen-drift guards; the re-export surface
keeps every caller (incl. the desktop binary) unchanged; the framework openapi
module names no domain type and introduces no build DB.

Re-audit of the post-fix diff surfaced no new issues.

**New confirmed findings:** 0
