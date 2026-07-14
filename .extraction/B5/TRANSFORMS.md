# Chunk B5 — TRANSFORMS (every non-byte-identical change + rationale)

The moved code is byte-identical to its pre-extraction ziee form EXCEPT the
genericization transforms below. The wire types (`SyncEntity`/`SyncAction`/
`SyncEvent`/`SyncConnectedData`/`SyncSseEvent`) did NOT move and are byte-unchanged.

- **T-1** `SyncRegistry` → `SyncRegistry<P: Principal>`; `ClientConn` →
  `ClientConn<P: Principal>` (`ziee-framework::sync::registry`). The per-connection
  permission snapshot — formerly the three fields `is_admin: bool`, `user: User`,
  `groups: Vec<Group>` — collapses to ONE generic field `principal: P` where
  `P: ziee_identity::Principal`. The `Perm`-audience routing changes from
  `conn.is_admin || check_permission_union(&conn.user, &conn.groups, p)` to
  `conn.principal.is_admin() || conn.principal.has_permission(p)`.
  — **why:** de-couples the framework from ziee's concrete `User`/`Group` (the
  last framework-facing identity dependency in sync). `Principal::has_permission`
  is the generic union evaluator extracted in B1b; ziee's `SyncConnPrincipal`
  (T-5) implements it to reproduce `check_permission_union` byte-for-byte
  (direct-perms UNION each ACTIVE group's perms), and `is_admin()` preserves the
  exact call-site short-circuit. Behavior identical.

- **T-2** `SyncRegistry::deliver` signature: `deliver(audience, event: SyncEvent, …)`
  → `deliver(audience, event: axum::response::sse::Event, …)`.
  — **why:** the framework must not name ziee's schema-bearing `SyncEvent`/
  `SyncSseEvent`. The wire event is now serialized APP-SIDE (in `publish`, T-6) and
  the finished axum SSE `Event` is handed to the registry. The registry formerly
  built `SyncSseEvent::Sync(event).into()` ONCE then `.clone()`d per connection; it
  now receives that already-built `Event` and clones it per connection — identical
  observable behavior (one serialization, N clones).

- **T-3** `SyncRegistry::deliver_session_to_users` → `…::<E: SyncEntityKind>`. The
  loop body's inline `SyncSseEvent::Sync(SyncEvent{ entity: Session, action: Update,
  id: uid }).into()` becomes `E::session_signal(uid)`.
  — **why:** the one place the registry MUST synthesize a wire event (it builds a
  per-user Session signal internally) is abstracted through the new `SyncEntityKind`
  seam. ziee's `impl SyncEntityKind for SyncEntity` (T-6) makes `session_signal`
  build exactly the former expression, so the emitted event is byte-identical.

- **T-4** `SyncRegistry::refresh(conn_id, user: User, groups: Vec<Group>)` →
  `refresh(conn_id, principal: P)`. The body's `conn.is_admin = user.is_admin;
  conn.user = user; conn.groups = groups;` becomes `conn.principal = principal;`.
  — **why:** consequence of T-1 (one snapshot field). ziee's handler passes
  `SyncConnPrincipal { user: u, groups: g }` — same data, one value.

- **T-5** NEW `SyncEntityKind` trait (`ziee-framework::sync`):
  `trait SyncEntityKind { fn session_signal(user_id: Uuid) -> Event; }`.
  — **why:** the app-extensible seam replacing the closed `SyncEntity` enum
  (design gate). Deliberately MINIMAL — it abstracts only the single wire event the
  registry itself builds; the per-`{entity,action,id}` `publish` event is NOT
  abstracted (the app serializes it with its concrete, schema-bearing types), which
  is what keeps the OpenAPI/`types.ts` surface byte-identical. See `## Decision`.

- **T-6** ziee `sync/event.rs`: `Audience`/`PermRule` + their 5 typed constructors
  are DELETED and re-exported (`pub use ziee_framework::sync::Audience;`); the 2
  constructor unit tests moved to the framework. ADDED: a private `sync_sse_event()`
  serialization helper, `impl ziee_framework::sync::SyncEntityKind for SyncEntity`,
  and the `publish`/`publish_session_to_users` bodies now call
  `sync_sse_event(...)` / `deliver_session_to_users::<SyncEntity>(...)`.
  — **why:** N2 equivalence shim. Emit sites still `use crate::modules::sync::
  {Audience, …}`; the constructors are the framework's now but resolve through the
  re-export. `publish` serializes the (unchanged) ziee wire event and forwards the
  finished `Event` to the registry — same bytes on the wire.

- **T-7** ziee `permissions/mod.rs`: drop `PermissionList` from
  `pub use types::{PermissionCheck, PermissionInfo, PermissionList}`.
  — **why:** the ONLY `permissions::`-path (mod-level) consumer of `PermissionList`
  was the sync module's `Audience::{all_of, any_of}` constructors (T-6), now in the
  framework. The lib build flags the re-export unused (`-D unused-imports`). The
  trait is unchanged + still reachable via `permissions::types::PermissionList` and
  `crate::PermissionList`; tuple call sites (`<(A,B) as PermissionList>::…`) use it
  by trait resolution, not by name.

- **T-8** ziee `permissions/types.rs`: split the `ziee_identity` re-export so
  `pub use ziee_identity::PermissionList;` carries `#[allow(unused_imports)]`.
  — **why:** ziee compiles as BOTH a lib and a `--bin` with independent module
  trees. `PermissionList`'s last by-name consumer in the BIN tree was the moved
  audience constructors; the only remaining bin consumers are `#[cfg(test)]`
  (`server_update`) + the lib-root `crate::PermissionList` re-export (absent from
  the bin root). So a non-test `--bin` build flags `types.rs` unused. The trait
  MUST stay re-exported (B1b shim + those two consumers), so tolerate the unused
  state rather than delete a live re-export.

## Decision — `SyncEntity` extensibility: trait-seam, NOT generic enum

The gate: make the framework sync machinery app-extensible over the entity kind
WITHOUT changing `SyncEntity`'s serialized JsonSchema (its variant names/order
drive `types.ts`'s `sync:<entity>` union; E8 demands `types.ts` byte-identical).

Three options considered:
- **(a)** Genericize the wire types: `SyncEvent<E>` / `SyncSseEvent<E>` in the
  framework, `type SyncEvent = …<SyncEntity>` in ziee. REJECTED — schemars names a
  generic instantiation `SyncEvent_for_SyncEntity`, renaming the schema → `types.ts`
  changes → E8 fails by design.
- **(b)** Keep the whole `sync/event.rs` (incl. registry) in ziee. REJECTED — the
  registry + audience + routing are the genuinely reusable core the plan moves.
- **(c)** Keep the wire types concrete in ziee; move only the machinery, made
  generic over a MINIMAL `SyncEntityKind` trait that abstracts the single wire
  event the registry itself synthesizes (the Session fan-out). CHOSEN.

**Resolution: (c).** `SyncEntityKind` has one method — `session_signal(user_id)
-> Event` — the only point the registry must build a wire event on its own. Every
other event enters the registry pre-serialized via `deliver(audience, Event, …)`,
so the framework never names `SyncEntity`/`SyncEvent`/`SyncSseEvent`, and those
schema-bearing types stay concrete-in-ziee, byte-unchanged. `SyncEntity` keeps its
enum + all 59 variants + `#[derive(JsonSchema)]` and merely gains
`impl SyncEntityKind`. Verified: `types.ts` (ui + desktop) BYTE-IDENTICAL;
`openapi.json` (ui + desktop) canonically-equal. The connection permission
snapshot is genericized over the ALREADY-extracted `Principal` (B1b) rather than a
second new trait, minimizing added surface.

## Decision — the process-wide singleton stays app-side

A generic `SyncRegistry<P>` cannot own a `lazy_static` process-global (it needs a
concrete `P`). ziee pins it: `lazy_static REGISTRY: SyncRegistry<SyncConnPrincipal>`
+ `registry()` live in ziee's `sync/registry.rs` shim.

**Resolution:** framework owns the registry TYPE + all logic + its unit tests; the
app owns the concrete GLOBAL instance + the concrete `SyncConnPrincipal`. This is
the same seam as B4 (framework owns the `declare_repositories!` generator; ziee
owns the generated `Repos` global) and B3 (framework enforces; ziee installs the
concrete resolver). No framework-facing global is introduced.

## Decision — EventBus split (B2 deferred `AppEvent`/`EventBus` "for B5")

B2 moved the domain-free `EventHandler` trait to the framework but left the
`AppEvent` enum + `EventBus` dispatcher app-side, noting B5 would revisit them.

**Resolution: EventBus STAYS app-side; only the SSE sync core moves.** The
`EventBus` dispatches over ziee's closed `AppEvent` enum (every variant wraps a
concrete ziee module event) — it is domain-coupled with no schema-neutral generic
form that carries its weight, and it is unrelated to the SSE realtime-sync
machinery this chunk is scoped to (the two share only the word "event"). Moving it
would mean either (a) a second erased-`dyn Any` dispatcher genericized over an app
event type — pure churn with no current second consumer, or (b) forcing every
`AppEvent` variant through the framework, re-coupling it to ziee's domain. Neither
is warranted; B5's mandate is the sync core + `SyncEntityKind`, which is complete.
The framework's `EventHandler` trait (B2) remains the only event-system symbol
there. "Move only what's genuinely generic" → the SSE registry/audience/extractor
moved; the domain-coupled `AppEvent`/`EventBus` did not.
