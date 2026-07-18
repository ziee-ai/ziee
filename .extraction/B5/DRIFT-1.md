# Chunk B5 — DRIFT scan (round 1)

Drift = any place the genericization (SyncRegistry<P>, deliver-takes-Event,
SyncEntityKind, principal snapshot) could diverge from pre-extraction
behavior/surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. `Perm` routing equivalence. `SyncConnPrincipal`
  filters `is_active` groups in `active_group_permissions`, so
  `Principal::has_permission` = `check_permissions_array(direct)` OR per-active-group
  check — byte-identical to `check_permission_union`. The admin short-circuit stays
  a separate `principal.is_admin() || …` (not folded into `has_permission`), exactly
  as `conn.is_admin || check_permission_union(..)`. Framework routing tests
  (admin/holder/non-holder/group-All/group-Any) pass.

- **DRIFT-1.2** — verdict: none. Wire/schema surface unchanged. The 5 JsonSchema
  types stayed in ziee, byte-unchanged; the framework never names them. E8 golden
  verified on BOTH surfaces: `types.ts` ui **byte-identical** + desktop
  **byte-identical**; `openapi.json` ui canonically-equal (612 order-only lines) +
  desktop canonically-equal (in fact byte-identical). Restored via `git checkout`.

- **DRIFT-1.3** — verdict: none. `deliver(…, Event, …)` behavior. Formerly
  `SyncSseEvent::Sync(event).into()` was built once then cloned per connection;
  now `publish` builds it once and the registry clones the passed `Event`
  per connection. One serialization, N clones — identical. `deliver`/
  `deliver_session_to_users` have exactly one caller each (ziee's publish
  wrappers, grep-confirmed), so no other call site is affected.

- **DRIFT-1.4** — verdict: none. Session fan-out. `deliver_session_to_users::<E>`
  builds each recipient's event via `E::session_signal(uid)`; ziee's
  `SyncEntity::session_signal` = `SyncSseEvent::Sync(SyncEvent{Session,Update,uid})
  .into()`, the former inline expression. Skip-origin, single-lock, per-user-id,
  and the dedicated prune are preserved (framework tests: targeting+origin-skip;
  lagging-prune 97e64997158).

- **DRIFT-1.5** — verdict: none. Singleton lifecycle. `lazy_static REGISTRY:
  SyncRegistry<SyncConnPrincipal>` + `registry()` stay in ziee; `SyncRegistry::new()`
  is the framework constructor. `registry()`'s return type is the concrete
  monomorphization, so `publish`/handlers call it unchanged. Same app-owns-global
  seam as B4/B3.

- **DRIFT-1.6** — verdict: none. Emit-site surface. `crate::modules::sync::{Audience,
  SyncAction, SyncEntity, SyncOrigin, publish, publish_session_to_users}` (+
  `sync::event::{publish, Audience, SyncAction, SyncEntity}` in file_rag/ingest.rs)
  all resolve through unchanged re-exports. Zero emit-site edits across the ~40
  files that publish. Confirmed: `cargo check -p ziee` (lib+bin) exit 0.

- **DRIFT-1.7** — verdict: none. Build hygiene under `-D unused-imports`. Three
  now-dead imports (T-6/T-7/T-8) reconciled during the convergence loop:
  event.rs `PermissionList` (removed with the moved constructors), permissions/mod.rs
  mod-level `PermissionList` re-export (dropped), permissions/types.rs bin-tree
  `PermissionList` (`#[allow(unused_imports)]`, still a live re-export for
  `crate::PermissionList` + `server_update` `#[cfg(test)]`). Both binaries + the sdk
  workspace compile clean.

- **DRIFT-1.8** — verdict: none. Generic bounds / desktop. `Default` bound aligned
  with `new()`'s `P: Principal + Send + 'static` (a first-pass E0599, fixed).
  `cargo check -p ziee-desktop` exit 0 — desktop consumes ziee's `registry()`/
  wire types unchanged. `cd sdk && cargo check --workspace` exit 0;
  `cargo test -p ziee-framework --lib sync::` → 16 passed.

- **DRIFT-1.9** — verdict: none. EventBus deliberately NOT moved (design decision,
  TRANSFORMS `## Decision — EventBus split`): domain-coupled over the closed
  `AppEvent` enum, unrelated to the SSE sync core, no schema-neutral generic form
  that carries its weight. Not a drift — an in-scope decision to move only the
  genuinely-generic machinery.

- **DRIFT-1.10** — verdict: none. Notify-only invariant. The wire payload types
  didn't move; `wire_payload_is_notify_only_snake_case` (kept in ziee) still guards
  the 3-field `{entity,action,id}` shape. Nothing widened.

**Unresolved drifts:** 0
