# Chunk `sdk-standalone-fixes` — DRIFT scan (round 1)

Drift = any place these three fixes could diverge from pre-change ziee
output/behavior/surface. Each candidate reconciled; DRIFT count = 0.

- **DRIFT-1.1 — merged migration set.** verdict: none. Reconstructed the
  pre-change merged set (fkeys in the SDK crate, old blind `sdk/crates` glob) and
  `diff -rq` vs the current `migrations-merged` is EMPTY (92 files each). The N-1
  relocation + N-2 explicit-dir list produce the byte-identical merged set.

- **DRIFT-1.2 — EA-schema fingerprint.** verdict: none (w.r.t. my changes). The
  merged set is byte-identical (1.1), so the final schema is identical; the 4
  notification FK constraints appear byte-identical in baseline and reproduced
  fingerprints. The ONLY difference vs the committed baseline/schema.fp is the
  pre-existing `file_workflow_runs` side-table (workflow migration 202607144231,
  already in the branch base) — I touched no file-/workflow-domain code
  (`git diff --name-only` confirms). Not my drift; the baseline is simply stale
  relative to an earlier already-merged chunk.

- **DRIFT-1.3 — golden OpenAPI/types.** verdict: none. Regenerated BOTH surfaces
  against the migrated build DB: types.ui.ts + types.desktop.ts byte-identical,
  openapi.{ui,desktop}.json canonically equal. N-3's new code is behind the
  default-OFF `module` feature ziee does not enable, so ziee links no new route
  or schemars type.

- **DRIFT-1.4 — back-compat of the old compose fn.** verdict: none. The glob
  signature is retained and delegates with an empty explicit list; `cargo check
  --workspace` (default features) on the SDK is exit 0, so skeleton/examples
  callers are unaffected.

- **DRIFT-1.5 — AuthModule order vs coverage.** verdict: none, by design. The
  whole-app resolver/JWT layers only cover routes present when `Router::layer`
  runs; `order = i32::MAX` guarantees AuthModule registers LAST, so coverage is
  every module's routes. The focused test asserts a NON-auth gated route resolves
  (401, not 500). Documented in module.rs so a future high-order module doesn't
  silently steal the last slot.
