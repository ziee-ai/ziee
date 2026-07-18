# Chunk B5 — sync core + `SyncEntityKind` extensibility (CUT manifest)

Move the genuinely app-agnostic half of realtime sync — the per-user SSE
connection **registry**, the **audience** machinery, and the `SyncOrigin`
**extractor** — from ziee's `src-app/server/src/modules/sync/` into a new
`ziee_framework::sync` module, generic over an app **principal**
(`ziee_identity::Principal`) and an app **entity kind** (new `SyncEntityKind`
trait). ziee KEEPS its concrete, schema-bearing wire types + the process-wide
singleton and consumes the framework via re-export shims, so every `publish(...)`
emit site and the `SyncEntity` enum are byte-unchanged.

## Design gate — `SyncEntity` app-extensibility

The closed `SyncEntity` enum (59 variants, `#[derive(JsonSchema)]`, IN the
OpenAPI/`types.ts` surface — it drives the frontend `sync:<entity>` EventBus
vocabulary) must NOT change shape. The framework is made generic NOT by
genericizing `SyncEntity` (which would rename its schema) but by introducing a
minimal `SyncEntityKind` **trait** the registry's session fan-out is generic
over; `SyncEntity` stays a concrete ziee enum that IMPLEMENTS it. See TRANSFORMS
`## Decision` for the full rationale + the EventBus split.

## Files — MOVED INTO `ziee-framework` (submodule `sdk/`)

- new: `crates/ziee-framework/src/sync/mod.rs` — module doc + the `SyncEntityKind`
  trait (`fn session_signal(user_id) -> Event`) + re-exports.
- new: `crates/ziee-framework/src/sync/registry.rs` — `SyncRegistry<P: Principal>`,
  `ClientConn<P>`, the caps consts (`GLOBAL_MAX_CONNECTIONS` / `PER_USER_MAX_CONNECTIONS`
  / `SYNC_CHANNEL_CAPACITY`), `new`/`register`/`unregister`/`refresh`/`deliver`/
  `deliver_session_to_users::<E>`/`connection_count`, the shared `remove_conn`, and
  the moved routing unit tests (14 tests, re-expressed over a `TestPrincipal` +
  `TestEntity` + dummy `Event`s).
- new: `crates/ziee-framework/src/sync/audience.rs` — `Audience`, `PermRule`, and the
  `owner`/`everyone`/`perm::<P>`/`all_of::<L>`/`any_of::<L>` typed constructors + the
  2 moved constructor unit tests.
- new: `crates/ziee-framework/src/sync/extractor.rs` — `SyncOrigin` +
  `SYNC_CONNECTION_HEADER` (moved verbatim).
- edit: `crates/ziee-framework/src/lib.rs` — `pub mod sync;` + a crate-root
  `pub use sync::{...}`.
- edit: `crates/ziee-framework/Cargo.toml` — add `tokio` (feature `sync`) + `uuid`
  (the registry holds `mpsc::Sender`s + keys by `Uuid`).

## Symbols MOVED (framework-owned now)

`SyncRegistry` (now `<P>`), `ClientConn` (now `<P>`), `Audience`, `PermRule`,
`SyncOrigin`, `SYNC_CONNECTION_HEADER`, `SYNC_CHANNEL_CAPACITY`, the private caps
+ `remove_conn`. New: `SyncEntityKind` trait.

## Symbols that STAY in ziee (app-side — never moved)

- **Wire / schema types** (`sync/event.rs`, all `#[derive(JsonSchema)]`, all in the
  OpenAPI surface): `SyncEntity` (59 variants), `SyncAction`, `SyncEvent`,
  `SyncConnectedData`, `SyncSseEvent`. Byte-unchanged → `types.ts` byte-identical.
- **The singleton** `registry()` + its `lazy_static REGISTRY: SyncRegistry<SyncConnPrincipal>`
  (`sync/registry.rs`, now a shim). A generic registry can't own a monomorphized
  process-global; the app pins the concrete `P` (same pattern as B4's app-side `Repos`
  + B3's app-installed resolver).
- **The concrete principal** `SyncConnPrincipal { user: User, groups: Vec<Group> }` +
  its `Principal` impl (reproduces `check_permission_union` exactly).
- **`publish` / `publish_session_to_users`** (`sync/event.rs`) — thin wrappers that
  serialize the ziee wire event + call the singleton. Unchanged signatures.
- **The `impl SyncEntityKind for SyncEntity`** (`sync/event.rs`) — `session_signal`
  builds `SyncSseEvent::Sync(SyncEvent{ Session, Update, id })`, byte-identical to the
  former inline construction in `deliver_session_to_users`.
- **`handlers.rs`** (`GET /api/sync/subscribe`, JWT/Repos/re-check) — app-specific,
  stays; only the `ClientConn`/`refresh` construction is adapted to the new shape.
- **`mod.rs`** module registration + the `pub use event::{Audience, SyncAction,
  SyncEntity, publish, publish_session_to_users}` + `pub use extractor::SyncOrigin`
  re-exports — unchanged names, so all ~40 emit sites' imports are unchanged.

## EventBus (B2 deferred it "for B5")

Left app-side deliberately — see TRANSFORMS `## Decision — EventBus split`.

## Consequential edits OUTSIDE the sync module (2, both build-driven)

- `permissions/mod.rs` — drop `PermissionList` from the `pub use types::{…}`
  re-export (its only `permissions::`-path consumer was the audience constructors,
  now moved; lib-side flagged it unused).
- `permissions/types.rs` — split the `ziee_identity` re-export so `PermissionList`
  carries `#[allow(unused_imports)]` (its last by-name consumer in the `--bin`
  module tree moved; it stays re-exported for `crate::PermissionList` + the
  `server_update` `#[cfg(test)]` call site). See TRANSFORMS T-7/T-8.
