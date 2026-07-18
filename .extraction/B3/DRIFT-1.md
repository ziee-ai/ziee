# Chunk B3 — DRIFT scan (round 1)

Drift = any place the mechanical move + de-globalization could diverge from the
pre-extraction behavior/surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. `RequirePermissions` gained a generic param `R`,
  but ziee's `pub type RequirePermissions<Perms> =
  RequirePermissions<ZieeIdentityResolver, Perms>` keeps every call site
  (`RequirePermissions<(UsersRead,)>`, struct-pattern `{ user, groups, .. }`,
  and the `extractors::RequirePermissions` path in `project/handlers.rs`)
  compiling unchanged. Confirmed: `cargo check -p ziee` exit 0, no call-site edits.

- **DRIFT-1.2** — verdict: none. Admin-path group-load elision preserved. The
  framework calls `load_groups` ONLY on the non-admin branch (see TRANSFORMS
  Decision); an admin still gets `groups: vec![]` and issues no group query.
  Byte-identical control flow.

- **DRIFT-1.3** — verdict: none. `check_permission_union`'s `group.is_active`
  guard preserved by `ZieeIdentityResolver::active_group_permissions` returning
  `None` for inactive groups; `user_holds` is direct-first-then-active-groups,
  matching the original loop order exactly.

- **DRIFT-1.4** — verdict: none. 403 response schema unchanged. `PermissionError`
  / `PermissionErrorDetails` / `PermissionDetail` moved to the framework but keep
  their schemars short-names (crate path is not part of the schema key), and the
  403 is still registered via `with_permission`'s `response_with::<403,
  Json<PermissionError>>`. Verified: `types.ts` byte-identical + `openapi.json`
  canonically-equal on BOTH ui and desktop surfaces (E8).

- **DRIFT-1.5** — verdict: none. JWT layer untouched. `ZieeIdentityResolver::authenticate`
  still reads `Arc<JwtService>` from `parts.extensions`, so the existing
  `Extension(jwt_service)` layer at both boot sites is unchanged; only a new
  `Extension(Arc::new(ZieeIdentityResolver))` layer is added alongside it.

- **DRIFT-1.6** — verdict: move-fix. `RequireAdmin`'s former `#[allow(dead_code)]`
  would be spurious on a `pub` framework primitive but IS needed on ziee's re-export
  alias (no route uses it yet). Moved the `#[allow(dead_code)]` from the struct to
  ziee's `pub type RequireAdmin` alias; framework struct is `pub` (no lint). Verified:
  ziee builds with zero new warnings.

- **DRIFT-1.7** — verdict: move-fix. Re-exporting all three `PermissionError*`
  types from the ziee openapi shim tripped `-D unused-imports` (no app-side
  reference). Resolved with `#[allow(unused_imports)]` on the `pub use`, preserving
  the module's former public surface without the error. Verified: ziee builds clean.

- **DRIFT-1.8** — verdict: none. Both boot paths covered. `git grep` confirms the
  only two `.layer(Extension(jwt_service…))` install sites are `main.rs` (standalone)
  and `lib.rs::setup_server` (embedded/desktop, reached via `start_server_with_routes`);
  both received the resolver layer. Integration/e2e harnesses spawn one of these two
  binaries, so every runtime path installs the resolver.

**Unresolved drifts:** 0
