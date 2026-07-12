# PLAN_AUDIT — config-as-code (plan vs. the actual codebase)

Every claim below was checked against the tree at `9323ffda3`.

## Breakage risk

- **Boot path**: the reconciler runs INSIDE `main.rs` only (not `lib.rs::setup_server`), gated on
  `ZIEE_DESIRED_STATE_FILE` being set AND the file existing. Desktop (`ziee-desktop`) uses
  `lib.rs::setup_server` and never sets that env → **zero desktop impact**. Every existing
  integration test spawns the `ziee` binary WITHOUT that env → **zero impact on the existing
  suite**. Confirmed the harness passes env through `TestServerOptions::extra_env`
  (`tests/common/harness_inner.rs:333`), so the new tests need **no harness edit** (B3 respected).
- **Deleting `filesystem` / `browser` / `git` (migration 157)**: verified **no runtime code path**
  resolves these by name or id — they are absent from `auto_attach_builtin_ids` and
  `is_builtin_server_id` (`mcp/chat_extension/mcp.rs`), skipped by connection-health
  (`WHERE is_built_in = false`, `repository.rs:974`), and referenced only in comments. They are
  `enabled = false` and assigned to no group. Real breakage is limited to TESTS
  (`tests/mcp/mod.rs` count-≥4 + two `filesystem` lookups; two e2e specs driving the
  "Filesystem Access" card) → ITEM-10 fixes them. `fetch` is deliberately KEPT (it is enabled and
  assigned to the default group, and 6+ tests depend on it). `files`/`files.ziee.internal` is a
  DIFFERENT, load-bearing row (http loopback, deterministic v5 id, auto-attached,
  approval-bypassed) and is untouched.
- **Removing `assistants::*` from the Users group**: the chat composer's assistant picker
  (`ui/src/modules/assistant/chat-extension/`) is NOT permission-gated, so a non-admin user's
  picker will issue a request that now 403s. Server-side chat is unaffected (the assistant
  chat-extension resolves `conversation.assistant_id` without a permission check). Per the user's
  explicit instruction ("just set permission in the db, don't delete anything") we do **not**
  change product UI code. **Tracked as a real risk**: the phase-8 `gate:ui` runtime-health pass
  classifies a failed request as HIGH, but it drives the GALLERY (mock cassette, no live 403), so
  it will not fire there. The restricted-user e2e (TEST) will observe the live behavior; if it
  surfaces a broken composer for the seeded user, that becomes a HUMAN_FEEDBACK item to raise
  rather than a silent scope expansion. → see DEC-8.
- **`Repos.group.update` full-array replacement** (`user/repository.rs:524`, `permissions =
  COALESCE($4, permissions)`): we must read-modify-write the array. Two boots racing is not a
  concern (single container, reconcile runs before serving), but a concurrent admin edit during
  reconcile could be lost. Acceptable: reconcile completes before the server accepts connections.
- **`create_system_server` does not run the handler's `validate_sandbox_fields_create`**: our
  entries are `transport_type: http` with no `command`, so that validator is a no-op for them; the
  repo still runs `validate_transport_config` (url present + http/https) and
  `validate_header_entries`. No gap.
- **Soft-fail contract**: a malformed entry / unset `${VAR}` / DB error on ONE entry logs an error
  and is skipped; boot continues. A malformed FILE (unparseable YAML) also logs + skips (no crash),
  matching the task's "must not crash boot for a soft entry".

## Pattern conformance

- `seed_from_config_once` (`auth/session_settings.rs:113`) — the "config → DB once, DB
  authoritative after" contract our `ensure` mode implements. Conformant.
- `ensure_desktop_admin` (`desktop/tauri/src/modules/auth/bootstrap.rs`) — the exact precedent for
  boot-time admin creation (`Repos.user.has_admin()` → `Repos.app.create_admin_user`). Conformant;
  we reuse the same two repo calls plus `auth::password::hash_password` (pub, `password.rs:37`) and
  `validate_password_strength` (pub, `password.rs:20`).
- System-MCP creation from non-HTTP code — precedent `hub/handlers.rs:1331`. We call
  `Repos.mcp.create_system_server(request)` (`mcp/repository.rs:463`, pool-only, no JWT/user_id;
  `is_system=true`/`user_id=NULL` are set by construction). Group assignment via the wrapper
  `Repos.mcp.assign_to_group(server_id, group_id)` (`mcp/repository.rs:591` → free fn
  `assign_mcp_server_to_group`, `:2106`), which is already `ON CONFLICT DO NOTHING` and rejects
  non-system servers. Conformant.
- Group-permission removal — migration 37 is the in-tree precedent for the SEMANTICS; we do it
  declaratively at boot with `Repos.group.get_by_name` + `Repos.group.update`. Note the REST route
  would REFUSE this (`user/handlers/groups.rs:176-191` rejects perm edits on `is_system` groups),
  so the direct-repo write is not just convenient, it is **required**. Conformant.
- YAML parsing — `serde_norway` is already the workspace YAML crate (`server/Cargo.toml:46`), the
  same one `Config::load_from` uses. No new dependency.
- Deviation from the task brief, deliberate: the brief asked for **v5 UUIDs** as the idempotency
  key. `create_system_server` does not accept a caller-supplied `id` (the INSERT omits it), so
  minting a v5 id would force a new repository function. Since `name` is already the stable key and
  a `(name, is_system)` existence check is exactly as idempotent, we dedup on that and touch no
  shared repository code. Recorded as DEC-4.

## Migration collisions

- Highest existing migration: `00000000000156_create_voice_models.sql`. This branch adds exactly
  one: **157**. No other file in this diff is a migration.
- Migration 7 (which seeded the 4 rows) is **NOT** edited — sqlx records a checksum per applied
  migration, so an in-place edit would hard-fail boot on every existing deployment (incl. the
  user's live :8080 instance). The DELETE-in-a-new-migration is the only safe removal.
- The migration is order-independent (a pure DELETE), so a renumber at merge time (if a
  concurrent worker also claims 157) is safe. Re-checked by merge-gate C2.

## OpenAPI regen

**Not required.** The diff adds no route, no request/response type, no permission constant, and no
`SyncEntity`; the migration only deletes rows. `openapi.json` and `api-client/types.ts` are
untouched in BOTH `ui/` and `desktop/ui/` — verified by the absence of any `routes.rs` / `types.rs`
/ `permissions.rs` edit in "Files to touch". (The diff IS still frontend-touching via two e2e spec
edits, so the phase-8 `npm run check` + `gate:ui` gates apply; that is budgeted.)

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new module, no existing caller to break; `serde_norway` already a dep.
- **ITEM-2** — verdict: PASS — `Repos.mcp.create_system_server` (`repository.rs:463`) is
  pool-only/JWT-free; the `(name, is_system)` guard is mandatory because NO unique index exists on
  `mcp_servers.name` (verified across all migrations) — an unguarded second call WOULD duplicate.
- **ITEM-3** — verdict: PASS — `Repos.mcp.assign_to_group` (`repository.rs:591`) is idempotent
  (`ON CONFLICT (group_id, mcp_server_id) DO NOTHING`) and validates `is_system`.
- **ITEM-4** — verdict: PASS — mirrors `ensure_desktop_admin` exactly; `create_admin_user` is
  transactional, re-checks admin-exists inside the tx, and joins Administrators + Users itself.
  `unique_root_admin` (partial unique index) is the DB-level backstop against a double-create.
- **ITEM-5** — verdict: PASS — `Repos.auth.create_local_user_with_default_group`
  (`auth/repository.rs:42`) is what `POST /auth/register` uses; `users.username` and `users.email`
  are both UNIQUE, so the pre-check + the constraint together make it idempotent.
- **ITEM-6** — verdict: CONCERN — `Repos.group.update` replaces the WHOLE permissions array
  (read-modify-write). Mitigated: reconcile runs before the server serves, so no concurrent admin
  edit can be lost. Also: the Users group does NOT currently hold `projects::*` (migration 54
  granted those to Administrators only), so that removal is a declared no-op today — kept in the
  file for intent + future-proofing (a later migration granting projects to Users would be
  re-removed by `enforce`).
- **ITEM-7** — verdict: PASS — new file; `${...}`-only secrets is enforced by the parser (DEC-3).
- **ITEM-8** — verdict: PASS — insertion point verified: `main.rs` after
  `core::secrets::init_storage_key` (:224), before `ModuleContext::new` (:227) — pool, `Repos`, and
  the storage key are all live, migrations have run (inside `initialize_database`, :201), and
  nothing is served yet (`axum::serve`, :390).
- **ITEM-9** — verdict: PASS — 157 is free; no runtime code resolves the three rows; `fetch` and
  `files` explicitly preserved.
- **ITEM-10** — verdict: CONCERN — this is the one unavoidable blast-radius item: deleting the rows
  breaks 3 backend asserts + 2 e2e specs that were written against seeded fixture data. The fix is
  mechanical (retarget at `fetch`/"Web Fetch"), but it makes this a frontend-touching diff, which
  pulls in the phase-8 `npm run check` + `gate:ui` gates. Budgeted, not descoped.
- **ITEM-11** — verdict: PASS — `docker/web/Dockerfile` already COPYs a config + sets `ENV`
  defaults for every var; we add one COPY + one ENV, and compose passes the 5 new vars through with
  empty defaults (empty → the entry is skipped, so the committed compose still boots with no env).

No `BLOCKED` verdicts.
