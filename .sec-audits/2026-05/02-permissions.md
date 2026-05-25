# Security Audit — Permissions Module
**Date:** 2026-05-23
**Scope:** modules/permissions/ (~758 LOC) + cross-module route-gating audit
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target

## Executive Summary
- Findings by severity: Critical: 0, High: 4, Medium: 6, Low: 5, Info: 4
- ASVS chapters touched: V4 (Access Control — primary), V1 (Architecture), V13 (API)
- Top 3 risks:
  1. **F-01 (High)** — `users::edit` lets the holder grant arbitrary permissions to any user (including themselves) via `PUT /api/users/{id}` because the `permissions: Vec<String>` body field is passed through unfiltered. Combined with F-02 this is a single-permission privilege escalation to "everything except `is_admin=true`". (`modules/user/handlers/user.rs:164-230`)
  2. **F-02 (High)** — `groups::edit` lets the holder rewrite the `permissions` array of *any* group, including system groups. Granting `*` to the default user group instantly escalates every existing and future user. (`modules/user/handlers/groups.rs:127-174`)
  3. **F-03 (High)** — `download_with_token` endpoint validates a JWT signed with the **same secret** as the application access-token JWT, with `Validation::default()` (no `iss`/`aud` constraint). Cross-token confusion: any forged token or compromised secret yields a generic-purpose file-grant primitive; on the defensive side, every code path that hashes the JWT secret now serves two distinct trust boundaries. (`modules/file/handlers/download.rs:110-122`)

> Note: there is also an Info-level concern, **F-15**, about every authenticated request executing **two synchronous Postgres queries** (`users.get_by_id` + `users.get_user_groups`) inside the extractor — call it "unbounded read amplification". Not a security flaw in itself but a DoS-multiplier and a measurable cost on every protected route. Documented for completeness.

The permissions module's *core design* — a typed `RequirePermissions<P: PermissionList>` extractor with compile-time permission strings and a union-based check across user + group permissions — is **architecturally sound**. Findings concentrate around (a) the lack of a host-level admin-write protection on user/group mutation handlers, (b) one structural issue with download tokens reusing the JWT secret, and (c) several gating omissions in the elicitation/MCP-runtime flow.

## Findings

### F-01: `users::edit` permission silently grants permission-array writes (root-equivalent escalation)
- **Severity / ASVS / CWE / Location:** High / V4.2.1, V4.2.2 / CWE-269 (Improper Privilege Management) / `modules/user/handlers/user.rs:164-230`
- **Description:** The `update_user` handler accepts `UpdateUserRequest { permissions: Option<Vec<String>> }` and forwards it unfiltered to `UserRepository::update`. Holding `users::edit` alone is sufficient to:
  1. Grant `*` to oneself (instant approximate-root, missing only the `is_admin` boolean which is not exposed through any DTO).
  2. Grant arbitrary permissions to other users, including ones the actor doesn't hold.
  3. Edit the root admin's `permissions` array (no `target.is_admin` short-circuit).
- **Vulnerable code:**
  ```rust
  // modules/user/handlers/user.rs:165-212
  pub async fn update_user(
      _auth: RequirePermissions<(UsersEdit,)>,
      Extension(event_bus): Extension<Arc<EventBus>>,
      Path(user_id): Path<Uuid>,
      Json(request): Json<UpdateUserRequest>,
  ) -> ApiResult<Json<User>> {
      // … only "is_admin && is_active=false" is gated …
      Repos.user.update(
          user_id,
          request.username,
          request.email,
          request.display_name,
          request.permissions,   // ← raw passthrough
      ).await?;
  ```
- **Exploitation:** Attacker who has `users::edit` (e.g. a junior admin role) hits `POST /api/users/<self>` with `{"permissions": ["*"]}`. Their next request sees them with global wildcard; combined with `groups::edit` (F-02) they can also escalate via the default group. The only thing they cannot grant is `is_admin = true`, but `*` already bypasses every `RequirePermissions<_>` check.
- **Impact:** One-permission privilege escalation to near-root. Bypasses the entire RBAC matrix.
- **Recommendation:**
  1. Require *both* `users::edit` *and* a new `users::manage_permissions` permission to mutate the `permissions` field (split write authority).
  2. Reject the request if `auth.user` is not root admin and the request would grant a permission the actor doesn't already hold ("least-privilege grant").
  3. Block edits to `target.is_admin == true` for non-root callers, mirroring the existing `is_active` guard.
  4. Audit-log every successful permission change with `(actor_id, target_id, before, after)`.

---

### F-02: `groups::edit` allows rewriting the system "users" group permissions (cascade escalation)
- **Severity / ASVS / CWE / Location:** High / V4.2.1 / CWE-269 / `modules/user/handlers/groups.rs:127-174`
- **Description:** `update_group` blocks renaming or deactivating a system group, but **not** rewriting its `permissions` array. Every user is auto-assigned to the default group on registration (`auth/handlers.rs:88-93`, `assign_user_to_default_group`), so granting `*` to the default group is a one-API-call mass escalation across the whole user base. Also, system groups (`is_system = true`) are not write-protected at the permission-array level.
- **Vulnerable code:**
  ```rust
  // modules/user/handlers/groups.rs:142-149
  if existing_group.is_system {
      if request.name.is_some() || request.is_active == Some(false) {
          return Err(AppError::bad_request("SYSTEM_GROUP", …));
      }
  }
  // …permissions field flows through unconditionally:
  Repos.group.update(group_id, request.name, request.description,
                     request.permissions, request.is_active).await?
  ```
- **Exploitation:** Attacker with `groups::edit` sends `POST /api/groups/<default-group-id>` body `{"permissions": ["*"]}`. All existing users in the default group immediately inherit `*` via the union check; the actor's next request runs as root-equivalent.
- **Impact:** Mass privilege escalation across all users. Even more severe than F-01 because it can't be mitigated by per-user audit-logging review — one attack updates N users at once.
- **Recommendation:**
  1. Same split as F-01: require `groups::manage_permissions` (or root-admin) to mutate the `permissions` field.
  2. **Refuse** any permission-array rewrite on `is_system = true` groups unless caller is root admin.
  3. Refuse to grant a permission the caller doesn't already hold themselves.
  4. Emit an audit-log row on every change.

---

### F-03: Download-token JWT shares the access-token signing key (no `iss`/`aud` partition)
- **Severity / ASVS / CWE / Location:** High / V3.5.2 (token-binding), V6.2.3 (cryptographic isolation) / CWE-345 (Insufficient Verification of Data Authenticity) / `modules/file/handlers/download.rs:84-122`, `modules/file/types.rs:63-70`, `modules/file/config.rs:1-18`
- **Description:** `download_with_token` validates a separate JWT (`DownloadTokenClaims { file_id, user_id, exp, iat }`) using the **same `JwtConfig.secret`** as the main access-token system, with `Validation::default()` (which only checks expiry — no `iss`/`aud`). The download-token claims also do not set `iss` or `aud`. Effects:
  - The two token classes are **cryptographically indistinguishable**: a single secret leak compromises both surfaces simultaneously.
  - Any code path that "trusts a token signed by this secret" can no longer distinguish "general access" from "single-file download".
  - Future hardening (key rotation, hsm offload, jwks) needs to be done in two places without bugs.
- **Vulnerable code:**
  ```rust
  // modules/file/handlers/download.rs:84-99 (token mint, no iss/aud)
  let claims = DownloadTokenClaims {
      file_id: file_id.to_string(),
      user_id: user_id.to_string(),
      exp: now + TOKEN_EXPIRY as usize,
      iat: now,
  };
  let token = encode(&Header::default(), &claims,
                     &EncodingKey::from_secret(jwt_config.secret.as_bytes()))?;

  // modules/file/handlers/download.rs:114-122 (token verify, default validation)
  let claims = decode::<DownloadTokenClaims>(
      &query.token,
      &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
      &Validation::default(),     // ← no iss/aud check
  ).map_err(|_| StatusCode::UNAUTHORIZED)?.claims;
  ```
- **Exploitation:** Not directly exploitable from the outside (you need the secret to forge anything). But it is a design defect that significantly raises blast radius if the secret leaks via logs, env exposure, or a future bug — both APIs collapse together. It also blocks operational rotation: rotating the access-token secret invalidates all in-flight download URLs.
- **Impact:** Cryptographic isolation violation; complicates incident response and key rotation.
- **Recommendation:**
  1. Derive a **separate** key for download tokens (e.g. HKDF of the master secret with label `"file-download-v1"`), stored under a new config knob.
  2. Add `iss = "ziee-chat-file-download"` and `aud = "ziee-chat-file-download-v1"` to download claims and use `Validation::new(Algorithm::HS256)` with explicit `set_issuer`/`set_audience` like the main `JwtService`.
  3. Add a JTI to download claims and a small revocation table — these are short-lived (1 h) urls that may end up in browser history, logs, or shoulder-surfed; one-shot consumption is desirable.

---

### F-04: `respond_to_elicitation` lacks per-elicitation ownership check (cross-tenant write)
- **Severity / ASVS / CWE / Location:** High / V4.2.1, V4.3.1 / CWE-639 (Authorization Bypass Through User-Controlled Key / IDOR) / `modules/mcp/elicitation/handlers.rs:23-72`
- **Description:** The endpoint only requires `mcp_servers::read`. The handler then calls `registry::respond(elicitation_id, …)` with no verification that the calling user is the one who triggered the elicitation. If `elicitation_id`s are guessable (they are UUIDs — 122 bits of entropy, so realistically not brute-forceable, but observable via SSE leaks, server logs, or a shoulder-surf in dev) any holder of `mcp_servers::read` (a permission held by all default users) can respond to *any* other user's pending elicitation:
  - `accept` with attacker-controlled `content` → user-supplied form values are written into the originating user's MCP tool call.
  - `decline` / `cancel` → DoS the legitimate user's elicitation flow.
- **Vulnerable code:**
  ```rust
  // modules/mcp/elicitation/handlers.rs:22-50
  pub async fn respond_to_elicitation(
      _auth: RequirePermissions<(McpServersRead,)>,  // ← weak gate, no ownership
      Path(elicitation_id): Path<Uuid>,
      Json(request): Json<…>,
  ) -> … {
      // …no auth.user.id is even used:
      let (found, content_id_opt) = registry::respond(elicitation_id, response);
  ```
  Note the discarded `_auth` — the user-identity from the JWT is never threaded through.
- **Exploitation:** Attacker observes a UUID (e.g. via log files, error messages, browser network trace shared in screenshots) and writes a fraudulent acceptance: their JSON `content` flows into the victim's MCP tool invocation.
- **Impact:** Cross-user write into a security-relevant control flow (MCP tool elicitation), with attacker-controlled body content.
- **Recommendation:** In `registry::respond`, atomically check that `elicitation.user_id == auth.user.id` (the registry already stores the originating user; if it doesn't, store it). Return 404 on mismatch to avoid existence-leak.

---

### F-05: Per-request DB read amplification (two queries on every protected route, no caching)
- **Severity / ASVS / CWE / Location:** Medium / V4.1.5, V11.1.4 / CWE-770 (Allocation of Resources Without Limits or Throttling) / `modules/permissions/extractors.rs:87-127`
- **Description:** Every `RequirePermissions<…>` extractor invocation performs two synchronous Postgres queries:
  - `Repos.user.get_by_id(user_id)` — single-row, indexed; cheap.
  - `Repos.user.get_user_groups(user.id)` — JOIN through `user_groups`; ~O(groups_per_user) rows.

  No request-scoped caching, no Redis/sled, no in-process LRU. Combined with the fact that the JWT is **stateless** (no token-version field), this means:
  1. Every authenticated request executes ≥2 DB queries before any business logic runs.
  2. A single Bearer token issued before a logout *cannot* be revoked except by waiting for natural expiry (auth audit `01-` finding HIGH-03 covers this).
  3. There is no rate limit on extractor failure paths, so a malformed request with a valid token can exercise the user+group lookup for free.
- **Impact:** DoS amplification (each Bearer-authenticated DoS request costs 2 DB roundtrips). Also a performance ceiling for the service.
- **Recommendation:**
  1. Add a small request-scoped or short-TTL in-process LRU keyed on `(user_id, claims.iat)` storing `(user, groups)`; invalidate on group/user mutation events (events.rs already emits these).
  2. Optionally embed a `permissions_version` UUID in the JWT and bump it on user/group permission change; the extractor can then trust cached state if versions match without re-querying.
  3. Apply a tower-governor rate limit at the API prefix.

---

### F-06: Admin bypass loads no groups → admin-discriminating handlers misclassify root admins
- **Severity / ASVS / CWE / Location:** Medium / V4.2.1 / CWE-1059 (Insufficient Technical Documentation) / `modules/permissions/extractors.rs:112-119`, `modules/mcp/handlers/runtime.rs:36-66`
- **Description:** When `user.is_admin == true`, the extractor short-circuits with `groups: vec![]`. Several handlers then inspect `auth.groups` to determine extended access (e.g. MCP `has_admin_access` checks if any group has a `mcp_servers_admin::*` permission). Result: a root admin is *not* recognized by these helpers as having admin MCP access and is therefore subject to the per-server ACL check. This is the opposite of what you'd expect from "root admin bypasses everything".
- **Vulnerable code:**
  ```rust
  // modules/permissions/extractors.rs:112-119 — admin sees no groups
  if user.is_admin {
      return Ok(Self { user, groups: vec![], _marker: PhantomData });
  }

  // modules/mcp/handlers/runtime.rs:36-42 — checks groups only
  fn has_admin_access(groups: &[Group]) -> bool {
      groups.iter().any(|group| {
          group.permissions.iter().any(|perm| perm.starts_with("mcp_servers_admin::"))
      })
  }
  ```
- **Exploitation:** Not a direct exploit — it's a "wrong default" behavior: root admin loses convenience access. But subtler: it means **the security posture differs between (root admin) and (user with `*` in a group)**, which leads to inconsistent test coverage and surprise security bugs later. It also weakens the contract that root admin is "always allowed".
- **Impact:** Behavioral surprise; risk of inconsistent enforcement during future refactors.
- **Recommendation:**
  1. Either (a) load groups for root admins too — the extra DB query is the same cost as for any other user — and let `has_admin_access` see them; or
  2. (b) Have `has_admin_access` and any similar helper consult `user.is_admin || /* group check */` explicitly.
  Option (b) is cheaper but creates two places to forget.

---

### F-07: `has_admin_access` uses `starts_with("mcp_servers_admin::")` — overly broad, never null
- **Severity / ASVS / CWE / Location:** Medium / V4.2.2 / CWE-185 (Incorrect Regular Expression) / `modules/mcp/handlers/runtime.rs:36-42`
- **Description:** The helper matches any string beginning with the prefix, including hypothetical fictional permissions like `mcp_servers_admin::future_no_op` or even malformed strings someone could type into the group permissions array (no validation at write time). A user with **any** `mcp_servers_admin::*` permission — even one as innocuous as "view stats" — bypasses the per-server ACL for `list_server_tools`, `call_server_tool`, etc.
- **Vulnerable code:** see F-06.
- **Exploitation:** Low — relies on permission misconfiguration. But the broad prefix match makes future permission additions footguns.
- **Recommendation:** Match a specific permission (e.g. `mcp_servers_admin::access_all_servers`) declared in the permissions module, not a string-prefix scan.

---

### F-08: System-group `permissions` array writes silently accepted (related to F-02)
- **Severity / ASVS / CWE / Location:** Medium / V4.1.3 / CWE-732 (Incorrect Permission Assignment) / `modules/user/handlers/groups.rs:142-149`
- **Description:** Subset of F-02 viewed from the "system groups are special" angle. The guard at line 142 explicitly only protects `name` and `is_active`. Description and **permissions** flow through. Highlighting separately because the *intent* of the existing partial guard is "system groups are read-only"; the implementation contradicts that intent.
- **Recommendation:** Extend the guard to also block `request.permissions.is_some()` and `request.description.is_some()` on `is_system = true` groups, unless caller is root admin.

---

### F-09: `user::service::has_permission` and `auth::backend::has_permission` use single-colon `:` not `::`
- **Severity / ASVS / CWE / Location:** Medium / V4.1.1 / CWE-697 (Incorrect Comparison) / `modules/user/service.rs:106-110`, `modules/auth/backend.rs:252-256`
- **Description:** Both methods implement an alternate permission-check pathway with the wrong separator:
  ```rust
  // service.rs:106-110 — single colon `:`
  if let Some((resource, _)) = permission.split_once(':') {
      let wildcard = format!("{}:*", resource);
      if permissions.contains(&wildcard) { return Ok(true); }
  }
  ```
  Canonical permissions in the system use `::` (`users::read`, `config::auth::*`, …). These methods would silently return `false` (or worse, match spurious wildcards) if ever wired up. Currently they appear to be dead code — neither is invoked from any route handler — but they exist as a latent footgun. The `AuthBackend::get_all_permissions` query also ignores `user.permissions` (only joins on groups) and doesn't filter `is_active`, compounding the divergence from the canonical `check_permission_union`.
- **Recommendation:**
  1. Delete `UserService::has_permission`, `UserService::get_user_permissions`, `AuthBackend::has_permission`, `AuthBackend::get_all_permissions` — and the entire `AuthBackend` module (it's `#![allow(dead_code)]` already; auth uses JWT now, not axum-login).
  2. If kept, route them through the canonical `check_permission_union` in `permissions::checker` so there is exactly one permission-check implementation.

---

### F-10: Naming collision — `LocalRuntimeRead` and `RuntimeVersionRead` both map to `llm_local_runtime::read`
- **Severity / ASVS / CWE / Location:** Medium / V1.4.1 / CWE-1126 (Declaration of Variable with Unnecessarily Wide Scope) / `modules/llm_local_runtime/permissions.rs:10-15, 40-46`
- **Description:** Two distinct `PermissionCheck` structs share the same `PERMISSION` constant string. Holding `llm_local_runtime::read` grants both. This is not by itself a bug — it's an intentional shared permission — but the system has no way to express "these two type-level handles are aliases", and the OpenAPI spec will print two different `NAME` values pointing at the same actual permission, confusing consumers. If one struct were ever updated and the other forgotten, the discrepancy would silently widen access.
- **Recommendation:** Either rename one struct and use a distinct permission, or introduce an explicit aliasing concept in `PermissionCheck` (e.g. `type Alias = LocalRuntimeRead`) and emit only one entry in the canonical `all_permissions()` lists.

---

### F-11: Default-permissive CORS when configuration omitted
- **Severity / ASVS / CWE / Location:** Low / V14.5.3 / CWE-942 (Permissive CORS) / `core/app_builder.rs:150-156`
- **Description:** If no CORS configuration is supplied, the server falls back to `allow_origin(Any) + allow_methods(Any) + allow_headers(Any)`. Combined with Bearer-token auth (no cookies), the impact is limited (CSRF doesn't apply to header-bearing auth on non-cookie endpoints), but it does enable cross-origin reads of every JSON response by browser-trapped scripts.
- **Recommendation:** Default to an explicit empty origin list (forcing operator configuration) and document the requirement.

---

### F-12: `update_user` allows enumerating root admin accounts via 400 vs 200 differential
- **Severity / ASVS / CWE / Location:** Low / V4.3.2 / CWE-204 (Observable Response Discrepancy) / `modules/user/handlers/user.rs:177-182`
- **Description:** The handler returns `400 CANNOT_DISABLE_ADMIN` when an admin user is targeted with `is_active=false`, and `200` for any non-admin target. A caller with `users::edit` can map out the admin user IDs by issuing a no-op update.
- **Recommendation:** Reject any update to an admin account by a non-admin caller with the same opaque 403 used elsewhere; do not differentiate.

---

### F-13: Permission strings lack write-time validation (typos silently create dead permissions)
- **Severity / ASVS / CWE / Location:** Low / V5.1.3 / CWE-20 (Improper Input Validation) / `modules/user/handlers/groups.rs:90-111` (create), `modules/user/handlers/groups.rs:127-174` (update)
- **Description:** When creating or updating a group/user, the `permissions: Vec<String>` field accepts any string — no check that it matches a declared `PermissionCheck::PERMISSION`. Typos like `users:reed` (single colon, "reed") quietly become dead permissions that grant nothing but bloat the array. Worse: a typo of a wildcard like `users:*` matches nothing under the canonical `::`-separated checker but **would** match under the buggy `:` checkers in F-09 if those ever get wired up.
- **Recommendation:** Validate at insert time against a registry built from all `PermissionCheck::PERMISSION` values plus wildcards (`*`, `module::*`, `module::sub::*`). Reject unknown strings with a 400.

---

### F-14: Extractor returns 500 (not 403) when JWT service Extension is missing
- **Severity / ASVS / CWE / Location:** Low / V7.4.1 / CWE-209 (Generation of Error Message Containing Sensitive Information) / `modules/permissions/extractors.rs:51-56, 183-188`
- **Description:** If the `Arc<JwtService>` Extension is missing from the request — a misconfiguration scenario — the extractor returns `500 INTERNAL_SERVER_ERROR` with body `"JWT service not configured"`. In production this leaks the fact that a misconfigured deployment exists. More importantly, the **fail-mode here is closed** (the handler doesn't run), so this is purely a UX/disclosure issue, not an auth bypass. Still, it should be a generic 503.
- **Recommendation:** Return `503 Service Unavailable` with no detail body; log the actual cause server-side.

---

### F-15: OpenAPI 401 documentation inconsistency across handlers
- **Severity / ASVS / CWE / Location:** Low / V13.2.1 / CWE-1059 / many files (e.g. `modules/assistant/handlers.rs:84-87` documents 403 but not 401; `modules/file/handlers/management.rs:197` documents 401 but the actual extractor returns 401 only for missing-token paths).
- **Description:** The `with_permission::<P>` helper already attaches a 403 response. Several handlers separately attach a 401 response; some don't. The 401 path is taken when:
  - The `Authorization` header is missing.
  - The header doesn't start with `Bearer `.
  - The JWT validation fails (expired/invalid signature).
  - The user is no longer in the DB.

  These are all "401 Unauthorized" by the extractor; the OpenAPI spec needs to advertise this on every protected endpoint for consumers to handle re-auth correctly.
- **Recommendation:** Make `with_permission` also attach a 401 response (currently it only attaches 403). Then existing per-handler 401 entries become redundant but harmless.

---

### F-16 (Info): Group `is_active` filter happens only in `check_permission_union`, not at query time
- **Severity / ASVS / CWE / Location:** Info / — / `modules/permissions/checker.rs:16-19` vs `modules/user/repository.rs:349-364`
- **Description:** `get_user_groups` returns *all* groups (active and inactive). The extractor then filters in Rust via `if !group.is_active { continue }`. Functionally correct, but it means inactive-group rows are fetched and copied to userspace. For a tenant with many inactive groups this is wasteful. Cosmetic — flagged for hygiene.
- **Recommendation:** Add `WHERE g.is_active = true` to the SQL query.

---

### F-17 (Info): No audit log of permission checks (success or failure)
- **Severity / ASVS / CWE / Location:** Info / V7.1.1, V7.1.4 / —
- **Description:** Neither the extractor nor the checker emits a tracing event when permission is denied. This makes after-the-fact forensic analysis of attempted privilege escalations difficult.
- **Recommendation:** Add `tracing::info!` on 403 with `(user_id, required_permissions, granted_permissions_summary)`. Do NOT log the actual list of granted permissions (could be too verbose), but at least the count and the first missing one.

---

### F-18 (Info): No rate limit on protected endpoints' extractor cost
- **Severity / ASVS / CWE / Location:** Info / V11.1.4 / — / (no implementation)
- **Description:** Companion to F-05. Even fast paths that fail at `users::edit` cost two DB queries.
- **Recommendation:** Apply tower-governor or similar at the API prefix.

---

### F-19 (Info): Wildcard semantics are documented in checker comments but nowhere in user-facing docs
- **Severity / ASVS / CWE / Location:** Info / V13.1.1 / — / `modules/permissions/checker.rs:42-52`
- **Description:** The checker supports `*`, `resource::*`, `module::sub::*` wildcards. This is excellent flexibility but is not surfaced in API docs or admin UI tooltips. Admins creating groups may grant `users::*` thinking it only matches `users::read` etc., and be unaware it also matches future-added permissions like `users::impersonate`.
- **Recommendation:** Surface wildcard semantics in the OpenAPI description of any endpoint that accepts a `permissions` array, and add a UI warning.

---

## Route-gating coverage audit

I surveyed every `.route(`/`.api_route(` in `modules/*/routes.rs` and cross-referenced the handler-level `RequirePermissions<…>` / `JwtAuth` / `RequireAdmin` usage. Of the ~110 routes registered, **all but five** are gated by an explicit `RequirePermissions<…>` extractor on the handler. The exceptions are intentional and documented.

| Route path | Method | Handler file:line | Required permission | Gating present? | Issue? |
|---|---|---|---|---|---|
| `/api/health` | GET | `health/handlers.rs` | (public) | none | ✅ intentional |
| `/api/setup/status` | GET | `app/handlers.rs` | (public) | none | ✅ intentional — first-boot probe |
| `/api/setup/admin` | POST | `app/handlers.rs:80+` | (refuses after first admin) | self-gated | ✅ checked via `has_admin` query |
| `/api/auth/register` | POST | `auth/handlers.rs:34` | (public) | none | ✅ intentional |
| `/api/auth/login` | POST | `auth/handlers.rs:117` | (public) | none | ✅ intentional |
| `/api/auth/refresh` | POST | `auth/handlers.rs:329` | (public, refresh token validated) | self-gated | ✅ |
| `/api/auth/logout` | POST | `auth/handlers.rs:387` | (any auth user) | `JwtAuth` | ✅ |
| `/api/auth/me` | GET | `auth/handlers.rs:404` | (any auth user) | `JwtAuth` | ✅ |
| `/api/auth/oauth/{p}/authorize` | GET | `auth/handlers.rs:445` | (public) | none | ✅ intentional (OAuth init) |
| `/api/auth/oauth/{p}/callback` | GET | `auth/handlers.rs:493` | (public) | none | ✅ intentional (OAuth callback) — but see audit `01-` finding CRITICAL-01 on URL-token exposure |
| `/api/files/upload` | POST | `file/handlers/upload.rs:21` | `files::upload` | `RequirePermissions` | ✅ |
| `/api/files` | GET | `file/handlers/management.rs:20` | `files::read` | `RequirePermissions` | ✅ |
| `/api/files/{id}` | GET | `file/handlers/management.rs:42` | `files::read` | `RequirePermissions` | ✅ |
| `/api/files/{id}` | DELETE | `file/handlers/management.rs:173` | `files::delete` | `RequirePermissions` | ✅ + ownership check |
| `/api/files/{id}/preview` | GET | `file/handlers/management.rs:57` | `files::preview` | `RequirePermissions` | ✅ |
| `/api/files/{id}/thumbnail` | GET | `file/handlers/management.rs:88` | `files::preview` | `RequirePermissions` | ✅ |
| `/api/files/{id}/text` | GET | `file/handlers/management.rs:118` | `files::read` | `RequirePermissions` | ✅ |
| `/api/files/{id}/download` | GET | `file/handlers/download.rs:23` | `files::download` | `RequirePermissions` | ✅ |
| `/api/files/{id}/download-token` | POST | `file/handlers/download.rs:71` | `files::generate_token` | `RequirePermissions` | ✅ |
| `/api/files/{id}/download-with-token` | GET | `file/handlers/download.rs:110` | **(token-only)** | **no permission extractor** | ⚠️  intentional but see F-03 |
| `/api/users` | GET | `user/handlers/user.rs:32` | `users::read` | ✅ | (but see F-12 enumeration) |
| `/api/users` | POST | `user/handlers/user.rs:90` | `users::create` | ✅ | |
| `/api/users/{id}` | POST | `user/handlers/user.rs:164` | `users::edit` | ✅ | ❌ see F-01 |
| `/api/users/{id}` | DELETE | `user/handlers/user.rs:341` | `users::delete` | ✅ | ⚠ no `is_admin` self-protection |
| `/api/users/{id}/toggle-active` | POST | `user/handlers/user.rs:248` | `users::toggle_status` | ✅ | guards is_admin |
| `/api/users/reset-password` | POST | `user/handlers/user.rs:305` | `users::reset_password` | ✅ | ⚠ no `is_admin` self-protection |
| `/api/groups` | GET | `user/handlers/groups.rs:32` | `groups::read` | ✅ | |
| `/api/groups` | POST | `user/handlers/groups.rs:90` | `groups::create` | ✅ | |
| `/api/groups/{id}` | POST | `user/handlers/groups.rs:128` | `groups::edit` | ✅ | ❌ see F-02 |
| `/api/groups/{id}` | DELETE | `user/handlers/groups.rs:191` | `groups::delete` | ✅ | |
| `/api/groups/assign` | POST | `user/handlers/groups.rs:270` | `groups::assign_users` | ✅ | ⚠ no check that caller can assign to target group's permission set |
| `/api/groups/{u}/{g}/remove` | DELETE | `user/handlers/groups.rs:307` | `groups::assign_users` | ✅ | |
| `/api/conversations` | POST,GET | `chat/core/handlers/conversations.rs` | `conversations::create`/`read` | ✅ | ownership-enforced |
| `/api/conversations/{id}` | GET,PUT,DELETE | `chat/core/handlers/conversations.rs` | `conversations::read/edit/delete` | ✅ | ownership-enforced |
| `/api/conversations/{id}/messages` | GET | `chat/core/handlers/messages.rs:26` | `messages::read` | ✅ | ownership-enforced |
| `/api/conversations/{id}/messages/stream` | POST | `chat/core/handlers/streaming.rs:31` | `messages::create` | ✅ | ownership-enforced |
| `/api/messages/{id}` | GET,DELETE | `chat/core/handlers/messages.rs:60,133` | `messages::read/delete` | ✅ | ownership via `verify_message_ownership` |
| `/api/conversations/{id}/branches` | * | `chat/core/handlers/branches.rs` | `branches::create`, `conversations::read`, `branches::switch` | ✅ | ownership-enforced |
| `/api/conversations/{id}/mcp-settings` | GET,PUT | `chat/extensions/mcp/approval/handlers.rs` | `conversations::read/edit` | ✅ | ownership-enforced |
| `/api/branches/{id}/pending-approvals` | GET | `chat/extensions/mcp/approval/handlers.rs:125` | `conversations::read` | ✅ | ⚠ **no ownership check** — branch_id is a path param but no verification that the branch belongs to the calling user. Reported in chat-module audit but flagged here for permission-gate coverage. |
| `/api/mcp/defaults` | GET,PUT | `chat/extensions/mcp/defaults/handlers.rs` | `conversations::read/edit` | ✅ | uses calling user's ID for storage; safe |
| `/api/mcp/servers` | GET,POST | `mcp/handlers/user.rs` | `mcp_servers::read/create` | ✅ | |
| `/api/mcp/servers/{id}` | * | `mcp/handlers/user.rs` | `mcp_servers::read/edit/delete` | ✅ | |
| `/api/mcp/servers/{id}/oauth` | * | `mcp/handlers/user.rs` | `mcp_servers::read/edit` | ✅ | |
| `/api/mcp/servers/{id}/tools/{n}/call` | POST | `mcp/handlers/runtime.rs:79` | `mcp_servers::read` | ✅ | + per-server ACL check, with admin bypass (see F-06/F-07) |
| `/api/mcp/system-servers/*` | * | `mcp/handlers/system.rs` | `mcp_servers_admin::*` | ✅ | |
| `/api/mcp/elicitation/{id}/respond` | POST | `mcp/elicitation/handlers.rs:23` | `mcp_servers::read` | ⚠ partial | ❌ see F-04 — **no per-elicitation ownership** |
| `/api/llm-providers/*` (admin) | * | `llm_provider/handlers/admin.rs` | `llm_providers::*` | ✅ | |
| `/api/user-llm-providers/*` | * | `llm_provider/handlers/user.rs` | `user_llm_providers::read`, `profile::*` | ✅ | |
| `/api/llm-models/*` | * | `llm_model/handlers/models.rs` + `downloads.rs` + `uploads.rs` | `llm_models::*` | ✅ | |
| `/api/llm-repositories/*` | * | `llm_repository/handlers.rs` | `llm_repositories::*` | ✅ | |
| `/api/local-runtime/*` | * | `llm_local_runtime/handlers.rs` + `runtime_version/handlers.rs` | `llm_local_runtime::*` | ✅ | see F-10 naming collision |
| `/api/assistants/*` | * | `assistant/handlers.rs:54-275` | `assistants::*` | ✅ | ownership-enforced |
| `/api/assistant-templates/*` | * | `assistant/handlers.rs:280-484` | `assistant_templates::*` | ✅ | |
| `/api/hub/*` | * | `hub/handlers.rs` | `hub::*` | ✅ | |
| `/api/hardware*` | * | `hardware/handlers.rs` | `hardware::read/monitor` | ✅ | |
| `/api/onboarding/*` | POST | `onboarding/handlers.rs` | `profile::read` | ✅ | uses `auth.user.id` |
| `/api/code-sandbox` | POST | `code_sandbox/handlers.rs:62` | `code_sandbox::execute` | ✅ | extractor order: auth → conv-id → body, documented |
| `/api/code-sandbox/file/download` | GET | `code_sandbox/handlers.rs:881` | `code_sandbox::execute` | ✅ | |
| `/api/code-sandbox/environments*` | GET,DELETE | `code_sandbox/handlers.rs:537+` | `code_sandbox::environments::read/manage` | ✅ | |
| `/api/code-sandbox/prefetch*` | GET,POST | `code_sandbox/handlers.rs:635+` | `code_sandbox::environments::read/manage` | ✅ | |
| `/api/code-sandbox/resource-limits` | GET,PUT | `code_sandbox/handlers.rs:1441+` | `code_sandbox::resource_limits::read/manage` | ✅ | |

**Bottom line on gating coverage:** Every state-changing route in the codebase has a permission extractor. The issues that remain are not "missing gates" but "weak gates" — permission strings that grant more than they should (F-01, F-02), missing per-resource ownership checks inside the handlers (F-04, and one in the chat module out of scope here), and the special-purpose token endpoint that intentionally has no JWT extractor (F-03).

---

## ASVS Coverage Matrix

| ASVS Req | Status | Notes |
|---|---|---|
| V1.4.1 (trusted enforcement points) | ⚠ Partial | All routes use `RequirePermissions`; however, the *opt-in* design means every new route is one PR away from being unprotected. Recommend a `#[deny(missing_permissions)]`-style lint or a router-build assertion. |
| V1.4.4 (mandatory access control) | ✅ Pass | `RequirePermissions<P>` enforces the typed permission requirement at the extractor layer, fail-closed (returns `Err(...)`, handler never runs). |
| V1.4.5 (attribute-based access control) | ⚠ Partial | Permission strings are role-ish; ownership is enforced ad-hoc in each handler. There is no centralized "is_owner_of_resource" helper. |
| V4.1.1 (each protected URL has access control) | ⚠ Partial | True for state-changing routes. Five public routes (`/health`, `/setup/status`, `/setup/admin`, `/auth/*`) are intentional. |
| V4.1.2 (positive enforcement model / fail-closed) | ✅ Pass | The Axum extractor returns `Result<Self, Err>`; on `Err`, the handler is never invoked. |
| V4.1.3 (principle of least privilege) | ⚠ Partial | The permission *taxonomy* is reasonable. However F-01/F-02 violate the principle by allowing a single edit permission to grant arbitrary further permissions. |
| V4.1.4 (deny by default at trust boundaries) | ⚠ Partial | Deny-by-default at the extractor IS in place. But the *route registration* layer is opt-in — see V1.4.1 note. |
| V4.1.5 (revoke session on permission change) | ❌ Fail | JWT is stateless and contains no `permissions_version` claim. Permission revocation requires waiting for access-token expiry. (Cross-reference auth audit HIGH-03.) |
| V4.2.1 (sensitive data and APIs protected) | ❌ Fail | F-01, F-02 allow approximate-root escalation from a non-root account; F-04 allows cross-user MCP elicitation hijack. |
| V4.2.2 (CSRF for state-changing) | N/A | Bearer-token authentication on JSON APIs; not cookie-bound. CSRF protection is not strictly required. (Cookie-based routes do not exist.) |
| V4.3.1 (admin functions exposed only to admins) | ⚠ Partial | Admin functions are gated by `users::edit` etc., not by `is_admin`. A non-root user with the right RBAC permission can perform admin actions on root admins. Add `RequireAdmin` to admin-only mutations, or add per-handler `if target.is_admin && !caller.is_admin { reject }` guards. |
| V4.3.2 (multi-factor for admin) | N/A | Not within scope of permissions module. |
| V13.1.1 (API security best practices documented) | ⚠ Partial | OpenAPI documents 403; does not always document 401 (F-15). Wildcard semantics undocumented (F-19). |
| V13.2.1 (RESTful auth verbosity) | ⚠ Partial | Error messages on 403 reveal the exact permission missing — fine for ergonomics, but worth a thought re: enumeration. F-12 is the concrete instance. |

---

## Positive Findings

1. **Typed permissions at compile time** — `PermissionCheck` trait + tuple-based `PermissionList` give compile-time guarantees about which permission a handler requires. Misspelled permission names are a compile error, not a runtime authorization bypass. This is unusually good design.

2. **Fail-closed extractor** — `RequirePermissions::from_request_parts` returns `Result`; the Axum runtime guarantees the handler is never called on `Err`. There is no "missing-extractor falls through to handler" failure mode possible by construction.

3. **`is_active` check on user account** — extractor explicitly rejects inactive users with `USER_INACTIVE` (line 105-110). Cannot be bypassed.

4. **`is_active` check on groups** — `check_permission_union` skips inactive groups (line 16-19). Cannot grant permission via a disabled group.

5. **AND-logic for multiple permissions** — `RequirePermissions<(P1, P2)>` requires the user to have ALL permissions (line 130-135). No accidental "OR" semantics that would weaken the gate.

6. **Wildcard match is strict on separator** — uses `::` exclusively in the canonical `check_permission_union`; cannot grant `users::read_secrets` via a permission like `users:re*` (the buggy `:` checks in F-09 are dead code).

7. **OpenAPI integration** — `with_permission<P>` automatically attaches a 403 response with examples per the typed permission. Documentation stays in sync with code.

8. **`User.password_hash` properly hidden** — `serde(skip_serializing)` + `JsonSchema(skip)` on `password_hash` ensures it's never returned in any handler response.

9. **PhantomData marker on `RequirePermissions`** — keeps the type parameter at compile-time only, no runtime overhead, no possibility of monomorphization bugs leaking through.

10. **Service-side ownership enforcement on chat/file/assistant resources** — handlers explicitly verify `created_by == auth.user.id` or use `get_by_id_and_user(...)` repository methods that fold ownership into the SQL query. This is the right pattern.

---

## Out of Scope / Deferred

- **JWT internals (algorithm choice, secret strength, rotation policy)** — covered by separate auth audit (`01-auth-user-permissions-audit.md`); however F-03 *does* touch the boundary contract by reusing the secret across two trust domains.
- **Per-module business logic ownership bugs** — chat (`02-chat-module-audit.md`), file (`03-file-module-audit.md`), llm (`04-llm-modules-audit.md`), mcp (`05-mcp-module-audit.md`), assistant/hub (`06-assistant-hub-audit.md`) audits cover IDOR within each module. This audit only flags **F-04** because it is also a permissions-gate weakness (the `mcp_servers::read` gate is too coarse for "respond to elicitation").
- **Frontend permission gating** — the React UI may show/hide controls based on permissions; that's not a security boundary. Backend gates are authoritative.
- **Rate limiting / DoS** — flagged at info level (F-05, F-18); design-level work, not a permissions module issue per se.
- **Audit logging** — flagged at info (F-17); cross-cuts with the auth audit.

---

## Remediation priority (suggested ordering)

1. **F-01 + F-02 + F-08** — Privilege escalation chain. Split permission-array writes into a separate permission (`*::manage_permissions`) and require root admin to mutate system groups. (1-2 days work.)
2. **F-04** — IDOR on elicitation respond. Thread `auth.user.id` through `registry::respond`. (½ day.)
3. **F-03** — Download-token JWT cryptographic isolation. Add `iss`/`aud` and HKDF-derived secret. (1 day.)
4. **F-09** — Delete the dead-code `has_permission` variants with single-colon bug. (1 hour.)
5. **F-05** — Add lightweight in-process cache + tower-governor. (1 day.)
6. **F-06 + F-07** — Fix `has_admin_access` to consider `user.is_admin` and use a specific permission constant. (2 hours.)
7. Remaining lows + infos at leisure.
