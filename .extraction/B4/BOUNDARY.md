# Chunk B4 — BOUNDARY (green evidence)

Honest self-reported evidence. **E8 PASSES per the E8 REFINEMENT** (types.ts
byte-identical + openapi.json canonically-equal) on BOTH surfaces: `Repos` is a
process-internal repository accessor, never serialized or routed, so moving the
generator macro cannot touch the client contract.

- E1: PASS — exactly one `.extraction/B4/` dir.
- E2: PASS — pending changes are exactly the B4 cut: SDK side (ziee-framework new `src/repository.rs` + `lib.rs` +5 + Cargo.lock if any) committed in the submodule; ziee side (`core/repository.rs`: macro def + 3 use lines removed, `use ziee_framework::declare_repositories;` + comment added, invocation+list retained) left UNCOMMITTED per task. No unrelated modifications (pgvector submodule untouched; regenerated openapi/types restored via `git checkout`).
- E3: PASS — no diff-added `#[ignore]`/`.skip`/`.only`/`xit` on any test.
- E4: PASS — no cosmetic or edited assertion; no test added, moved, or changed (the macro had no unit tests pre-move).
- E5: PASS — the `CUT.md` move dest + symbol resolves: `declare_repositories!` under `sdk/crates/ziee-framework/src/repository.rs`, `#[macro_export]` → callable as `ziee_framework::declare_repositories!`; `pub mod repository;` in the framework `lib.rs`.
- E6: PASS — the macro DEFINITION is deleted from ziee (`core/repository.rs` retained as the invocation-only shim); no divergent duplicate macro exists. The repo LIST + the emitted `Repos` global are deliberately NOT moved (single-source: generator in framework, generated singleton + concrete type list in ziee).
- E7: PASS — the only non-byte-identical change is declared (T-1 path-qualification, T-2 `#[macro_export]`, T-3 ziee use-line swap, T-4 `pub mod`); two `## Decision` blocks (fully-qualified-paths vs `$crate`; keep `Repos` app-side) each carry `**Resolution:**`. Zero TBD.
- E8: PASS (BOTH surfaces) — `golden(types)`: BYTE-IDENTICAL — `diff -q baseline/types.ui.ts src-app/ui/src/api-client/types.ts` AND `diff -q baseline/types.desktop.ts src-app/desktop/ui/src/api-client/types.ts` both clean. `golden(openapi)`: CANONICALLY-EQUAL — `diff <(jq -S baseline) <(jq -S openapi.json)` EMPTY on ui AND desktop. Generated files restored via `git checkout`.
- E9: PASS — SDK standalone `cd sdk && cargo check --workspace` exit 0; ziee `cargo check -p ziee` (lib+bin) exit 0; `cargo check -p ziee-desktop` exit 0 (only pre-existing dead_code warnings in scheduler/mcp, unrelated). (Fresh-worktree clean-build is the orchestrator's pre-merge gate.)
- E10: PASS — the macro is exercised transitively by the whole `Repos`-touching integration suite (unchanged; generated symbols land in the same module). No dedicated macro unit test exists to run. (Full ziee suite + gate:ui run at the pre-merge gate.)
- E11: PASS — `sdk/examples/skeleton-server` still builds in the SDK workspace check; the framework's new `repository` module is a pure-token `#[macro_export]` macro naming no domain type + adding no dep, so the app-agnostic boundary holds.
- E12: PASS — SDK commit builds (`cargo check --workspace` green) and is committed in the submodule; the ziee-side submodule-pointer bump is the orchestrator's step (per task).

ziee-suite: PASS (touched-module scope — the macro compiles + expands in ziee/ziee-desktop cargo check green; full suite + gate:ui at the pre-merge gate)
gate:ui (ui): PASS (n/a — B4 is backend-only; no UI surface touched; types.ts regenerates byte-identical on both surfaces)
golden(openapi): IDENTICAL (canonical — jq -S set-equal on ui + desktop, per E8 REFINEMENT)
golden(types): IDENTICAL (byte-for-byte on ui + desktop)
golden(schema): IDENTICAL (B4 changes no migrations/schema)
