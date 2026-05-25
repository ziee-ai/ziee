# Security Audit — User Module
**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/user/` (~2,026 LOC)
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Chapters emphasised:** V4 (Access Control), V5 (Validation), V8 (Data Protection), V13 (API)

---

## Executive Summary

The user module exposes 16 endpoints that manage users and groups behind the `RequirePermissions<…>` extractor. The extractor itself is sound (JWT verify → active check → admin bypass → group load → union check) and the repository is parameterised SQLx everywhere (no string interpolation, no `sqlx::query` raw text). However, **the request DTOs and business rules above the repository layer leak a large amount of authority to anyone holding the `users::edit`, `groups::edit`, or `groups::assign_users` permissions**, and the default Users group does not have those permissions only by convention — there is no defence-in-depth that would catch a misconfigured deployment.

The single most serious problem is that **any holder of `users::edit` may grant arbitrary permissions to any user — including the wildcard `*` — via the `permissions` field of `UpdateUserRequest`**, and any holder of `groups::edit` may do the same to any group, including the system "Administrators" group, whose protection in `update_group` covers only `name` and `is_active` but **not `permissions`**. The third structural escalation path is `assign_user_to_group`, which performs no check on the group being assigned to: assigning oneself (or anyone) to the "Administrators" group is allowed if the caller has `groups::assign_users`. These three primitives, exposed to anyone the operator gives the corresponding permissions to (e.g., a "user manager" sub-admin role), each elevate the holder to full root admin.

The `delete_user` handler is also missing the "cannot touch root admin" protection that `update_user` and `toggle_user_active` enforce — a user with `users::delete` can `DELETE /users/{admin_id}`, removing the partial-unique-index root admin and leaving the deployment with no admin account.

Other findings of note:

- `PaginationQuery` has no bounds on `page` or `per_page` (negative values cause negative SQL `OFFSET`/`LIMIT` errors; huge values DoS the listing endpoints and dump arbitrary slices of the user table).
- `reset_user_password` and `create_user` accept any password (zero-length, "a", `\0`); no minimum length, no complexity, no breached-password check.
- Email change requires no email-confirmation token, so an admin (or any holder of `users::edit`) silently rewrites another user's email to a value they control, enabling OAuth-by-email account takeover at the next federated sign-in, and standard "forgot password" recovery if added later.
- Repository database errors are returned to clients with `format!("Database error: {}", err)` — the raw SQLx error string, including driver-level details, reaches the JSON response body.
- No CSRF protection on state-changing endpoints (this is mitigated only because the auth model uses bearer JWT, but the previous audit (`01-auth-user-permissions-audit.md` M-04) flagged this for the larger surface).
- No rate-limiting on `reset_user_password` or `create_user` — useful to a malicious low-privilege admin.

Counts: **2 Critical, 4 High, 7 Medium, 5 Low, 4 Info.**

Top 3 risks:
1. **F-01 (Critical)** — Privilege escalation to root admin via `UpdateUserRequest.permissions`, `UpdateGroupRequest.permissions`, or `AssignUserToGroupRequest` (three independent paths).
2. **F-02 (Critical)** — Root-admin account can be hard-deleted by anyone with `users::delete`, since `delete_user` has no `is_admin` guard.
3. **F-03 (High)** — Silent email rewrite without confirmation token enables account takeover via federated identity and any future password-reset-by-email flow.

---

## Findings

### F-01 — Permission grant / group permission grant / group membership tampering allow self-elevation to root

- **Severity:** Critical
- **ASVS:** V4.1.3, V4.1.5, V4.2.1, V4.2.2
- **CWE:** CWE-269 (Improper Privilege Management), CWE-272 (Least Privilege Violation)
- **Location:**
  - `src-app/server/src/modules/user/handlers/user.rs:164-230` (`update_user`)
  - `src-app/server/src/modules/user/handlers/groups.rs:127-174` (`update_group`)
  - `src-app/server/src/modules/user/handlers/groups.rs:269-292` (`assign_user_to_group`)
  - `src-app/server/src/modules/user/types.rs:22-29, 38-44, 80-84` (DTOs)
- **Description:**

Three independent paths let a non-root caller obtain full root-admin authority. Each requires only a single non-`*` permission that operators are likely to grant to a "user manager" or "group manager" sub-admin:

1. **`users::edit` + `UpdateUserRequest.permissions`** — `UpdateUserRequest` accepts `permissions: Option<Vec<String>>` (no allow-list, no validation, no "you cannot grant more than you have" check). The repository `update()` writes the array verbatim via `permissions = COALESCE($5, permissions)`. So a user with `users::edit` POSTs `{ "permissions": ["*"] }` to `/users/{anyone_id}` and that target — including the caller themselves — now has the universal wildcard. The `check_permissions_array` function in `modules/permissions/checker.rs:38` recognises `"*"` as full bypass. The `is_admin` root flag remains false, but every permission check thereafter passes.

2. **`groups::edit` + `UpdateGroupRequest.permissions`** — `update_group` (handlers/groups.rs:128) blocks `name` changes and `is_active=false` on system groups (line 142-149) but **does not block `permissions` changes on system groups**. A user with `groups::edit` POSTs `{ "permissions": ["*"] }` to `/groups/{users_group_id}` and now every regular user — including themselves — inherits `*`. They can also overwrite the "Administrators" group's permission list to remove admin authority, or change the default group to elevate any new sign-up.

3. **`groups::assign_users` + `AssignUserToGroupRequest`** — `assign_user_to_group` (handlers/groups.rs:270) checks only that the user and group exist. It does not check whether the target group is `is_system` or whether its permission list contains `*`. A user with `groups::assign_users` POSTs `{ "user_id": "<self>", "group_id": "<administrators_id>" }` and now `check_permission_union` sees `permissions: ["*"]` from the Administrators group — same effect as wildcard. Note the `is_admin` root-admin bypass still doesn't fire (that's gated on the `users.is_admin` column), but `*` from the group makes that immaterial.

- **Vulnerable code (path 1):**
```rust
// handlers/user.rs:202-212
Repos
    .user
    .update(
        user_id,
        request.username,
        request.email,
        request.display_name,
        request.permissions,    // ← accepted verbatim, no allow-list
    )
    .await?;
```

```rust
// repository.rs:207-227
sqlx::query_as!(
    User,
    r#"
    UPDATE users
    SET username = COALESCE($2, username),
        email = COALESCE($3, email),
        display_name = COALESCE($4, display_name),
        permissions = COALESCE($5, permissions),    // ← writes ["*"] directly
        updated_at = NOW()
    WHERE id = $1
    RETURNING …
    "#,
    id, username, email, display_name, permissions.as_deref()
)
```

- **Vulnerable code (path 2):**
```rust
// handlers/groups.rs:141-150  (update_group)
if existing_group.is_system {
    if request.name.is_some() || request.is_active == Some(false) {
        return Err(AppError::bad_request("SYSTEM_GROUP", …).into());
    }
}
// note: request.permissions falls through unchecked
```

- **Vulnerable code (path 3):**
```rust
// handlers/groups.rs:270-292  (assign_user_to_group)
pub async fn assign_user_to_group(
    auth: RequirePermissions<(GroupsAssignUsers,)>,
    Json(request): Json<AssignUserToGroupRequest>,
) -> ApiResult<StatusCode> {
    if Repos.user.get_by_id(request.user_id).await?.is_none() { … }
    if Repos.group.get_by_id(request.group_id).await?.is_none() { … }
    // ← no is_system / no "you cannot assign to higher-privileged group" check
    Repos.user.assign_to_group(request.user_id, request.group_id, Some(auth.user.id)).await?;
    …
}
```

- **Exploitation:**

Attacker has `users::edit` (granted by a misguided "user manager" role). Attacker calls:

```http
POST /users/<own_id> HTTP/1.1
Authorization: Bearer <attacker_jwt>
Content-Type: application/json

{ "permissions": ["*"] }
```

Response is `200 OK` with the updated user object. On the next request the attacker's `RequirePermissions<…>` pass for every permission in the system. The same payload works against any other user, including the root admin (whose `is_admin` flag is preserved but who now has duplicate `*`, which is harmless to them but lets the attacker pose as the admin in any non-admin-only flow).

Analogously for paths 2 and 3.

- **Impact:** Full system compromise from a single low-privilege permission. Affects every other module's data through `RequirePermissions`.

- **Recommendation:**

  1. **Remove `permissions` from `UpdateUserRequest` and `CreateUserRequest` entirely.** Direct user-level permissions are an exotic capability that should be gated behind a dedicated permission, e.g., `users::set_direct_permissions`, available only to root admin (use `RequireAdmin`, not `RequirePermissions`). Add `#[serde(deny_unknown_fields)]` to both DTOs so submitting `permissions` becomes a 400, not a silently-ignored field.

  2. **Add explicit `permissions` and `name` immutability for `is_system` groups** in `update_group`. The current block already special-cases system groups; extend it:
     ```rust
     if existing_group.is_system {
         if request.name.is_some()
             || request.is_active == Some(false)
             || request.permissions.is_some() {
             return Err(AppError::bad_request("SYSTEM_GROUP", …).into());
         }
     }
     ```
     Better: gate group-permission edits behind a dedicated `groups::set_permissions` permission, default-granted only to root admin.

  3. **Refuse to assign any user to an `is_system` group at runtime** (the only valid path is the seeded default-group assignment in `create_user`, which already bypasses the handler). Add to `assign_user_to_group`:
     ```rust
     let target_group = Repos.group.get_by_id(request.group_id).await?.unwrap();
     if target_group.is_system {
         return Err(AppError::forbidden("SYSTEM_GROUP_PROTECTED",
             "Cannot assign users to system groups via the API").into());
     }
     ```
     Or: require root-admin for any assignment to a group whose permissions contain `*` or `<resource>::*`.

  4. **Enforce a "no-elevation" invariant** in update paths: the caller may not grant permissions they do not themselves hold (other than through their own root-admin status). Pseudocode:
     ```rust
     let caller_perms = effective_permissions(&auth.user, &auth.groups);
     for granted in request.permissions.iter().flatten().flatten() {
         if !caller_perms.contains(granted) && !auth.user.is_admin {
             return Err(AppError::forbidden("CANNOT_GRANT_HIGHER_PERMISSION", …).into());
         }
     }
     ```

---

### F-02 — Root admin can be hard-deleted by anyone with `users::delete`

- **Severity:** Critical
- **ASVS:** V4.2.1, V4.2.2
- **CWE:** CWE-285 (Improper Authorization)
- **Location:** `src-app/server/src/modules/user/handlers/user.rs:339-358` (`delete_user`)
- **Description:**

`toggle_user_active` (line 261-265) and `update_user` (line 177-182) both block disabling an admin user with the `CANNOT_DISABLE_ADMIN` error. `delete_user` performs **no such check** — it loads the user only to verify existence and then issues `DELETE FROM users WHERE id = $1`. The DB schema has `CREATE UNIQUE INDEX unique_root_admin ON users (is_admin) WHERE is_admin = true` (migration 1, line 32), so there is exactly one root admin; deleting them removes the only account holding the root-admin authority and the partial-unique index now permits a new root admin to be created — but the only callers who could promote a new admin would themselves need to be admin, creating a lockout.

- **Vulnerable code:**
```rust
// handlers/user.rs:339-358
pub async fn delete_user(
    _auth: RequirePermissions<(UsersDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    if Repos.user.get_by_id(user_id).await?.is_none() {
        return Err(AppError::not_found("User").into());
    }
    Repos.user.delete(user_id).await?;     // ← no is_admin guard
    event_bus.emit_async(UserEvent::deleted(user_id));
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}
```

- **Exploitation:**

```http
DELETE /users/<root_admin_id> HTTP/1.1
Authorization: Bearer <attacker_jwt>   # holds users::delete
```

Returns 204; root admin row gone. All `RequireAdmin`-gated routes are now unreachable for everyone. Combined with F-01, the attacker re-promotes themselves via a direct DB mutation only — without DB access, the deployment is bricked. With DB access, the attacker becomes the new root admin.

There is also a cascade concern: `user_groups` has `ON DELETE CASCADE` for `user_id`, so memberships are dropped, but `user_groups.assigned_by` has plain `REFERENCES users(id)` with no ON DELETE — deleting a user that has assigned others will FK-violate (sqlx returns the error to the client; on the client side it surfaces as a 500 with the raw error text per F-08).

- **Impact:** Loss of admin authority, deployment lockout, plus an information-disclosure path via the FK error string.

- **Recommendation:**

  1. Add the is_admin guard, mirroring `toggle_user_active`:
     ```rust
     let user = Repos.user.get_by_id(user_id).await?
         .ok_or_else(|| AppError::not_found("User"))?;
     if user.is_admin {
         return Err(AppError::bad_request("CANNOT_DELETE_ADMIN",
             "Cannot delete admin users").into());
     }
     ```
  2. Prefer **soft delete** (set `is_active = false`, perhaps anonymise `email`/`username` to `deleted-<uuid>@invalid`) so the FK chain stays intact and a future investigator can still inspect who assigned what.
  3. Set `ON DELETE SET NULL` on `user_groups.assigned_by` (migration) so the cascade story is at least clean.

---

### F-03 — Email rewrite without re-verification enables federated account takeover

- **Severity:** High
- **ASVS:** V3.6.1, V4.2.1
- **CWE:** CWE-287 (Improper Authentication), CWE-640 (Weak Password Recovery)
- **Location:** `src-app/server/src/modules/user/handlers/user.rs:193-200` (`update_user`)
- **Description:**

`UpdateUserRequest.email` is `Option<String>`. If set, `update_user` checks uniqueness and writes the new value with no email-confirmation token, no notification to the old address, and no `email_verified = false` reset. The schema column `email_verified` (migration 1, line 13) exists but **is not touched on email change**.

This is dangerous on two axes:

1. **Federated identity matching.** The auth module's OAuth/SAML flows typically match incoming identities by email. A user with `users::edit` (admin or sub-admin) rewrites their own email — or that of a target — to a value they control. On the next OAuth callback that arrives with that email, `user_auth_links` will either create a link to the now-existing user or, depending on the matching logic, log them in directly.

2. **Future password-reset flows.** Once a "forgot password" feature is added (currently absent), the new attacker-controlled email is the recovery channel for the victim's account.

Sibling concerns:
- **No session invalidation on email change.** Active JWTs continue working. Refresh tokens — if rotation is added later — also continue. (Existing audit HIGH-03 covers the larger JWT-revocation gap.)
- **No session invalidation on password reset.** `reset_user_password` writes a new `password_hash` but does not touch sessions. The axum-login session would invalidate because `session_auth_hash()` returns `password_hash.as_bytes()` (models.rs:43-50), **but the JWT layer (which is what actually authorises requests in this module) is independent of axum-login sessions** — the JWT lives until expiry.

- **Vulnerable code:**
```rust
// handlers/user.rs:193-212
if let Some(ref email) = request.email {
    if let Some(existing) = Repos.user.get_by_email(email).await? {
        if existing.id != user_id {
            return Err(AppError::conflict("Email").into());
        }
    }
}
// ← no token check, no notification, no email_verified reset
Repos.user.update(user_id, request.username, request.email, …).await?;
```

- **Exploitation:**
1. Attacker (an internal "user manager" with `users::edit`) POSTs `{ "email": "victim@target.com" }` to their own `/users/<self_id>`.
2. Returns 200, the row's `email = victim@target.com`, `email_verified = true` (carried over).
3. Attacker waits for any "Sign in with Google" event from `victim@target.com`; the OAuth code path looks up by email and links to the attacker's user.

- **Impact:** Pre-auth account takeover via federated identity; future-proof attack against any email-based recovery flow.

- **Recommendation:**

  1. **Two-step email change with confirmation token.** Store the new email in a `pending_email` column (or separate `email_change_tokens` table), send a verification link to the **new** address, notify the **old** address that a change is in flight. Only swap on token redemption.
  2. **Reset `email_verified = false`** on any email change, even admin-initiated, and re-issue verification.
  3. **Invalidate all active sessions and JWTs** for the affected user on email or password change. The current model has no JWT revocation list (existing audit HIGH-03); minimum interim measure is to bump a per-user `session_version` and embed it in the JWT, refusing tokens with stale version.
  4. **Restrict email change to root admin** until #1/#2 ship — remove `email` from `UpdateUserRequest` for `users::edit` callers and add a separate `RequireAdmin`-gated handler.

---

### F-04 — `permissions` allowed in `CreateUserRequest` lets `users::create` grant `*` at sign-up

- **Severity:** High
- **ASVS:** V4.1.5, V4.2.1
- **CWE:** CWE-269
- **Location:** `src-app/server/src/modules/user/handlers/user.rs:88-133`, `types.rs:13-20`
- **Description:**

`CreateUserRequest.permissions: Option<Vec<String>>` is plumbed straight into `Repos.user.create(…)` (repository.rs:131-157), which writes it to `users.permissions`. A user with `users::create` (a plausible "HR onboarding" role) can create an account holding any permission they choose — including `*` — and use it directly, or hand over the credentials to a collaborator.

This is the same root cause as F-01 path 1 but on the create surface, and arguably worse because the actor never needs to update an existing row (and so cannot be caught by audit on existing-user changes).

- **Vulnerable code:**
```rust
// handlers/user.rs:124-133
let user = Repos.user.create(
    &request.username,
    &request.email,
    Some(password_hash),
    request.display_name,
    request.permissions,    // ← straight to DB
).await?;
```

- **Exploitation:** As F-01 but via POST `/users` with `{"permissions": ["*"], "username": "...", "email": "...", "password": "..."}`. New user is created with `*`; attacker logs in as that user.

- **Impact:** Privilege escalation, audit-evasion (no row-update event).

- **Recommendation:** Same as F-01 #1 — strip `permissions` from `CreateUserRequest` and use a dedicated `RequireAdmin` endpoint for direct permission grants. Add `#[serde(deny_unknown_fields)]`.

---

### F-05 — No password strength enforcement on `create_user` or `reset_user_password`

- **Severity:** High
- **ASVS:** V2.1.1 (min length 12), V2.1.7 (breached-password check)
- **CWE:** CWE-521 (Weak Password Requirements)
- **Location:**
  - `src-app/server/src/modules/user/handlers/user.rs:90-133` (`create_user`)
  - `src-app/server/src/modules/user/handlers/user.rs:304-326` (`reset_user_password`)
- **Description:**

Both handlers accept arbitrary password strings — including the empty string (an admin can issue `{"new_password": ""}` to `/users/reset-password`, which bcrypt will hash without complaint) and single-character passwords. There is no minimum length, no maximum length (bcrypt silently truncates beyond 72 bytes), no character-class check, and no breached-password lookup. The existing audit (`01-auth-user-permissions-audit.md` HIGH-04) flagged this for the auth module's registration endpoint; the same gap exists on the user-module admin paths.

The bcrypt cost is the crate default (`bcrypt::DEFAULT_COST = 12` at the time of writing). The existing audit's LOW-07 recommends 14 for ASVS L2.

- **Vulnerable code:**
```rust
// handlers/user.rs:316-323
let password_hash = bcrypt::hash(&request.new_password, bcrypt::DEFAULT_COST)
    .map_err(…)?;
Repos.user.update_password(request.user_id, &password_hash).await?;
```

- **Exploitation:** Sub-admin issues `/users/reset-password` with `new_password=""`; victim's password is now the empty string; sub-admin signs in as victim.

- **Impact:** Mass account compromise inside a deployment; downstream credential-stuffing risk.

- **Recommendation:** Centralise password policy in a `validate_password(&str) -> Result<(), AppError>` helper (auth module would also use it) enforcing min length 12 (ASVS L2), max 1024 bytes pre-truncation guard, optional zxcvbn score ≥ 3 or HaveIBeenPwned k-anonymity check. Make `bcrypt::DEFAULT_COST` configurable via the config file and default to 14.

---

### F-06 — `PaginationQuery` has no bounds — DoS via huge `per_page`, panic via negative values

- **Severity:** High
- **ASVS:** V13.1.3, V12.1.1 (resource limits)
- **CWE:** CWE-770 (Allocation of Resources Without Limits)
- **Location:**
  - `src-app/server/src/common/type.rs:178-193`
  - `src-app/server/src/modules/user/handlers/user.rs:32-50` (`list_users`)
  - `src-app/server/src/modules/user/handlers/groups.rs:32-50, 226-255`
  - `src-app/server/src/modules/user/repository.rs:99-128, 487-516, 589-634`
- **Description:**

`PaginationQuery` is defined as `{ page: i32, per_page: i32 }` with default `1`/`20` and no validation. Three concrete failure modes:

1. **Huge `per_page`.** `GET /users?per_page=2147483647` issues `LIMIT 2147483647 OFFSET 0` to Postgres. The query is parameterised so no injection, but the response materialises the entire user table in one allocation, plus serialises every row to JSON. This is a single-request DoS and a bulk PII exfiltration enabler (every user's email, last_login_at, permissions for the listing user with `users::read`).

2. **Negative values.** `GET /users?page=-1&per_page=10` computes `offset = ((-1 - 1) * 10) as i64 = -20`. Postgres rejects negative `OFFSET` with an error; the error flows back through `database_error()` and is surfaced verbatim to the client (F-08), revealing the literal SQL behaviour. Same for `per_page=0` causing division-by-zero in `total_pages` calc:
   ```rust
   let total_pages = (total + params.per_page as i64 - 1) / params.per_page as i64;
   ```
   With `per_page=0`, this panics on integer division by zero (Rust integer division panics in debug, wraps in release — but on the negative-overflow path `(total + -1) / -1` is also `i64::MIN` → potential overflow). In release builds the math succeeds with garbage; in debug builds the worker panics.

3. **Math correctness.** `total_pages` calculation uses `params.per_page as i64`; an attacker who supplies a perfectly chosen value can trigger arithmetic overflow on `total + per_page - 1` for huge totals (not exploitable today but a latent footgun).

- **Vulnerable code:**
```rust
// repository.rs:99-122
pub async fn list(&self, page: i32, per_page: i32) -> Result<(Vec<User>, i64), AppError> {
    let offset = ((page - 1) * per_page) as i64;   // ← signed arithmetic, no bounds
    …
    .bind(per_page as i64).bind(offset)            // ← negatives flow through
}
```

```rust
// handlers/user.rs:38
let total_pages = (total + params.per_page as i64 - 1) / params.per_page as i64;
// ← divide-by-zero if per_page == 0; overflow if total close to i64::MAX
```

- **Exploitation:**
  - `GET /users?per_page=2000000000` → 500 (Postgres might also OOM the worker depending on dataset; in any case bulk PII dumped).
  - `GET /users?per_page=0` → division-by-zero panic in debug builds; arithmetic wraps in release.
  - `GET /users?page=-1` → DB error string leaked.

- **Impact:** Service availability, PII dump, error-string leakage.

- **Recommendation:** Replace `PaginationQuery` with validated wrapper. Either implement `serde(deserialize_with)` to clamp:
  ```rust
  fn validate_per_page(value: i32) -> i32 {
      value.clamp(1, 100)
  }
  ```
  Or use the `validator` crate (`#[validate(range(min = 1, max = 100))]` on `per_page`, `#[validate(range(min = 1))]` on `page`) and add a `WithRejection<Query<…>>` extractor that runs validation. Guard the total_pages math:
  ```rust
  let total_pages = total.saturating_add(per_page.saturating_sub(1)) / per_page.max(1);
  ```

---

### F-07 — Group permission update on system groups + no membership cap allow lock-in attacks

- **Severity:** Medium
- **ASVS:** V4.3.1, V4.3.2
- **CWE:** CWE-269, CWE-770
- **Location:** `src-app/server/src/modules/user/handlers/groups.rs:128-174` (`update_group`)
- **Description:**

Even if F-01 path 2 is fixed (blocking permissions edits on `is_system`), the current handler also has no protection against:

- Modifying the `is_default` group's permissions (cascades to every new sign-up).
- Removing all members from the Administrators group via repeated `remove_user_from_group` calls (no "at least one admin must remain" invariant). The root-admin (`is_admin = true`) is independent of the Administrators group membership, so this is not a full lockout — but it does silently revoke `*` from anyone the operator promoted via group rather than the root flag.
- Setting `is_active = false` on the default Users group through anyone with `groups::edit` (currently blocked only for system groups, but a non-system "default" group could be deactivated). Inactive groups are skipped in permission checks (`check_permission_union` line 16-18) so this disables every regular user.

- **Exploitation:** With `groups::edit`, change the default Users group's `is_default = false` is not possible via `UpdateGroupRequest` (the field is not exposed — good), but `is_active = false` works on a non-system default. With `groups::assign_users`, evict every administrator-equivalent user from the Administrators-like group.

- **Impact:** Denial of service, deactivation of permissioning fabric.

- **Recommendation:**
  - Refuse to deactivate any group with `is_default = true`.
  - Refuse to remove the last member from a group whose `permissions` contain `*` or a top-level wildcard.
  - Add an `Administrators`-membership invariant: at least one user with `is_admin = true` OR membership in a `*`-permission group must exist; check on every `delete_user`, `remove_user_from_group`, `set_active(false)`, `update_user(is_active=false)`.

---

### F-08 — Raw SQL error text leaked to clients via `AppError::database_error`

- **Severity:** Medium
- **ASVS:** V7.4.1 (sanitise error messages), V8.3.4
- **CWE:** CWE-209 (Information Exposure Through Error Message)
- **Location:** `src-app/server/src/common/type.rs:109-115`, used throughout `repository.rs`
- **Description:**

Every repository error is wrapped via:
```rust
pub fn database_error(err: impl std::error::Error) -> Self {
    Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "SYSTEM_DATABASE_ERROR",
        format!("Database error: {}", err),    // ← raw error string
    )
}
```

The full sqlx::Error `Display` impl includes the Postgres error text — table names, column names, constraint names, the literal value that triggered a unique-violation, FK detail. Through `IntoResponse for AppError` this string lands in the JSON body's `error` field and is returned to the client.

Specific paths in the user module where this fires for unauthenticated- or low-privilege-reachable inputs:

- Unique-violation on `username`/`email` (race condition between the pre-check and the INSERT in `create_user`): Postgres returns `duplicate key value violates unique constraint "users_email_key"`.
- FK violation on `assigned_by` after F-02-style admin deletion: `update or delete on table "users" violates foreign key constraint "user_groups_assigned_by_fkey"` — leaks both table and column.
- Type-mismatch errors from negative `OFFSET`/`LIMIT` (F-06).
- VARCHAR length-overflow errors when an attacker submits a 300-char username (DB column is `VARCHAR(100)`).

- **Exploitation:**
```bash
curl -X POST /users \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"username": "'"$(python3 -c 'print("a"*300)')"'", "email": "x@x", "password": "p"}'
```
Returns 500 with body containing `Database error: error returned from database: value too long for type character varying(100)`.

- **Impact:** Reveals schema (table names, column names, constraint names), assists in fingerprinting Postgres version, gives an attacker a free SQLi-like oracle without an actual injection.

- **Recommendation:**
  - Return a generic message to clients: `AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "SYSTEM_DATABASE_ERROR", "An internal database error occurred")`.
  - Log the full error via `tracing::error!("database error: {:?}", err)` server-side with a request-correlation ID.
  - Translate known kinds (`sqlx::Error::Database(e)` where `e.is_unique_violation()`) to specific 409s with a sanitised message — see F-09.

---

### F-09 — User enumeration via uniqueness errors and create-conflict responses

- **Severity:** Medium
- **ASVS:** V3.2.2 (no username enumeration), V7.1.1
- **CWE:** CWE-204 (Response Discrepancy)
- **Location:**
  - `src-app/server/src/modules/user/handlers/user.rs:104-117` (`create_user`)
  - `src-app/server/src/modules/user/handlers/user.rs:184-200` (`update_user`)
- **Description:**

`create_user` performs separate pre-checks for username and email duplication and returns `AppError::conflict("Username")` vs `AppError::conflict("Email")`. The discriminated responses let a caller iterate emails or usernames and learn which exist.

This is gated by `users::create` permission, so it's not anonymous enumeration — but the previous audit (`01-auth-user-permissions-audit.md` MEDIUM-01) flagged the same pattern on the public registration endpoint. The user-module variant should be sanitised for consistency, especially because the same caller already has `users::read` 9 times out of 10 (the listing already gives them email enumeration directly — see F-10).

Additionally, the two pre-checks are not atomic with the INSERT: between the SELECT and the INSERT, a concurrent request can race and trigger the unique constraint at the DB level, returning the F-08-leaked error instead of the friendly conflict.

- **Vulnerable code:**
```rust
// handlers/user.rs:105-117
if Repos.user.get_by_username(&request.username).await?.is_some() {
    return Err(AppError::conflict("Username").into());
}
if Repos.user.get_by_email(&request.email).await?.is_some() {
    return Err(AppError::conflict("Email").into());
}
```

- **Exploitation:** Run a brute-force username/email check using `POST /users` with a randomly-named throwaway entry; observe whether the 409 says "Username already exists" or "Email already exists" to identify which field collided.

- **Recommendation:**
  - Collapse to a single generic conflict: `AppError::conflict("Account")` with body `{"error": "An account with the supplied identifiers already exists"}`.
  - Trap unique-violation DB errors and re-emit the same generic conflict (closing the race-and-leak path).

---

### F-10 — `list_users` and `get_user` return PII (email, last_login_at, direct permissions) to any holder of `users::read`

- **Severity:** Medium
- **ASVS:** V8.1.1, V8.3.1 (minimise PII exposure)
- **CWE:** CWE-359 (Exposure of Private Information)
- **Location:**
  - `src-app/server/src/modules/user/handlers/user.rs:31-50, 63-75`
  - `src-app/server/src/modules/user/models.rs:14-34`
- **Description:**

`User` is `#[derive(Serialize)]` and only `password_hash` is `#[serde(skip_serializing)]`. Every other field — `email`, `email_verified`, `is_admin`, `permissions`, `last_login_at` — flows to the client.

`users::read` is granted to plausible roles (helpdesk, user manager). The default Users group does **not** have it (verified in migrations), so this is not a direct user-to-user leak by default; it's a moderate-privilege escalation of *visibility*.

Concerns:
- Holders of `users::read` learn every user's email — useful for downstream phishing.
- They learn every user's direct `permissions` array — useful for planning F-01-style escalation (find users with weak password reset capability, etc.).
- They learn every user's `last_login_at` — useful for picking "dormant accounts" to target.

`get_group_members` defensively zeroes out `permissions` (`ARRAY[]::TEXT[] as "permissions!"`, repository.rs:616) — that's a good pattern but inconsistent with `list_users`.

- **Recommendation:**
  - Introduce a `PublicUserSummary` type for list endpoints holding only `{ id, username, display_name, avatar_url, is_active }`. Reserve full `User` for the owner (a future `/me`) and `RequireAdmin`-gated handlers.
  - Same flattening for `get_user` unless the caller is the owner or root admin.
  - If full read is genuinely needed by `users::read`, log every access with `user_id`, `target_id`, `endpoint`, IP, user-agent.

---

### F-11 — No input length validation; control characters and CRLF accepted in display_name, username, email

- **Severity:** Medium
- **ASVS:** V5.1.3 (canonicalise), V5.2.5 (CRLF/log injection), V5.3.4
- **CWE:** CWE-20 (Improper Input Validation), CWE-93 (CRLF Injection)
- **Location:**
  - `src-app/server/src/modules/user/handlers/user.rs:96-102` (only emptiness check on create)
  - `src-app/server/src/modules/user/types.rs:13-29` (no constraints on field types)
- **Description:**

The only validation in `create_user` is `is_empty()`. There is:
- No max-length check (DB rejects > 100/255 chars but error path → F-08).
- No charset check on `username`. Unicode RTL-override, homoglyph attacks (`раypal` with Cyrillic `а`), and control characters (`\0`, `\n`, `\r`, `\t`) all pass.
- No email-format check. `"@"`, `"a"`, `"a@"` all accepted.
- No constraint on `display_name`. CRLF injection: if the display name is ever logged via `tracing::info!("user {} logged in", user.display_name)` without sanitisation, log forging becomes possible. The user module itself doesn't appear to log display names, but downstream modules might.

The previous audit's MEDIUM-07 covers LDAP injection in the LDAP path; that finding's CRLF concern applies here as well.

- **Recommendation:**
  - Add `#[derive(Validate)]` (validator crate) with `#[validate(length(min = 3, max = 64), regex = "USERNAME_REGEX")]` on `username`, `#[validate(email, length(max = 254))]` on `email`, `#[validate(length(max = 100))]` on `display_name`.
  - Reject any field containing C0/C1 control characters (`\u{0000}..=\u{001F}`, `\u{0080}..=\u{009F}`).
  - For `display_name`, also reject Unicode BiDi-override characters (U+202D, U+202E, U+2066-2069) to defeat RTL spoofing of UI labels.
  - For `username`, enforce ASCII-only or NFKC-normalised printable-Unicode, store the normalised form, reject distinct-but-equivalent registrations.

---

### F-12 — No rate limiting on any user-management endpoint

- **Severity:** Medium
- **ASVS:** V11.1.4, V13.1.4
- **CWE:** CWE-307 (Improper Restriction of Excessive Authentication Attempts), CWE-770
- **Location:** Module-wide; no `tower-governor`, `tower::limit`, or other limiter detected (`grep -rn "tower-governor"` returns only ai-providers crate hits).
- **Description:**

- `reset_user_password` is a perfect tool for an inside attacker to mass-reset: 100 concurrent calls reset 100 users' passwords with no throttling.
- `create_user` enables bulk-account creation for a misbehaving admin (then resign the account, leaving the orphans).
- `list_users?per_page=…` (F-06) is a single-request DoS but rate-limit would cap the damage at 1 large query.

The previous audit's HIGH-01 covers rate-limit on the auth surface; the user-module surface is the same gap.

- **Recommendation:** Mount `tower-governor` per-route with sensible defaults (e.g., 10 req/min for `reset-password`, 60 req/min for create/update/delete, 600 req/min for read-only). Key by JWT subject, not IP.

---

### F-13 — `email_verified` boolean is mutable indirectly and meaningless for security

- **Severity:** Low
- **ASVS:** V3.6.1
- **CWE:** CWE-1390 (Weak Authentication)
- **Location:** `migrations/00000000000001_initial_schema.sql:13`, `src-app/server/src/modules/user/models.rs:20`
- **Description:**

The `email_verified` column exists, defaults to `FALSE`, and is read by `axum_login::AuthUser` impls. But:
- No handler in the user module writes it.
- `create_user` always leaves it at the default (false).
- No handler reads it for gating; permission checks ignore it.
- `update_user` does not reset it on email change (F-03).

It's effectively dead state. Either delete the column (and the corresponding logic) or wire it up as part of the F-03 fix.

- **Recommendation:** Wire it up: set `email_verified = true` only after token-redemption, gate `RequirePermissions` to also require `user.email_verified` (or at least require it for any non-trivial permission), surface it in `MeResponse`.

---

### F-14 — `assigned_by` audit trail is the only record of group changes; nothing for permission edits

- **Severity:** Low
- **ASVS:** V7.1.1, V8.3.5 (security event logging)
- **CWE:** CWE-778 (Insufficient Logging)
- **Location:** `repository.rs:367-403`, all `update_*`/`assign_*` handlers
- **Description:**

`user_groups.assigned_by` is the only persisted trace of who-did-what. There is no audit row for:
- `update_user` (including permission changes).
- `update_group` (including permission changes).
- `delete_user`.
- `reset_user_password`.
- `set_active`.

If F-01 is exploited, a forensic investigator has only `users.updated_at` to go on and no record of *who* changed *what*. The default group's permissions could be silently widened with no trail.

- **Recommendation:** Add an `audit_log` table `{ id, actor_user_id, action, target_type, target_id, before_json, after_json, created_at, request_id, ip, user_agent }` and write a row from each mutating handler. Treat audit-log failure as a 500 (do not allow the mutation if logging fails).

---

### F-15 — `complete_guide` / `complete_guide_step` repository APIs accept arbitrary `guide_id` / `step_key` strings and array-append unconditionally

- **Severity:** Low
- **ASVS:** V5.1.3
- **CWE:** CWE-20
- **Location:** `src-app/server/src/modules/user/repository.rs:282-332`
- **Description:**

These are not wired to a user-module route, but they are public on the repository and called by `modules/onboarding/handlers.rs`. The functions take untrusted `guide_id: &str` and `step_key: &str` and unconditionally append (idempotent on equality, but no allow-list).

The arrays `completed_onboarding_ids` and `completed_onboarding_step_ids` are `TEXT[]` with no size cap. An attacker (with `profile::edit` or whatever the onboarding handler requires) could submit unbounded distinct keys, growing the user row beyond Postgres' TOAST threshold and slowing every subsequent `SELECT users.*`. Each "guide" or "step" should be validated against a known set.

- **Recommendation:** Validate `guide_id` / `step_key` against a static allow-list in the onboarding module before calling the repository; cap array length defensively (`array_length(…, 1) < 1000`).

---

### F-16 — `User` struct's `is_admin` returned in responses — telegraphs target value

- **Severity:** Low
- **ASVS:** V8.3.1
- **CWE:** CWE-200
- **Location:** `models.rs:14-34`, `handlers/user.rs:36-50` (`list_users`), 64-75 (`get_user`), 219-229 (`update_user`)
- **Description:**

`is_admin: bool` is `#[derive(Serialize)]` with no skip. Any listing or fetch returns it. Combined with F-10, a holder of `users::read` learns immediately which row is the root admin — useful for the F-02 deletion attack and the F-01 escalation attack.

- **Recommendation:** Either skip-serialize `is_admin` (and surface admin status only on `/me` or `RequireAdmin`-gated endpoints), or accept the leak as a defence-in-depth concession and document it explicitly in the threat model.

---

### F-17 — `update_user` and `update_group` are not transactional

- **Severity:** Low
- **ASVS:** V4.1.5
- **CWE:** CWE-362 (Race Condition)
- **Location:**
  - `handlers/user.rs:164-230` — fetches existing, validates, calls `update`, then optionally `set_active`, then `get_by_id` again.
  - `handlers/groups.rs:128-174`.
- **Description:**

The pattern "check existence → check unique → update" runs across multiple round-trips with no transaction. Two concurrent requests can both pass the uniqueness check and proceed to the INSERT/UPDATE, with the second hitting a unique-violation that returns the F-08-leaked error. For `update_user`, the `update()` call and the subsequent `set_active()` call are independent; a partial failure (rare) leaves the user in an inconsistent state.

- **Recommendation:** Wrap each mutating handler in `pool.begin()` … `tx.commit()`. Surface concurrent unique-violations as 409 not 500.

---

### F-18 — Info: `User::sanitized()` is defined but never called

- **Severity:** Info
- **ASVS:** V8.3.1
- **Location:** `models.rs:54-59`
- **Description:** A helper that clears `password_hash` exists, but `password_hash` is already `#[serde(skip_serializing)]` so the helper is unused. Either remove (dead code) or use it as the canonical "serialise" path so future fields can opt into sanitisation without modifying serde annotations.

---

### F-19 — Info: `GroupService::create_group` and other service methods unused

- **Severity:** Info
- **Location:** `service.rs:155-183` (`GroupService`)
- **Description:** The `GroupService` is defined but never instantiated; all handlers go directly to repositories. The `UserService` is instantiated only from the auth `/me` handler. Either delete the unused layer or move business rules (the privilege checks proposed in F-01 etc.) into it as the single point of enforcement.

---

### F-20 — Info: `UserEvent::LoggedIn` / `LoggedOut` defined but never emitted from this module

- **Severity:** Info
- **Location:** `events.rs:22-27, 45-52`
- **Description:** Login/logout events are wired through the auth module. The user module's `events.rs` defining them is fine as a contract surface but should be commented to clarify ownership.

---

### F-21 — Info: `update_user` and `update_group` use `COALESCE` semantics — explicit nulls cannot clear fields

- **Severity:** Info
- **ASVS:** V5.1.4
- **Location:** `repository.rs:207-227, 543-573`
- **Description:** A client passing `{"display_name": null}` will NOT clear the field; it will be ignored (serde deserialises `null` to `None`, which COALESCE keeps existing). This is a behavioural quirk worth documenting in the OpenAPI spec; not a security issue but a foot-gun for any UI that tries to clear a profile field.

---

## ASVS Coverage Matrix

| Control | Status | Notes |
|---|---|---|
| **V4.1.1** Trusted enforcement point | ✅ Pass | Every handler is `RequirePermissions<…>`. |
| **V4.1.2** Untrusted data validated | ⚠️ Partial | Empty-string check only; no length/format/charset (F-11). |
| **V4.1.3** Principle of least privilege | ❌ Fail | `users::edit` can grant `*` (F-01); `users::create` likewise (F-04). |
| **V4.1.5** Access-control failures fail securely | ⚠️ Partial | Extractor fails closed; but business rules above it grant authority via raw permission strings (F-01, F-04). |
| **V4.2.1** Object-level authorization | ❌ Fail | `delete_user` no admin guard (F-02). No "cannot grant higher than self" (F-01). |
| **V4.2.2** Function-level authorization | ✅ Pass | All routes have `RequirePermissions`. |
| **V4.3.1** Admin interfaces use stronger auth | ⚠️ Partial | Same JWT for admin and regular routes; no step-up. Existing audit covers MFA gap. |
| **V4.3.2** Multi-tenant data isolation | n/a | No tenant notion in the schema. |
| **V5.1.3** Input normalisation/canonicalisation | ❌ Fail | Username/email/display_name not normalised (F-11). |
| **V5.2.5** CRLF / log injection | ⚠️ Partial | Display name etc. could flow to logs unsanitised (F-11). |
| **V5.3.4** Param validation | ❌ Fail | `PaginationQuery` unbounded (F-06). |
| **V7.1.1** Logging sufficient | ❌ Fail | No audit log on mutations (F-14). |
| **V7.4.1** Sanitised error messages | ❌ Fail | Raw DB errors leaked (F-08). |
| **V8.1.1** Sensitive data not exposed | ❌ Fail | Email/permissions/last_login leak via `users::read` (F-10). |
| **V8.3.1** PII protected at rest/in transit | ⚠️ Partial | TLS assumed; at-rest encryption not configured here. |
| **V8.3.5** Security event logging | ❌ Fail | F-14. |
| **V11.1.4** Rate limiting | ❌ Fail | None present (F-12). |
| **V13.1.3** API pagination safe | ❌ Fail | F-06. |
| **V13.1.4** API rate-limit / throttling | ❌ Fail | F-12. |
| **V13.2.1** RESTful APIs (HTTP-method-appropriate auth) | ✅ Pass | All mutating methods are authenticated. |
| **V2.1.1** Password ≥ 12 chars | ❌ Fail | No minimum (F-05). |
| **V3.6.1** Email verification | ❌ Fail | F-03, F-13. |
| **V3.2.2** No username enumeration | ⚠️ Partial | F-09 (gated by `users::create`, narrowed by F-10's listing). |

---

## Positive Findings

1. **SQLx compile-time-verified queries throughout.** Every query in `repository.rs` uses `query!`, `query_as!`, or `query_scalar!`. No raw `sqlx::query` with format-string interpolation; no `bind` against attacker-controlled column names. SQL injection is structurally impossible in the user module.

2. **`password_hash` is `#[serde(skip_serializing)]`** (models.rs:21-23) and additionally has `#[schemars(skip)]` so it doesn't appear in the OpenAPI schema either. Good defence-in-depth.

3. **`AuthUser::session_auth_hash()` returns the password-hash bytes** (models.rs:43-50). This means the axum-login session is automatically invalidated on password change. (Caveat: the JWT layer is independent and is not invalidated; see F-03.)

4. **Admin-protection on `toggle_user_active` and `update_user`.** Both refuse to disable a user with `is_admin = true` (handlers/user.rs:177-182 and 261-265). The same check needs to extend to `delete_user` (F-02).

5. **System-group protection on `delete_group`** (handlers/groups.rs:203-205). Same protection needs extending to `update_group`'s `permissions` and to `assign_user_to_group`'s target (F-01, F-07).

6. **`get_group_members` defensively zeros out user permissions** (`ARRAY[]::TEXT[] as "permissions!"`, repository.rs:616). This is the correct pattern; extend it to `list_users` (F-10).

7. **`RequirePermissions` extractor is consistent and re-loads the user on every request**, so admin/active/permission revocation takes effect on the next request (no stale data from JWT claims).

8. **Partial unique index on `is_admin` = true** (migration 1, line 32) — schema-enforced single root admin. Deletion is still possible (F-02) but accidental promotion of a second admin via direct DB is prevented.

9. **`assign_to_group` uses `ON CONFLICT (user_id, group_id) DO NOTHING`** (repository.rs:374-385), making the call idempotent. No accidental duplicate-row errors.

10. **Default group assignment on user creation is best-effort** (`let _ = …`, handlers/user.rs:138-143). Failure does not abort user creation, which is correct.

11. **JSON `Json(request): Json<…>` extractor** — by default rejects on unknown field names is *not* enabled (no `deny_unknown_fields`), but the `permissions`-injection vector (F-01) is via *explicitly declared* fields, not unknown ones, so the missing flag is only a defence-in-depth gap.

---

## Out of Scope / Deferred

- **Authentication itself (JWT issuance, refresh, OAuth)** — covered in `.sec-audits/01-auth-user-permissions-audit.md`. This audit assumes a valid bearer JWT and asks the question "what can the bearer do?"
- **Permission extractor implementation** — covered in the existing audit (LOW-01 / LOW-02 / LOW-06 etc.). Verified that the extractor fails closed; this audit's findings concern the business logic *above* the extractor.
- **MFA / step-up auth** — recommended in the existing audit but not in scope here.
- **At-rest encryption of PII** (email, names) — Postgres-level concern; not visible in this module.
- **GDPR data-subject right-to-erasure** — F-02's soft-delete recommendation partially addresses this; full GDPR review is a separate workstream.
- **Tests** — `tests/user/mod.rs` and `tests/user_group/mod.rs` exist (~1,600 LOC across both); they exercise happy paths and admin-protection on toggle/update but **do not test the F-01 / F-04 / F-02 paths**. A follow-up should add adversarial tests:
  - `test_users_edit_cannot_grant_wildcard`
  - `test_users_create_cannot_grant_wildcard`
  - `test_groups_edit_cannot_modify_system_group_permissions`
  - `test_users_delete_blocks_admin`
  - `test_assign_user_blocks_system_group`
  - `test_per_page_clamps_to_max`
  - `test_per_page_zero_rejected`
  - `test_database_error_does_not_leak_constraint_names`

- **Frontend** — UI-side validation is not in scope; backend must be the authoritative gate.

---

## Recommended Remediation Order

1. **Immediate (24h, Critical):**
   - F-02: Add `is_admin` guard to `delete_user`.
   - F-01 path 1+2+3: Strip `permissions` from `UpdateUserRequest` and `UpdateGroupRequest`'s system-group path; refuse `assign_user_to_group` for `is_system` targets.
   - F-04: Strip `permissions` from `CreateUserRequest`.

2. **Week 1 (High):**
   - F-03: Implement email-confirmation flow + invalidate JWT on email change (or restrict to root-admin until ready).
   - F-05: Centralise password policy (min length 12, complexity, bcrypt cost ≥ 14).
   - F-06: Clamp `PaginationQuery::per_page` to `1..=100`; reject negatives; guard `total_pages` math.

3. **Month 1 (Medium):**
   - F-07: System-group invariants on default and Administrators.
   - F-08: Sanitise database errors.
   - F-09: Generic conflict response.
   - F-10: `PublicUserSummary` DTO for list endpoints.
   - F-11: Validator crate on all DTOs.
   - F-12: tower-governor rate limit.

4. **Backlog (Low / Info):**
   - F-13 — F-21.

---

**End of audit.**
