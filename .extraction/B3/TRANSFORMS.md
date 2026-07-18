# Chunk B3 — TRANSFORMS (every non-byte-identical change + rationale)

The moved code is byte-identical to its pre-extraction ziee form EXCEPT the
transforms below. `with_permission` + the three `PermissionError*` schema types
carry **verbatim** bodies (only the `PermissionList`/`PermissionCheck` import path
changed, `super::types` → `ziee_identity`). The extractor's enforcement algorithm
(header parse, token validate, admin short-circuit, missing-permission format, 403
codes/messages) is verbatim; only the identity/repository *access* is threaded
through the resolver.

- **T-1** `RequirePermissions` (`ziee-framework::permissions::extractors`): gains a
  leading generic param `R: IdentityResolver`; fields become `user: R::User`,
  `groups: Vec<R::Group>`; `_marker: PhantomData<Perms>` → `PhantomData<(R, Perms)>`.
  — **why:** the framework cannot name ziee's `User`/`Group`, and handlers consume
  `auth.user` (concrete `User` methods) + `auth.groups` pervasively, so the fields
  must stay concretely typed — achieved via the resolver's associated types. ziee
  re-fixes `R = ZieeIdentityResolver` in a type alias, so `RequirePermissions<(UsersRead,)>`
  is unchanged at all ~547 call sites.

- **T-2** `RequirePermissions::from_request_parts` (same file): the two global
  touch points are replaced by resolver calls — `extract_authenticated_user(parts)`
  → `resolver.authenticate(parts).await?`; `Repos.user.get_user_groups(user.id)` →
  `resolver.load_groups(&user).await?`; the inline
  `check_permission_union(&user,&groups,perm)` filter → `user_holds::<R>(&user,&groups,perm)`.
  The `Arc<R>` resolver is pulled from extensions via `get_resolver::<R>` at the top
  (replacing the former `parts.extensions.get::<Arc<JwtService>>()` — that lookup now
  lives inside ziee's `authenticate`). — **why:** de-globalize. Everything else in
  the fn (admin short-circuit returning empty `groups`, `Perms::permissions()`,
  the missing-permission `Vec<&str>` + singular/plural message, the
  `INSUFFICIENT_PERMISSIONS` 403) is verbatim.

- **T-3** `user_holds<R>` (new free fn, same file): the generic form of ziee's
  `check_permission_union` — `check_permissions_array(user.direct_permissions(), perm)`
  then, per group, `R::active_group_permissions(group)` (which returns `Some` only
  for active groups) → `check_permissions_array`. — **why:** the framework can't call
  ziee's concrete `check_permission_union` (which STAYS app-side, still used by 5
  other modules). The `Option`-returning `active_group_permissions` folds the
  `group.is_active` guard into the resolver so this generic loop is byte-equivalent
  (direct-first, then active groups). `is_admin` is intentionally NOT folded (a
  call-site short-circuit, exactly as before).

- **T-4** `RequireAdmin` (same file): gains `R: IdentityResolver`; field
  `user: User` → `user: R::User`; `extract_authenticated_user(parts)` →
  `get_resolver::<R>` + `resolver.authenticate(parts).await?`. The former
  `#[allow(dead_code)]` is dropped (it is now a `pub` framework primitive → no
  dead-code lint in a lib; ziee's re-export alias carries the `#[allow(dead_code)]`
  instead). — **why:** same de-globalization; `R` appears in the field type so no
  `PhantomData` is needed. The `ADMIN_REQUIRED` 403 + `is_admin()` check are verbatim.

- **T-5** `IdentityResolver` (new, `ziee-framework::permissions::resolver`): a new
  `#[async_trait]` trait with `type User: Principal`, `type Group`, and three methods
  `authenticate(&self, parts) -> Result<User, (StatusCode, AppError)>`,
  `load_groups(&self, user) -> Result<Vec<Group>, (StatusCode, AppError)>`,
  `active_group_permissions(group) -> Option<&[String]>`. — **why:** the injection
  seam. Split into `authenticate` + `load_groups` (not one `resolve`) to preserve
  ziee's exact behavior: an admin short-circuits BEFORE its groups are loaded (the
  former extractor returns empty `groups` for admins and never issues the group
  query). See `## Decision`.

- **T-6** `ZieeIdentityResolver` (ziee `modules/permissions/extractors.rs`, STAYS
  app-side): a zero-sized `#[derive(Clone, Copy, Default)]` unit implementing
  `IdentityResolver`. `authenticate` is the former `extract_authenticated_user`
  body verbatim (reads `Arc<JwtService>` from `parts.extensions`, `Repos.user.get_by_id`,
  active check); `load_groups` is the former `Repos.user.get_user_groups(user.id)`
  map_err verbatim; `active_group_permissions` = `if group.is_active { Some(&group.permissions) } else { None }`.
  — **why:** the concrete backing of the seam. The JWT service is still read from
  extensions inside `authenticate` (unchanged), so ziee's existing `Extension(jwt_service)`
  layer is untouched; only a new `Extension(Arc::new(ZieeIdentityResolver))` layer is added.

- **T-7** ziee startup (`server/src/lib.rs::setup_server`, `server/src/main.rs::main`):
  one `.layer(axum::Extension(Arc::new(ZieeIdentityResolver)))` added next to the
  existing `.layer(axum::Extension(jwt_service…))`. — **why:** install the injected
  resolver so `get_resolver::<ZieeIdentityResolver>` finds it. Both the standalone
  (main.rs) and embedded/desktop (lib.rs::setup_server) boot paths get it; these are
  the only two places the JWT layer is installed (all other JWT sites are consumers).

- **T-8** ziee `modules/permissions/openapi.rs`: whole file → a re-export shim of
  `with_permission` + the three `PermissionError*` types, `#[allow(unused_imports)]`.
  — **why:** N2 surface preservation. `with_permission` is used at ~530 sites via
  `mod.rs`'s `pub use openapi::with_permission`; the `PermissionError*` types have no
  app-side references (the 403 schema is now registered into the OpenAPI doc from
  inside the moved framework `with_permission`), hence the `allow`.

## Decision — one `resolve` vs split `authenticate`/`load_groups`

The former `RequirePermissions` extractor loads groups ONLY on the non-admin path:
an `is_admin` user returns `{ user, groups: vec![] }` and the group query is never
issued. A single `resolve() -> (user, groups)` method would eagerly load groups
for admins too, changing both the observable `auth.groups` value for admins (empty
→ populated) AND issuing an extra DB query — a behavioral (equivalence) regression.

**Resolution:** `IdentityResolver` exposes `authenticate` (→ user, no groups) and
`load_groups` (→ groups) as separate methods. The framework extractor calls
`authenticate`, applies the `is_admin` short-circuit returning `groups: vec![]`
with NO `load_groups` call, and only calls `load_groups` on the non-admin branch —
byte-identical control flow to the pre-extraction extractor. The `group.is_active`
filter is likewise preserved by `active_group_permissions` returning `None` for
inactive groups, so `user_holds` matches `check_permission_union` exactly.

## Decision — `Arc<R>` extension injection vs `Arc<dyn IdentityResolver>`

The task suggested `Arc<dyn IdentityResolver>`. But the extractor is necessarily
generic over `R` already (its `pub user`/`pub groups` fields are `R::User`/`R::Group`,
which a `dyn` object with associated types cannot express without erasing exactly
the concrete types handlers rely on).

**Resolution:** inject the resolver as a concrete `Arc<R>` in the axum request
extensions; the extractor pulls `parts.extensions.get::<Arc<R>>()` where `R` is
fixed to `ZieeIdentityResolver` by ziee's type alias. This is fully type-safe, keeps
handler field types concrete, and still satisfies the gate — the framework names
neither `Repos` nor `User`/`Group`, only the abstract `R` + its `Principal` bound.
A missing resolver → 500 (parallels the former "JWT service not configured").
