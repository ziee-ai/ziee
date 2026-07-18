# Chunk B6 — DRIFT scan (round 1)

Drift = any place moving the generator + driver tail could diverge from
pre-extraction output/behavior/surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. **Generator output identity.** The 1237 lines of
  production code moved byte-for-byte (only the test block changed). Proof is
  end-to-end, not by inspection: regenerated BOTH surfaces with the RELOCATED
  generator → `ui/src/api-client/types.ts` **byte-identical** (git status clean)
  AND `desktop/ui/src/api-client/types.ts` **byte-identical**. The generator was
  byte-preserved — the chunk's core requirement. Restored via `git checkout`.

- **DRIFT-1.2** — verdict: none. **openapi.json canonical equality (E8 REFINEMENT).**
  `jq -S` of old vs new is equal on BOTH surfaces (ui: 862 order-only lines;
  desktop: ~1162 order-only lines). Top-level key order is a deterministic fn of
  the linkme dep-graph (E8 REFINEMENT); the moved TAIL touches no route
  registration, so only key-order churn appears. Restored via `git checkout`.

- **DRIFT-1.3** — verdict: none. **Output paths.** `finish_and_emit` writes
  `<output_dir>/openapi.json` and the caller-supplied `types_ts_path`. ziee passes
  `output_dir/../src/api-client/types.ts` and desktop the same relative to
  `desktop/ui/openapi` — the identical locations the inlined code wrote (the
  desktop regen log confirms `desktop/ui/openapi/../src/api-client/types.ts`). The
  formerly-hardcoded derivation now lives at each call site; the emitted files land
  in the same place.

- **DRIFT-1.4** — verdict: none. **Driver split.** Only the app-agnostic tail
  moved; the app-specific head (Config, globals, module create+init) stays in
  ziee's `generate_openapi_spec`. Desktop still assembles the combined router
  (merge desktop routes) BEFORE calling the tail. `main.rs` + `lib.rs:664` call
  `openapi::generate_openapi_spec` unchanged; `end_to_end` driver test retained.

- **DRIFT-1.5** — verdict: none. **Golden-test split.** Generator-correctness →
  SDK `generator_golden_fixture` (small committed fixture, byte-identical, produced
  by the current generator so it's a real drift catcher). Per-app regen-drift →
  ziee's retained `types_ts_parity` + `types_ts_parity_desktop` (now call the
  framework generator against ziee's committed artifacts). SDK never depends on
  ziee's committed files. SDK: `cargo test -p ziee-framework openapi::` → 4 passed
  (3 emit_ts + 1 pre-existing permissions::openapi).

- **DRIFT-1.6** — verdict: none. **Re-export surface.**
  `ziee::generate_types_ts_from_json` + `ziee::finish_and_emit` (both consumed by
  the desktop binary) re-export directly from `ziee_framework::openapi::*` at the
  ziee crate root, so desktop resolves them unchanged. `openapi::generate_openapi_spec`
  is app-side, unchanged. Zero call-site edits outside the 3 touched app files.

- **DRIFT-1.7** — verdict: none. **Build hygiene.** A first-pass transitive
  `pub use` re-export through ziee's private `mod openapi` tripped
  `-D unused-imports` on the `emit_ts` module re-export; fixed by direct crate-root
  re-exports + full-path internal references (FIX_ROUND-1). `cargo check -p ziee`,
  `-p ziee-desktop`, and `cd sdk && cargo check --workspace` all exit 0. The 3
  pre-existing `ziee (lib)` warnings (mcp `list_enabled_for_health_check`, a voice
  `is_active`) are unrelated dead-code, present before this chunk.

- **DRIFT-1.8** — verdict: none. **Framework deps / boundary.** Added
  `indexmap`(serde) + promoted `serde_json` to a runtime dep, versions matching the
  ziee server catalog (single `Cargo.lock` unifies, no duplicate build). The
  openapi module names no domain type; `skeleton-server` still checks. No build DB
  introduced (the generator is a pure fn; the framework stays build-DB-free).

**Unresolved drifts:** 0
