# Chunk `server-update` — BOUNDARY

What `ziee-server-update` may and may not name, and why the split keeps the
app-agnostic + build-DB-free boundary clean.

## `ziee-server-update` is domain-free AND build-DB-free

- `types.rs` deps: `serde`, `schemars`. `UpdateStatusResponse` is a plain wire
  struct.
- `permissions.rs` deps: `ziee_identity::PermissionCheck` (+ `PermissionList` in
  the test). Static const key.
- Grep confirms: no `crate::modules`, no `sqlx`/`query!`, no build.rs, and
  crucially **no `env!("CARGO_PKG_VERSION")`** (that stayed with `checker`).

## The boundary line — what stayed app-side (the load-bearing decision)

`server_update/checker.rs` STAYS in ziee: `env!("CARGO_PKG_VERSION")` must compile
to ziee's version (a move would report the crate's `0.0.0` at
`/api/server-update/status` — a behavior change), and a test names
`crate::core::config::UpdateCheckConfig`. `handlers.rs`/`routes.rs` name
`RequirePermissions`/`with_permission` (ziee's resolver alias), and the `mod.rs`
holds the `AppModule` registration + the daily-poll spawn + `checker::set_enabled`.
So only the two files with ZERO app/version coupling move. This is a deliberately
thin extraction — the honest equivalence-preserving boundary, not maximal code
movement.

## E-gates (this chunk)

- **E (cargo):** `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk &&
  cargo check --workspace` = 0.
- **E8 (golden, BOTH surfaces):** `types.ui.ts` + `types.desktop.ts`
  **BYTE-IDENTICAL**; `openapi.ui.json` + `openapi.desktop.json`
  **CANONICALLY-EQUAL** (jq -S) vs `.extraction/baseline/`. The move touches no
  route/schema/permission-string/OpenAPI-visible type. Generated paths restored via
  `git checkout`.
- **test-fidelity:** `cargo test -p ziee-server-update` → 1 passed.
