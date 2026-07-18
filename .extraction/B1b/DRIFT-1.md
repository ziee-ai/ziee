# Chunk B1b — DRIFT round 1

Reconciliation of the moved code against `CUT.md`/`TRANSFORMS.md` and the
equivalence tripwires.

- **DRIFT-1.1** — Every moved file resolves in the SDK: `permission.rs`, `rbac.rs`,
  `principal.rs`, `token.rs` all exist under `sdk/crates/ziee-identity/src/`, and
  every `## Symbols` entry (`PermissionCheck`, `PermissionList`, `PermissionInfo`,
  `check_permissions_array`, `Principal`, `TokenVerifier`) is present and re-exported
  at the crate root. — verdict: none

- **DRIFT-1.2** — Every changed/new symbol is declared in `TRANSFORMS.md`: T-1
  (`check_permissions_array` vis), T-2 (`check_permission_union` delegation), T-3
  (`PermissionInfo` move + derives), T-4 (`Principal` new), T-5 (`TokenVerifier`
  new), T-6 (`types.rs` shim). No undeclared non-byte-identical change remains. —
  verdict: none

- **DRIFT-1.3** — No stale ziee reference points at the old locations: `types.rs`
  re-exports the three trait/DTO symbols (consumed via `mod.rs`, `openapi.rs`,
  `user/permissions.rs` — all resolve); `checker.rs` imports the moved
  `check_permissions_array` and every `check_permission_union` call site (~20 across
  sync/user/control_mcp/hub/memory_mcp/mcp/workflow/file/chat/files_mcp/extractors)
  resolves unchanged; `impl Principal for User` + `impl TokenVerifier for JwtService`
  compile. `cargo check -p ziee` (lib + bin) is green (exit 0). — verdict: resolved

- **DRIFT-1.4** — **Equivalence tripwire: `types.ts` BYTE-IDENTICAL, `openapi.json`
  CANONICALLY-EQUAL.** After `--generate-openapi` for the ui binary,
  `src-app/ui/src/api-client/types.ts` is **byte-identical** to the committed
  baseline (`.extraction/baseline/types.ui.ts`), and `jq -S`-canonicalized
  `openapi.json` equals the canonicalized baseline (`openapi.ui.json`) — same
  paths/schemas, JSON key-order churn only (the linkme route-registration order is a
  deterministic function of the dependency graph; adding a path-dep perturbs order
  but adds/removes/renames nothing). Per the E8 REFINEMENT this is the pass
  condition. `PermissionInfo` never appears in either output (it is not registered
  into the spec), so its relocation is provably schema-neutral. — verdict: none

- **DRIFT-1.5** — **N2 shim vs. E6 file-absence.** `CUT.md`'s two `move:` sources
  (`permissions/types.rs`, `permissions/checker.rs`) are RETAINED — `types.rs` as a
  pure `pub use` shim, `checker.rs` as the retained concrete `check_permission_union`
  + its 14 tests (the moved private fn deleted). No divergent duplicate definition
  remains in ziee (the trait/DTO defs and the generic-eval body exist ONLY in
  `ziee-identity`). Single-source is preserved; the literal `E6 source-absent` file
  check is intentionally waived for a symbol-level extraction. — verdict: resolved

- **DRIFT-1.6** — **No concrete-type leakage into `ziee-identity`.** The moved crate
  references no `User`/`Group`/`JwtService`/`Claims`/table type; `Principal` and
  `TokenVerifier` are pure abstractions (associated types keep `jsonwebtoken`/
  `AppError` out). The concrete impls (`impl Principal for User`, `impl TokenVerifier
  for JwtService`) live in ziee. Design-gate #1 (pluggable identity) is satisfied. —
  verdict: none

**Unresolved drifts:** 0
