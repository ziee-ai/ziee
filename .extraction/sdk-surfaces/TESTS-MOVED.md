# Chunk `sdk-surfaces` — TESTS-MOVED

## Deliverable 1 — `sync_routes`

### Moved INTO `ziee-framework`

The `subscribe_sync` handler carried NO in-source `#[cfg(test)]` units (the sync
unit tests — audience routing, registry caps/prune, self-echo, session fan-out,
the wire-format tests — already live in `ziee_framework::sync::{registry,audience}`
and ziee's `event.rs` from chunk B5; those are UNTOUCHED). `cargo test -p
ziee-framework` still compiles + passes; the registry's `TestEntity` mock
(`SyncEntityKind`, `session_signal` only) is intact because `SyncSurface` is a
separate trait, not a widening.

### Stayed in ziee (integration)

| Test home | Why it stayed |
|---|---|
| `tests/sync/subscribe_test.rs` | Drives the real `GET /api/sync/subscribe` (now the mounted `sync_routes`) through the app `TestServer` — subscribe auth-gate (401) + the SSE handshake. Endpoint path/response unchanged, so it exercises the moved handler end-to-end. |
| `ui/tests/e2e/13-sync/` | Cross-device delivery + cross-user isolation (unchanged surface). |

## Deliverable 2 — `ziee-onboarding`

### Moved INTO the crate

`models.rs` + `repository.rs` carried NO `#[cfg(test)]` units, so none moved.
`cargo test -p ziee-onboarding` → **0 tests** (compiles clean). The crate's `query!`
verifying against its own build DB is the compile-time proof the moved queries
match the moved schema.

### Stayed in ziee

| Test home | Why it stayed |
|---|---|
| `onboarding/handlers.rs` `#[cfg(test)]` (4 units: `is_valid_onboarding_id` slug/empty/max-len/disallowed-chars) | The id-validator is app-side (in the retained handler); moved byte-unchanged with the handler. |
| `tests/onboarding/*` (integration) | Drive the retained handlers/routes (`RequirePermissions`/`Repos`/`SyncEntity::Onboarding`/`JwtAuth`) through the app `TestServer` against a live DB — completion cap, validation, sync-emit. |

No behavioral assertion was edited. The MOVE-preserves-behavior discipline holds:
the only content edits are the `AppError` import (repository) and the `use` re-points
(shims).

## Runnability note

`source src-app/server/tests/.env.test; cargo test --test integration_tests sync::
onboarding:: -- --test-threads=1` — the harness spawns a server subprocess from
`src-app/target/debug/ziee`; because this worktree builds under a private
`CARGO_TARGET_DIR`, the binary must be symlinked/available there for the harness to
find it (reported honestly in the run notes).
