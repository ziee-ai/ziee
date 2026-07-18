# Chunk `hardware` — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 12 findings across 11 angles incl.
`equivalence`, `feature-parity`, `visibility`, `shim-transparency`, `security`,
`cross-crate-resolution`, `boundary`, `test-fidelity`, `build-hygiene`,
`canonical-equality`, `desktop-parity`) reconciled against every diff hunk
(`AUDIT_COVERAGE.tsv` — 7 hunks, ≥3 angles each).

Three items were handled DURING implementation (the drift-convergence loop):

- **edition-compat** (`ziee-hardware`): the first crate scaffold was edition 2021;
  `detection.rs` uses Rust-2024 let-chains, so `cargo check` failed with `error:
  let chains are only allowed in Rust 2024 or later` (×12). Fixed by setting the
  crate to `edition = "2024"` (matching ziee), keeping `detection.rs` byte-for-byte
  rather than lowering the chains. Re-verified: sdk `--workspace` exit 0.

- **feature-parity** (`server/Cargo.toml` + `ziee-hardware`): the first pass left
  the 4 GPU deps + `gpu-detect = ["nvml-wrapper", …]` in the server, so
  `ziee-hardware` (which now HOLDS `detection.rs`) had no way to compile the
  gpu-detect branches → `unresolved import nvml_wrapper`. Fixed by moving the 4
  optional deps + the `gpu-detect` feature into `ziee-hardware` and rewriting the
  server's `gpu-detect = ["ziee-hardware/gpu-detect"]` (feature forward). Grep first
  confirmed no other server code used those crates. Re-verified: `cargo check -p
  ziee` (default features) exit 0; `cargo test -p ziee-hardware --features
  gpu-detect` → 11 passed.

- **visibility** (`monitoring.rs`): `collect_hardware_usage` was `pub(super)`;
  after the move ziee's `handlers.rs:176` couldn't reach it (`E0603`). Fixed by
  widening to `pub`. Re-verified: `cargo check -p ziee` exit 0.

- **test-tooling** (`ziee-hardware/Cargo.toml`): the moved `monitoring` tests use
  `#[serial_test::serial]`; first test build failed `unresolved import
  serial_test`. Fixed by adding the `serial_test = "3.2"` dev-dep (matching the
  server). Re-verified: `cargo test -p ziee-hardware --features gpu-detect` → 11
  passed / 1 ignored.

All other findings are `verified`: `detection.rs` byte-for-byte; `types.rs`/
`permissions.rs`/`monitoring.rs` differ only by the documented single edits each;
the security caps/allowlist are intact and covered by moved tests; the shim keeps
every call site + `main.rs` resolving; the crate names no app type + no DB; and
golden E8 is `types.ts` byte-identical + `openapi.json` canonically-equal on BOTH
ui + desktop.

Re-audit of the post-fix diff surfaced no new issues.

**New confirmed findings: 0**
