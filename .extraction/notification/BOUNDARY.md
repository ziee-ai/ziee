# Chunk `notification` — BOUNDARY

What `ziee-notification` may and may not name, and why the split keeps the
app-agnostic boundary clean while the crate CARRIES the module's schema.

## `ziee-notification` is domain-free AND build-DB-free (but schema-carrying)

- `models.rs` deps: `chrono`, `schemars`, `serde`, `uuid`, `sqlx::FromRow`.
  `Notification` derives `FromRow` (no DB needed) but issues **no `query!`** — the
  compile-time-verified queries are all in the retained `repository.rs`.
- `permissions.rs` deps: `ziee_identity::PermissionCheck`. Static const key.
- `migrations/` — the module's 2 `.sql` (schema + fkeys), byte-for-byte with their
  original filenames/content, globbed into the app's merged set from
  `sdk/crates/*/migrations/`.
- Grep confirms: no `crate::modules`, no `query!`, no build.rs → the crate compiles
  its sqlx macros WITHOUT a per-crate `DATABASE_URL` / build DB.

## The boundary line — what stayed app-side (the load-bearing decision)

Unlike `ziee-auth` (self-contained identity tables → its own crate build DB),
`notification` sits ON TOP of other modules: its fkeys reference
`users`/`conversations`/`scheduled_tasks`/`workflow_runs`. So:

- `repository.rs` (compile-time `query_as!` ×10) STAYS in ziee — its queries verify
  only against the app's FULL merged build DB. Keeping it app-side is what lets the
  crate be build-DB-free.
- `events.rs` STAYS — it names the concrete `crate::modules::sync::
  {SyncEntity, SyncAction, publish}` (app-owned sync enums with the
  `SyncEntity::Notification` variant).
- `handlers.rs`/`routes.rs`/`prune.rs` STAY — they name `RequirePermissions`,
  `crate::core::Repos`, `SyncOrigin`, the aide/axum boundary.
- `mod.rs` keeps the `AppModule` registration (name "notification", order 89) + the
  prune-loop spawn.

The crate is the reusable TYPES + SCHEMA seed; the app owns the schema-bound
queries + sync + HTTP glue.

## E-gates (this chunk)

- **E (cargo):** `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk &&
  cargo check --workspace` = 0.
- **N7 (migration composition):** ziee's `build.rs` globs the crate's `migrations/`
  into the merged set, provisions the build DB, and the retained `repository.rs`'s
  `query_as!` verify against it — all proven by `cargo check -p ziee` exit 0.
- **E8 (golden, BOTH surfaces):** `types.ui.ts` + `types.desktop.ts`
  **BYTE-IDENTICAL**; `openapi.ui.json` + `openapi.desktop.json`
  **CANONICALLY-EQUAL** (jq -S) vs `.extraction/baseline/`. Generated paths restored
  via `git checkout`.
- **test-fidelity:** `cargo test -p ziee-notification` → 0 tests (no in-source
  units in the moved files); the schema↔types match is proven by `cargo check -p
  ziee`.
