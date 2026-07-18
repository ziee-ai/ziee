# Chunk B1b — TRANSFORMS (every non-byte-identical change + rationale)

The moved code is byte-identical to its pre-extraction ziee form EXCEPT the
transforms below. `PermissionCheck`/`PermissionList`/`PermissionInfo` + all their
tuple impls and unit tests are a **verbatim** copy of `types.rs`; the
`check_permissions_array` body is a **verbatim** copy of the private fn from
`checker.rs`. `Principal` and `TokenVerifier` are NEW interfaces (no prior ziee
form to diff).

- **T-1** `check_permissions_array`: private `fn` in ziee `checker.rs` → `pub fn` in `ziee-identity::rbac`. — **why:** it must be callable across the crate boundary by the retained `check_permission_union` (which stays in ziee). The BODY (exact-match → full-`*` → hierarchical-prefix `foo::*` loop) is byte-identical; only the visibility keyword changed. The equivalence gate rests on this body being unchanged.

- **T-2** `check_permission_union`: retained in ziee `checker.rs`; its two internal calls now resolve to the moved `ziee_identity::check_permissions_array` via a module-level `use`. — **why:** `check_permission_union` operates on ziee's concrete `User`/`Group` (their `.permissions` / `.is_active`), so it CANNOT move; but the string-set matching it delegates to is generic and DID move. The delegation is call-for-call identical (same two call sites, same argument order), so every one of the ~20 `check_permission_union` call sites is unchanged and behavior is byte-identical. All 14 `checker.rs` tests stay in ziee and still exercise this path.

- **T-3** `PermissionInfo`: moved from `permissions/types.rs` into `ziee-identity::permission`, keeping its `#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]`. — **why:** `PermissionCheck::to_info()` (a default trait method) returns `PermissionInfo`, so the type is inseparable from the trait and must move with it. It is a projection DTO, NOT a table type, and it is NOT registered into the OpenAPI spec (0 occurrences in the baseline `openapi.json` + `types.ts`), so moving it — derives intact — leaves the generated `types.ts` byte-identical. `ziee-identity` gains `schemars`/`serde` deps (versions pinned to ziee's workspace catalog) to carry the derives.

- **T-4** `Principal` (NEW): a minimal authenticated-identity trait — `is_admin()`, `direct_permissions() -> &[String]`, `active_group_permissions() -> Vec<&[String]>` (default empty), and a provided `has_permission(&str)` that reuses `check_permissions_array`. — **why:** the pluggable-identity seam (decision #1). Framework enforcement needs exactly this interface; the default `has_permission` mirrors `check_permission_union`'s UNION semantics (direct OR any active-group set), and deliberately does NOT fold in `is_admin` (the admin short-circuit is a call-site concern in ziee: `user.is_admin || check_permission_union(..)`), keeping it a pure union check. `impl Principal for User` (in ziee's user module) exposes the user's direct permissions + admin flag.

- **T-5** `TokenVerifier` (NEW): a JWT-verify interface with associated `Claims`/`Error` types and `verify_access_token`/`verify_refresh_token`. — **why:** the interface must move (so framework enforcement depends on an abstraction) while the concrete `JwtService` — HMAC keys, issuer/audience, `jsonwebtoken` decode, leeway, `AppError` mapping — STAYS in ziee. Associated types keep `ziee-identity` free of any `jsonwebtoken`/`AppError`/config dependency. `impl TokenVerifier for JwtService` (in ziee's auth module) is a thin delegation to the existing `validate_access_token`/`validate_refresh_token` methods, which are unchanged.

- **T-6** `permissions/types.rs` shim: the file's 528 lines of trait+DTO+tests are replaced by `pub use ziee_identity::{PermissionCheck, PermissionInfo, PermissionList};`. — **why:** decision N2 — every `crate::modules::permissions::{PermissionCheck, PermissionList, PermissionInfo}` call site (via `mod.rs`'s re-export, `openapi.rs`, `user/permissions.rs`) resolves unchanged; the moved unit tests now run in `ziee-identity`.

## Decision

**Question:** Why is `Principal` genericized as (is_admin + direct + active-group)
rather than a single opaque `has_permission`, and why does `impl Principal for
User` expose an EMPTY active-group set when authorization today is group-aware?

**Resolution:** Framework enforcement needs to (a) run the UNION permission check
and (b) apply the admin short-circuit separately (ziee call sites do
`user.is_admin || check_permission_union(..)`). So `Principal` exposes both the
data (`direct_permissions` + `active_group_permissions`) and the `is_admin` flag,
with a provided `has_permission` that reuses the moved `check_permissions_array` so
wildcard/hierarchical semantics stay identical to `check_permission_union`. Groups
are threaded at ziee call sites TODAY via `check_permission_union(user, groups, ..)`
— the `User` value alone does not carry its groups — so `impl Principal for User`
correctly returns its direct permissions + admin flag and the default (empty)
active-group set; nothing calls `User::has_permission` yet (call sites are
unchanged this chunk, per task). The group dimension is wired into `Principal` when
the `RequirePermissions` extractor moves in **B3** (it constructs the Principal from
`(User, groups)` there). This is an equivalence-preserving genericization: the live
authorization path (`check_permission_union`) is byte-for-byte unchanged; the new
trait is additive and inert until B3.

Zero unresolved markers remain; every transform and interface carries a rationale.
