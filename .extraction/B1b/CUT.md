# Chunk B1b — `ziee-identity` abstractions (CUT manifest)

Move the identity **abstractions** (traits + generic RBAC evaluation) into
`sdk/crates/ziee-identity` (depends on `ziee-core`), consumed by ziee via
equivalence-preserving re-export shims + trait impls (decision N2). **No concrete
identity types move** — `User`/`Group`/`JwtService` + the auth tables STAY in ziee
and IMPLEMENT the moved traits (pluggable identity, decision #1). Framework
enforcement (the `RequirePermissions` extractor) STAYS in ziee this chunk; it moves
in B3.

## Files

Symbol-level relocations (source → SDK dest). Per decision **N2**, the ziee source
files are RETAINED as re-export shims / trait-impl hosts (the moved *definitions*
are deleted, not the files), so the whole-file `E6 source-absent` check is
intentionally waived — see `## Shims`.

- move: `src-app/server/src/modules/permissions/types.rs` → `sdk/crates/ziee-identity/src/permission.rs`
- move: `src-app/server/src/modules/permissions/checker.rs` → `sdk/crates/ziee-identity/src/rbac.rs`

New abstraction files introduced in the SDK (no direct ziee source — synthesized
interfaces the concrete ziee types implement):

- move: `sdk/crates/ziee-identity/src/principal.rs` → `sdk/crates/ziee-identity/src/principal.rs`
- move: `sdk/crates/ziee-identity/src/token.rs` → `sdk/crates/ziee-identity/src/token.rs`

## Symbols

Permission traits + projection (byte-preserved into `permission.rs`):
- symbol: `PermissionCheck` (sdk/crates/ziee-identity/src/permission.rs)
- symbol: `PermissionList` (sdk/crates/ziee-identity/src/permission.rs)
- symbol: `PermissionInfo` (sdk/crates/ziee-identity/src/permission.rs)

Generic RBAC evaluator (byte-preserved body into `rbac.rs`):
- symbol: `check_permissions_array` (sdk/crates/ziee-identity/src/rbac.rs)

New pluggable-identity interfaces:
- symbol: `Principal` (sdk/crates/ziee-identity/src/principal.rs)
- symbol: `TokenVerifier` (sdk/crates/ziee-identity/src/token.rs)

## Symbols that STAY in ziee (concrete identity — never moved)

- `User`, `Group` (concrete table models, `src/modules/user/models.rs`) — IMPLEMENT `Principal`
- `check_permission_union(user, groups, ..)` (concrete UNION over `User`/`Group`, `checker.rs`) — CALLS the moved `check_permissions_array`
- `JwtService`, `Claims`, `TokenPair`, … (concrete JWT, `src/modules/auth/jwt.rs`) — `JwtService` IMPLEMENTS `TokenVerifier`
- `RequirePermissions` extractor, `permissions/openapi.rs` (`with_permission`, `PermissionError*`) — stay; extractor moves in B3
- `all_permissions()` (`src/modules/user/permissions.rs`) — stays; consumes the re-exported `PermissionInfo`

## Shims (retained ziee files — decision N2)

- `src-app/server/src/modules/permissions/types.rs` → `pub use ziee_identity::{PermissionCheck, PermissionInfo, PermissionList};`
- `src-app/server/src/modules/permissions/checker.rs` → retains `check_permission_union` + all its tests; `use ziee_identity::check_permissions_array;` (moved definition deleted)
- `src-app/server/src/modules/user/models.rs` → adds `impl ziee_identity::Principal for User`
- `src-app/server/src/modules/auth/jwt.rs` → adds `impl ziee_identity::TokenVerifier for JwtService`
- `src-app/server/Cargo.toml` → adds `ziee-identity = { path = "../../sdk/crates/ziee-identity" }`

## Design-gate

**Pluggable identity (decision #1).** The framework must enforce authorization
against *abstractions*, so an app can inject its own identity. This chunk lands the
seam: `Principal` (the minimal authenticated-identity interface — is_admin /
direct permissions / active-group permissions) and `TokenVerifier` (token → claims)
are defined in `ziee-identity`; ziee's concrete `User`/`JwtService` implement them.
The `PermissionCheck`/`PermissionList` traits and the generic
`check_permissions_array` evaluator are framework-generic (they operate on
permission STRINGS, not on any table), so they move wholesale. `PermissionInfo` is
NOT part of the OpenAPI surface (0 occurrences in the baseline `openapi.json`/
`types.ts`), so moving it (with its `JsonSchema` derive intact) is E8-neutral —
B1b is equivalence-preserving with no schema/name movement (decision N2 satisfied).
