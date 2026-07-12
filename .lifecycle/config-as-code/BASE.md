# BASE — conflict-surface scoping (P3)

Branch: `feat/config-as-code`, cut from `origin/khoi` @ `9323ffda3`
("Merge remote-tracking branch 'origin/main' into khoi"). `origin/khoi == origin/main` at cut time.
PR target: **`khoi`**.

## Migration numbers

- Highest existing: `00000000000156_create_voice_models.sql`.
- This branch adds exactly ONE: **`00000000000157_remove_unused_builtin_mcp_servers.sql`**.
- Collision risk: another in-flight worker also claiming 157. Re-checked by the merge-gate (C2)
  against real main at merge time; if it collides, renumber to the next free slot (the migration is
  order-independent — a pure DELETE of 3 rows seeded by migration 7).
- Note: migration 7 is NOT edited (sqlx checksums applied migrations).

## Files main is also likely to touch

- `src-app/server/src/main.rs` — 2-line insertion in the boot prologue; low collision risk, but a
  concurrent feature adding its own boot hook could touch adjacent lines.
- `src-app/server/src/modules/mod.rs` + `src-app/server/tests/integration_tests.rs` — single-line
  `mod` registrations; classic append-collision spot (trivial to resolve).
- `src-app/server/tests/mcp/mod.rs` — retargeting 3 asserts off the deleted `filesystem` row.
- `src-app/ui/tests/e2e/07-mcp/mcp-admin-servers.spec.ts`,
  `src-app/ui/tests/e2e/chat/mcp-chip-row-persistence.spec.ts` — string retargets.
- `docker/web/Dockerfile`, `docker/web/README.md`, `docker-compose.yml` — the deploy surface the
  lead/user is also actively working (TeamCity). Coordinate via `config-as-code.STATUS.md`.

## OpenAPI regen

**Not implied.** No route, request/response type, permission, or `SyncEntity` is added or changed
— the reconciler is boot-time-only code and the migration only deletes rows. Therefore
`openapi.json` / `api-client/types.ts` are untouched in BOTH `ui/` and `desktop/ui/`, and this diff
is NOT treated as UI work by the phase-3/phase-8 frontend gates on the basis of a regen.
(The e2e spec edits DO make it a frontend-touching diff — the UI gates are budgeted for that.)
