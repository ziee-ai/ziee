# Chunk `sdk-standalone-fixes` — three standalone-consumption defects (CUT manifest)

Not an extraction (nothing new is CUT from ziee into the SDK). This chunk FIXES
three defects a second standalone app (CytoAnalyst) hit adopting the merged SDK —
defects ziee's own equivalence gates missed because ziee itself masks each one.
Full context: `tmp/cytoanalyst/SDK_GAPS.md`, "SDK batteries audit" gaps N-1/N-2/N-3.

The unifying theme: an SDK *infra* crate / helper that works FOR ZIEE (which has
the full domain schema, its own hand-built router, and its own resolver wiring)
but breaks a fresh app that has only a subset. Each fix makes the SDK piece
domain-agnostic / scope-explicit / module-system-composable, WITHOUT changing
ziee's behavior or its API surface (golden byte-identical, EA fingerprint
unchanged).

## Decision N-1 (HIGH · domain-FK LEAK) — infra-crate migration must apply STANDALONE

`sdk/crates/ziee-notification/migrations/202607144180_notification_fkeys.sql`
added FK constraints on `public.{conversations, scheduled_tasks, workflow_runs,
users}` — chat/scheduler/workflow (ziee-domain) + auth tables, none created by
`ziee-notification` or its ONE declared dep (`ziee-identity`, which ships no
migrations). A fresh app linking `ziee-notification` without those domain tables
fails to migrate (`relation "public.conversations" does not exist`).

- **Fix:** the domain FK constraints move OUT of the SDK crate migration and
  ziee-side. The SDK `notifications` table keeps `conversation_id` /
  `scheduled_task_id` / `workflow_run_id` (plain nullable UUID) + `user_id`
  (NOT NULL) as **plain columns, no FK** → the crate is domain-agnostic and its
  `migrations/` apply standalone. ziee re-adds all four FK constraints in a
  ziee module migration.
- **Relocation, not rewrite:** the fkeys `.sql` is moved BYTE-IDENTICAL
  (sha256 `02abd32c…`) from `sdk/crates/ziee-notification/migrations/` to
  `server/src/modules/notification/migrations/`. Both source roots are already
  in the merged composition (SDK crate via the explicit list — see N-2; ziee
  module via the `src/modules` glob), so the merged set is byte-for-byte
  unchanged (92 files, verified `diff -rq` empty) → the final schema (and its
  EA fingerprint) is identical, the FKs merely authored ziee-side. The migration
  version `202607144180` already sits in the fkeys range (after every `440xx`
  schema migration and every domain table exists), and there was a reserved gap
  (memory `4175` → onboarding `4185`) exactly at `4180` for notification.
- **Full-crate audit (`grep -rn "REFERENCES public\." sdk/crates/*/migrations/`):**
  - `ziee-auth` — CLEAN. All FKs reference `users`/`groups`/`auth_providers`,
    all self-created in `ziee-auth/migrations`. Applies standalone.
  - `ziee-file` — CLEAN. No FKs at all; the one former domain leak
    (`files.workflow_run_id`) was already side-tabled to ziee's
    `file_workflow_runs` (chunk `ziee-file`, N9). Applies standalone.
  - `ziee-notification` — LEAKED (4 FKs). FIXED as above.
  Only `ziee-notification` leaked.

## Decision N-2 (MED) — `compose_merged_migrations` must scope to LINKED crates

`ziee_build_support::compose_merged_migrations(merged, roots)` globbed
`<root>/*/migrations` for EVERY crate under `sdk/crates`, regardless of what the
app links — so a fresh app depending only on `ziee-auth` still merged in
`ziee-file` + `ziee-notification` migrations (→ N-1 breakage).

- **Fix:** add `compose_merged_migrations_from(merged, app_module_roots,
  sdk_crate_migration_dirs)`: the app's own module roots are still GLOBBED; the
  SDK-crate migration dirs are passed EXPLICITLY (one `.../migrations` per
  schema-bound dep the app actually links), never globbed. The old glob
  signature is kept working (it now delegates with an empty explicit list).
- ziee's `build.rs` moves to the explicit variant, naming its three schema-bound
  SDK crates (`ziee-auth`, `ziee-file`, `ziee-notification`) — exactly the crates
  the old blind glob picked up, so the merged set is byte-identical.

## Decision N-3 (MED) — a turnkey auth MODULE for the `build_api_router` path

`ziee_auth::auth::mount_auth` fits an app that builds+nests its router by hand,
but NOT the standard module-system path: `build_api_router` nests every module
under `/api`, so `mount_auth` lands auth at `/auth` (not `/api/auth`) and its
`Extension` layers cover only the auth sub-router — the framework's
`RequirePermissions` extractor then finds no resolver on other modules' routes.

- **Fix:** ship `ziee_auth::auth::module::AuthModule` — a
  `#[distributed_slice(MODULE_ENTRIES)]`-registered `AppModule` (concrete
  `DefaultIdentityResolver`) that (1) registers `auth_routes` + `auth_admin_routes`
  through the module system → `/api/auth/*`, and (2) installs the resolver / JWT /
  AuthContext extensions at the WHOLE-APP router level (it registers LAST,
  `order = i32::MAX`, so its `Router::layer` covers every module's routes). Plus
  the boot side-effects `mount_auth` does (trust flag + one-time session-settings
  seed). This is exactly what CytoAnalyst hand-wrote — now SDK-owned.
- **Gated behind the non-default `module` feature** (implies `routes` + `linkme`).
  ziee does NOT enable it (it keeps its own `ZieeIdentityResolver`-backed module),
  so ziee never links a second, conflicting auth module → ziee's route/OpenAPI
  surface is byte-identical. `mount_auth` is retained for hand-built routers.

## Gate improvement — standalone-migration-apply

New reusable gate `standalone_apply_gate.sh`: for each schema-bound SDK crate,
apply its `migrations/` ALONE (plus only its declared crate-deps' migrations) to
a fresh DB and require NO "relation does not exist". Closes the hole that let N-1
through (ziee's gates only ever applied the FULL merged set). Proven: all three
crates PASS; reintroducing the leak makes `ziee-notification` FAIL (negative
control).

## Files

- del:  `sdk/crates/ziee-notification/migrations/202607144180_notification_fkeys.sql`
- new:  `server/src/modules/notification/migrations/202607144180_notification_fkeys.sql` (byte-identical relocation)
- edit: `sdk/crates/ziee-notification/Cargo.toml` (doc comment: standalone-applicable)
- edit: `sdk/crates/ziee-build-support/src/migrations.rs` (+`compose_merged_migrations_from`; old fn delegates)
- edit: `sdk/crates/ziee-build-support/src/lib.rs` (export the new fn)
- edit: `server/build.rs` (explicit SDK-crate migration dirs)
- new:  `sdk/crates/ziee-auth/src/auth/module.rs` (`AuthModule` + 2 tests)
- edit: `sdk/crates/ziee-auth/src/auth/mod.rs` (`#[cfg(feature="module")] pub mod module; pub use module::AuthModule`)
- edit: `sdk/crates/ziee-auth/Cargo.toml` (`module` feature + optional `linkme` + `tower`/`http-body-util` dev-deps)
- new:  `.extraction/sdk-standalone-fixes/standalone_apply_gate.sh` (per-crate standalone gate)
