# Chunk B3 — BOUNDARY (green evidence)

Honest self-reported evidence. **E8 PASSES per the E8 REFINEMENT** (types.ts
byte-identical + openapi.json canonically-equal) on BOTH surfaces: moving
permission enforcement keeps the `PermissionError` 403 schema (schemars
short-name keyed) and the `with_permission` registration identical, so the client
contract is untouched.

- E1: PASS — exactly one `.extraction/B3/` dir.
- E2: PASS — pending changes are exactly the B3 cut: SDK side (ziee-framework 4 new `permissions/*` src files + lib.rs +2 + Cargo.toml deps + Cargo.lock) committed in the submodule; ziee side (`modules/permissions/{extractors,openapi}.rs` shims, `lib.rs`/`main.rs` +1 resolver layer each, Cargo.lock) left UNCOMMITTED per task. No unrelated modifications (pgvector submodule + regenerated openapi/types restored via `git checkout`).
- E3: PASS — no diff-added `#[ignore]`/`.skip`/`.only`/`xit` on any test.
- E4: PASS — no cosmetic or edited behavioral assertion; the moved `with_permission` test is verbatim (only the `PermissionCheck` import path changed); the `check_permission_union` suite in `checker.rs` is unchanged and stays app-side; new assertions: none.
- E5: PASS — every `CUT.md` move dest + symbol resolves: `RequirePermissions`/`RequireAdmin`/`user_holds`/`get_resolver` under `sdk/crates/ziee-framework/src/permissions/extractors.rs`; `with_permission`+`PermissionError`+`PermissionErrorDetails`+`PermissionDetail` under `.../permissions/openapi.rs`; `IdentityResolver` under `.../permissions/resolver.rs`; all re-exported from `ziee-framework::permissions` + crate root.
- E6: PASS — the moved definitions are deleted from ziee (`extractors.rs`/`openapi.rs` retained as shims: resolver-impl+aliases / re-export); no divergent duplicate. `check_permission_union` (concrete, app-side) is deliberately NOT moved (single-source: enforcement in framework, concrete union in ziee — distinct responsibilities).
- E7: PASS — every non-byte-identical symbol declared (T-1..T-8 in TRANSFORMS.md); the resolver split (`authenticate`/`load_groups`) + the `Arc<R>` injection are the only genericizations, both documented in `## Decision` with `**Resolution:**`.
- E8: PASS (BOTH surfaces) — `golden(types)`: BYTE-IDENTICAL — `diff -q ui/src/api-client/types.ts baseline/types.ui.ts` AND `diff -q desktop/ui/src/api-client/types.ts baseline/types.desktop.ts` both clean. `golden(openapi)`: CANONICALLY-EQUAL — `diff <(jq -S openapi.json) <(jq -S baseline)` EMPTY on ui (1724 key-order-only lines) AND desktop (1176 order-only lines). Generated files restored via `git checkout`.
- E9: PASS — SDK standalone `cd sdk && cargo check --workspace` exit 0; ziee `cargo check -p ziee` (lib+bin) exit 0; `cargo check -p ziee-desktop` exit 0 (only pre-existing dead_code warnings in scheduler/mcp/knowledge_base/notification, unrelated). (Fresh-worktree clean-build is the orchestrator's pre-merge gate.)
- E10: PASS — touched-module tests green: `cargo test -p ziee-framework permissions` = 1 passed / 0 failed (the moved `with_permission_documents_403_bearer_and_permission`); the `check_permission_union` suite compiles + runs unchanged in the ziee lib target. (Full ziee suite + gate:ui run at the pre-merge gate per decision N4.)
- E11: PASS — `sdk/examples/skeleton-server` still builds in the SDK workspace check depending only on ziee-core + ziee-framework (no domain/auth/Repos pull-through); the framework's new `permissions` module names only `ziee_core::AppError` + `ziee_identity::Principal` + axum, so the app-agnostic boundary holds (BG de-globalization complete — no framework-facing global remains).
- E12: PASS — SDK commit builds (`cargo check --workspace` green) and is committed in the submodule; the ziee-side submodule-pointer bump is the orchestrator's step (per task).

ziee-suite: PASS (touched-module scope — ziee-framework unit test green + ziee/ziee-desktop cargo check green; full suite + gate:ui run at the pre-merge gate, decision N4)
gate:ui (ui): PASS (n/a — B3 is backend-only; no UI surface is touched, and types.ts regenerates byte-identical on both surfaces)
golden(openapi): IDENTICAL (canonical — jq -S set-equal on ui + desktop, per E8 REFINEMENT; 1724 / 1176 order-only lines)
golden(types): IDENTICAL (byte-for-byte on ui + desktop)
golden(schema): IDENTICAL (B3 changes no migrations/schema)
