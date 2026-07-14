# Chunk `ziee-test-harness` — BOUNDARY

- E1 (CUT present, ≥1 move: line, Design-gate): PASS
- E2 (TRANSFORMS: every differing symbol has a T-N; Decision Resolution; no TBD): PASS
- E3 (LEDGER valid, ≥8 angles, includes equivalence + security): PASS (13 entries, 12 distinct angles incl. equivalence + security)
- E4 (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS (12 rows)
- E5 (move-completeness: every move: dest exists in SDK; every Symbol resolves): PASS — all NEW/moved symbols resolve in `sdk/crates/ziee-test-harness/src/lib.rs`
- E6 (source-deletion: moved generic engine absent from ziee as a divergent duplicate): PASS — the generic engine symbols exist ONLY in the SDK; ziee's `harness_inner.rs` is a thin shim that references them (retained same-file surface is the equivalence mechanism, as ziee-file retained routes.rs)
- E7 (transform-declared: every differing moved symbol has a T-N): PASS (T-1..T-8 + 2 Decisions)
- E8 (regen-parity / golden): PASS — dev/test-only move; NO server route / schemars / aide change. The 4 generated files (openapi.{ui,desktop}.json + types.{ui,desktop}.ts) are UNTOUCHED (git status clean for them) → trivially BYTE-IDENTICAL, no regen needed.
- E9 (clean-build): PASS — `cargo check -p ziee` = 0 (via `cargo test --test integration_tests --no-run` linking the shim), `cargo test -p ziee-desktop --test integration_tests --no-run` = 0, `cd sdk && cargo check --workspace` = 0.
- E10 (no divergent duplicate / dead code): PASS — no `ziee::` and no `CARGO_MANIFEST_DIR` in the SDK crate (both `git grep` = 0); the engine exists once.
- E11 (seam-purity / SDK names only the seam): PASS — `git grep 'ziee::' sdk/crates/ziee-test-harness` = 0.
- E12 (submodule-pin): sdk submodule committed locally (no push); ziee records the new pointer (staged).

- ziee-suite: PASS (representative end-to-end subset through the extracted harness) —
  `cargo test --test integration_tests auth::admin_providers hub::migration -- --test-threads=1`
  (see the "Equivalence run" block below for the exact result). The harness spawns the
  ziee bin from `src-app/target/debug/ziee`, symlinked to the private target's build.
- golden(openapi): IDENTICAL (untouched)
- golden(types): IDENTICAL (untouched)
- golden(schema): IDENTICAL (no migration touched)

## Equivalence run

- **Server variant (Variant::Server) — LIVE, GREEN.** `cargo test --test
  integration_tests -- --test-threads=1 auth::admin_providers hub::migration`:
  the extracted harness spawned the real `ziee` bin (server started on
  127.0.0.1:20568), built `ziee_test_template_<key>`, cloned per-test DBs, and
  ran the suite: **`test result: ok. 15 passed; 0 failed; 0 ignored; 2205
  filtered out; finished in 24.18s`**. Exit 0. Proves the spawn + isolated-DB +
  config-render + health-poll + Drop engine works end-to-end through the seam.
- **Desktop variant (Variant::Desktop) — LIVE, GREEN.** Built the `ziee-desktop`
  bin, symlinked it into `src-app/target/debug/`, and ran
  `cargo test -p ziee-desktop --test integration_tests -- --test-threads=1
  remote_access::password_login`: the desktop test binary (which `#[path]`-
  reincludes the shim, seeds `Variant::Desktop`) built `ziee_test_template_desktop_<key>`
  with the server-merged + desktop migration dirs and spawned `ziee-desktop
  --headless`. Result: see the "Desktop run" line below (nails Risk-3 — the
  desktop template carries the extra migrations, no `relation … does not exist`).

Desktop run: `ziee-desktop` bin built (exit 0), spawned `ziee-desktop --headless`
on 127.0.0.1:20311 (`DesktopRepositoryFactory initialized`, `backfill_system_mcp_assignments:
ensured 6 (server, group) assignments` — desktop-only schema present), **`test result:
ok. 3 passed; 0 failed; 0 ignored; 49 filtered out; finished in 4.78s`**. Exit 0.

Seam-purity invariants (deterministic): `git grep 'ziee::' sdk/crates/ziee-test-harness`
= **0**; `git grep 'CARGO_MANIFEST_DIR' sdk/crates/ziee-test-harness` = **0**;
generated golden files touched = **NONE**.

## Gate commands (reproducible)
```
export CARGO_TARGET_DIR=/data/pbya/ziee/tmp/sdk-testharness-target
export DATABASE_URL="postgresql://postgres:password@127.0.0.1:54321/postgres"
(cd sdk && cargo check --workspace)                              # = 0
cargo test -p ziee --test integration_tests --no-run            # = 0 (ziee lib+shim link)
cargo test -p ziee-desktop --test integration_tests --no-run    # = 0 (desktop #[path] shim)
# equivalence: spawn ziee bin via the harness
ln -sf $CARGO_TARGET_DIR/debug/ziee src-app/target/debug/ziee
cargo test --test integration_tests auth::admin_providers hub::migration -- --test-threads=1
```
