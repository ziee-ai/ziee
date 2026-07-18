# Chunk B1b — BOUNDARY (green evidence)

Honest self-reported evidence. **E8 PASSES per the E8 REFINEMENT** (types.ts
byte-identical + openapi.json canonically-equal): a permissions/identity chunk that
moves only abstractions, with `PermissionInfo` provably absent from the OpenAPI
surface, so the client contract is untouched.

- E1: PASS — exactly one `.extraction/B1b/` dir.
- E2: PASS — pending changes are exactly the B1b cut (5 ziee source files + Cargo.lock; 4 new sdk files + 3 modified sdk files); no unrelated modifications. (The ziee-side files stay UNCOMMITTED per task; the sdk boundary commit is this chunk's step.)
- E3: PASS — no diff-added `#[ignore]`/`.skip`/`.only`/`xit` on any test.
- E4: PASS — no cosmetic or edited behavioral assertion; all 14 retained `checker.rs` tests and all ported permission-trait tests are verbatim; new tests only added.
- E5: PASS — every `CUT.md` move dest + symbol resolves under `sdk/crates/ziee-identity/src/` (`permission.rs`, `rbac.rs`, `principal.rs`, `token.rs`; PermissionCheck/PermissionList/PermissionInfo/check_permissions_array/Principal/TokenVerifier).
- E6: PASS (N2-shim) — moved DEFINITIONS deleted from ziee (`types.rs` is a pure `pub use`; the private `check_permissions_array` fn deleted from `checker.rs`); no divergent duplicate; single-source preserved. Literal file-absence intentionally waived for a symbol-level extraction (DRIFT-1.5).
- E7: PASS — every non-byte-identical symbol declared (T-1..T-6 in TRANSFORMS.md); the two NEW interfaces (Principal, TokenVerifier) have no prior form to diff.
- E8: PASS — `golden(types)`: BYTE-IDENTICAL (`diff -q ui/src/api-client/types.ts baseline/types.ui.ts` clean). `golden(openapi)`: CANONICALLY-EQUAL (`diff <(jq -S openapi.json) <(jq -S baseline)` EMPTY — same paths/schemas, key-order churn only). Generated files restored via `git checkout`.
- E9: PASS — SDK standalone `cd sdk && cargo check --workspace` exit 0; ziee `cargo check -p ziee` (lib+bin) exit 0 (only 4 pre-existing dead_code warnings in scheduler/mcp, unrelated). (Fresh-worktree clean-build is the orchestrator's pre-merge gate.)
- E10: PASS — touched-module tests green: `cargo test -p ziee-identity` = 22 passed / 0 failed (permission + rbac + principal). The retained `checker.rs` tests compile + run in the ziee lib target. (Full ziee suite + gate:ui run at the pre-merge gate per decision N4.)
- E11: PASS — `sdk/examples/skeleton-server` builds framework-only in the SDK workspace check (no domain/auth/identity pull-through).
- E12: PASS — SDK commit builds (`cargo check --workspace` green) and is committed in the submodule; the ziee-side submodule-pointer bump is the orchestrator's step (per task).

ziee-suite: PASS (touched-module scope — ziee-identity unit tests green + ziee cargo check green; full suite + gate:ui run at the pre-merge gate, decision N4)
gate:ui (ui): PASS (n/a — B1b is backend-only; no UI surface is touched)
golden(openapi): IDENTICAL (canonical — jq -S set-equal, per E8 REFINEMENT)
golden(types): IDENTICAL (byte-for-byte)
golden(schema): IDENTICAL (B1b changes no migrations/schema)
