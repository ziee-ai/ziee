# Chunk B3 — permission enforcement (pluggable) + BG de-globalization (CUT manifest)

Move permission ENFORCEMENT — `with_permission` (`modules/permissions/openapi.rs`)
and the `RequirePermissions` / `RequireAdmin` extractors
(`modules/permissions/extractors.rs`) — into `sdk/crates/ziee-framework`
(`permissions` module), GENERIC over an injected `IdentityResolver` implementing
`ziee-identity`'s `Principal`. The framework never names ziee's global `Repos`,
`JwtService`, or concrete `User`/`Group`. This SUBSUMES chunk BG: JWT was
abstracted in B1b (`TokenVerifier`), config in B2 (`ServerConfig`); the only
remaining framework-facing global — `Repos`-for-permission-resolution — is
de-globalized here via the resolver seam. Consumed by ziee via
equivalence-preserving type-alias + re-export shims (decision N2).

## Files

The moved *definitions* land in new framework files; ziee's
`modules/permissions/{extractors,openapi}.rs` are RETAINED as shims (the resolver
impl + type aliases in extractors, a re-export in openapi).

- move: `src-app/server/src/modules/permissions/extractors.rs` → `sdk/crates/ziee-framework/src/permissions/extractors.rs`
- move: `src-app/server/src/modules/permissions/openapi.rs` → `sdk/crates/ziee-framework/src/permissions/openapi.rs`
- new:  `sdk/crates/ziee-framework/src/permissions/resolver.rs` (the `IdentityResolver` seam — the de-globalization primitive)
- new:  `sdk/crates/ziee-framework/src/permissions/mod.rs`

## Symbols

Enforcement extractors (byte-preserved logic into `ziee-framework::permissions::extractors`):
- symbol: `RequirePermissions` (sdk/crates/ziee-framework/src/permissions/extractors.rs) — now `RequirePermissions<R: IdentityResolver, Perms>`
- symbol: `RequireAdmin` (sdk/crates/ziee-framework/src/permissions/extractors.rs) — now `RequireAdmin<R: IdentityResolver>`
- symbol: `user_holds` (sdk/crates/ziee-framework/src/permissions/extractors.rs) — generic form of `check_permission_union`'s inline loop
- symbol: `get_resolver` (sdk/crates/ziee-framework/src/permissions/extractors.rs) — pulls `Arc<R>` from extensions

OpenAPI decoration (byte-preserved bodies into `ziee-framework::permissions::openapi`):
- symbol: `with_permission` (sdk/crates/ziee-framework/src/permissions/openapi.rs)
- symbol: `PermissionError` (sdk/crates/ziee-framework/src/permissions/openapi.rs)
- symbol: `PermissionErrorDetails` (sdk/crates/ziee-framework/src/permissions/openapi.rs)
- symbol: `PermissionDetail` (sdk/crates/ziee-framework/src/permissions/openapi.rs)

De-globalization seam (new, into `ziee-framework::permissions::resolver`):
- symbol: `IdentityResolver` (sdk/crates/ziee-framework/src/permissions/resolver.rs)

## Symbols that STAY in ziee (app-side — never moved)

- `ZieeIdentityResolver` (`modules/permissions/extractors.rs`) — ziee's concrete
  resolver: the former `extract_authenticated_user` body (JWT-from-extensions +
  `Repos.user.get_by_id` + active check) is its `authenticate`;
  `Repos.user.get_user_groups` is its `load_groups`; the `group.is_active` guard is
  its `active_group_permissions`.
- `check_permission_union` + `checker.rs` tests (`modules/permissions/checker.rs`)
  — the CONCRETE union over ziee's `User`/`Group`; still used by `sync/registry`,
  `control_mcp`, `hub`, `mcp`, `user` handlers. Not part of enforcement move.
- `all_permissions()` (`user/permissions.rs`) — the permission-STRING registry
  stays app-side (plan gate; the wired registry is introduced in a later chunk).
- The ~171 `Repos.xxx` call sites in ziee modules — unchanged (app's own global).
- `PermissionCheck` / `PermissionList` / `PermissionInfo` — already in
  `ziee-identity` (B1b); `check_permissions_array` — already in `ziee-identity` (B1b).

## Shims (retained ziee files — decision N2)

- `modules/permissions/extractors.rs` → defines `ZieeIdentityResolver` + `impl
  IdentityResolver for ZieeIdentityResolver` + `pub type RequirePermissions<Perms>
  = ziee_framework::permissions::RequirePermissions<ZieeIdentityResolver, Perms>` +
  `pub type RequireAdmin = …<ZieeIdentityResolver>`.
- `modules/permissions/openapi.rs` → `pub use
  ziee_framework::permissions::openapi::{with_permission, PermissionDetail,
  PermissionError, PermissionErrorDetails}` (`#[allow(unused_imports)]` — surface
  preservation; only `with_permission` is referenced app-side).
- `modules/permissions/mod.rs` → UNCHANGED (`pub use extractors::RequirePermissions;
  pub use openapi::with_permission;` now resolve to the aliases/re-export).
- `server/src/lib.rs` + `server/src/main.rs` → each add one
  `.layer(axum::Extension(Arc::new(ZieeIdentityResolver)))` next to the JWT layer,
  installing the resolver at startup.
- `crates/ziee-framework/Cargo.toml` → adds `serde` + `schemars` (403-body derives)
  + `serde_json` dev-dep (the moved `with_permission` test).

## Design-gate

**Injected identity resolver.** The framework's enforcement layer must not name
ziee's global `Repos`/`JwtService` or concrete `User`/`Group`. It is generic over
`IdentityResolver` (associated `User: Principal` + `Group`), pulled from the axum
request extensions as `Arc<R>`; ziee installs `Arc<ZieeIdentityResolver>` at
startup. The extractor output keeps its `pub user` / `pub groups` fields (typed
`R::User` / `Vec<R::Group>`), so every one of the ~547 `RequirePermissions<(…,)>`
call sites — and every `auth.user` / `auth.groups` handler access — is byte-for-byte
unchanged behind ziee's type alias. The enforcement algorithm (JWT → user →
`is_admin` short-circuit BEFORE loading groups → group UNION with `is_active`
guard → ALL-of AND logic → `INSUFFICIENT_PERMISSIONS` 403) is byte-identical to
the former extractor. The `PermissionError` 403 schema keeps its schemars name
(short-name keyed, not crate-path), so both OpenAPI surfaces are E8-neutral.
