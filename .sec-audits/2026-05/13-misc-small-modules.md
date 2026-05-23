# Security Audit — Misc Small Modules (app, health, onboarding)
**Date:** 2026-05-23
**Scope:** `modules/app/` (~393 LOC) + `modules/health/` (~110 LOC) + `modules/onboarding/` (~146 LOC) — combined ~649 LOC
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Read-only review** — no source files modified; no tests executed; no `cargo`/`git`/`sqlx`/`docker`/`npm` commands run.

---

## Executive Summary

- **Combined findings by severity:** Critical: **0** · High: **2** · Medium: **4** · Low: **5** · Info: **4**
- **ASVS chapters touched:** V2 (Authentication / setup), V4 (Access Control), V5 (Validation), V6 (Stored Cryptography — by reference), V7 (Logging), V11 (Business Logic — anti-automation), V13 (API), V14 (Configuration)
- **Top risks per module:**
  1. **app** — Initial-admin setup endpoint is unauthenticated, has **no rate limiting** and a **weak password policy** (≥8 chars only, no complexity / breach-corpus check / length cap). Setup-race is well-defended by a partial unique DB index on `is_admin = true`, but a brute-force attacker who reaches the host within the operator's setup window can still trivially win the race **and** pick a guessable password. Combined with the bcrypt-cost-12 floor inherited from `modules/auth/password.rs`, the policy fails ASVS V2.1.1. [H-1, M-1]
  2. **health** — `/health` is the smallest possible safe probe (`{"status":"ok"}` only, no DB, no fingerprint). The one notable gap is **lack of a readiness counterpart** (`/health/ready` or `/health/live`): the orchestrator cannot distinguish "process is alive" from "process can serve requests". Not a vulnerability, but an availability concern flagged Info-only. No information-disclosure or DB-pressure issues found. [I-1]
  3. **onboarding** — Both endpoints rely on `array_append` without de-duplication enforcement at the DB level; the `NOT ($2 = ANY(...))` guard is correct, but **there is no upper bound on the `completed_onboarding_ids` / `completed_onboarding_step_ids` array length, and no length cap on `guide_id` / `step_id`**. An authenticated user with `profile::read` (every user has this) can append arbitrary strings — each ≤ a few KB and the array unbounded — leading to row-bloat and slow JSON serialization on every subsequent `/api/auth/me`. Authenticated DoS / storage growth. [H-2]

Both the **app** and **onboarding** modules ship the standard `AppError::database_error()` body, which propagates the raw `sqlx::Error` text to the client (cross-module finding, surfaced here as M-2). The **health** module is functionally fine — it is the cleanest of the three.

---

## Module: `app`

### Files reviewed
- `modules/app/mod.rs` (74 LOC)
- `modules/app/routes.rs` (17 LOC)
- `modules/app/handlers.rs` (107 LOC)
- `modules/app/types.rs` (22 LOC)
- `modules/app/utils.rs` (77 LOC)
- `modules/app/repository.rs` (98 LOC)

### Routes exposed
| Method | Path | Auth | Permission | Notes |
|---|---|---|---|---|
| `GET` | `/api/app/setup/status` | none | none | Returns `{needs_setup, app_name, version}` |
| `POST` | `/api/app/setup/admin` | none | none | One-shot first-admin creation; DB-locked by partial unique index |

### Findings

---

#### F-01 (H-1): Setup-admin endpoint has weak password policy + no rate limiting + no length cap
- **Severity:** High
- **ASVS:** V2.1.1 — "Verify that user set passwords are at least 12 characters in length." V2.1.7 — "Verify that passwords submitted during account registration … are checked against a set of breached passwords." V11.1.4 / V2.2.1 — "Verify that anti-automation controls are effective at mitigating breached credential testing, brute force, and account lockout attacks."
- **CWE:** CWE-521 (Weak Password Requirements), CWE-307 (Improper Restriction of Excessive Authentication Attempts), CWE-20 (Improper Input Validation — missing length cap)
- **Location:** `modules/app/utils.rs:75-77`, `modules/app/handlers.rs:48-97`
- **Description:**
  - **Password policy is ≥8 characters and nothing else** (`is_strong_password` returns `password.len() >= 8`). No complexity check, no breach-corpus lookup (HIBP `pwned`), no maximum length, no entropy estimation. ASVS L2 requires ≥12 characters and either complexity OR breach lookup; this module satisfies neither. Bcrypt's silent truncation at 72 bytes is also not handled — a 1 MB password is accepted by `validate_setup_request`, then silently truncated by `bcrypt::hash`, hashing only the first 72 bytes (`modules/auth/password.rs:5-7`, inherited).
  - **No rate limiting.** A grep for `RateLimit`, `tower-governor`, `RateLimiter` across the server source returns zero hits. An attacker who races the operator can spam this endpoint at line speed.
  - **No `Content-Length` / body-size cap on the JSON payload.** Coupled with the bcrypt cost-12 + missing password length cap, a single 10 MB password POST request would (a) consume bandwidth, (b) waste a worker thread on bcrypt for hundreds of ms, and (c) be silently truncated to 72 bytes anyway. This is a classic password-DoS gadget; multiplied across an unbounded number of attempts, it is also a CPU-exhaustion vector even before any admin is created.
- **Vulnerable code:**
  ```rust
  // modules/app/utils.rs:75-77
  pub fn is_strong_password(password: &str) -> bool {
      password.len() >= 8
  }
  ```
  ```rust
  // modules/app/handlers.rs:48-97 (abbreviated — no rate-limit extractor, no payload-size guard)
  pub async fn setup_admin(
      Extension(jwt_service): Extension<Arc<JwtService>>,
      Json(req): Json<SetupAdminRequest>,
  ) -> ApiResult<Json<AuthResponse>> {
      let has_admin = Repos.user.has_admin().await...;
      if has_admin { return Err(403); }
      validate_setup_request(&req)?;     // ≥8 chars only
      let password_hash = password::hash_password(&req.password)...;
      ...
  }
  ```
- **Exploitation:**
  1. Operator deploys server, navigates to `/setup` to create the admin. The window during which the endpoint is exposed and unauthenticated is **bounded only by operator wall-clock time** (typically seconds-to-minutes).
  2. An attacker who can reach `POST /api/app/setup/admin` during this window can race the operator with `passwordpassword` (12 chars — but here only 8 needed!) and become root admin.
  3. Even after the operator wins the race, the *password they chose* is only constrained by the ≥8 rule. The most common ≥8 passwords (`password`, `qwerty12`, `12345678`) all pass.
  4. If a CI/CD pipeline auto-runs setup at deploy time (likely deployment pattern based on the `setup/status` polling shape), the password it picks is whatever the operator put in the secrets store — but the policy provides no enforcement to prevent the operator picking a weak one.
- **Impact:** Full root-admin compromise (note: `is_admin = true` is the wildcard-permission flag — see `modules/auth/middleware/wildcard.rs` and `Administrators` group `*` in migration 1). If the attacker wins the race or guesses the password, the entire deployment is owned.
- **Recommendation:**
  1. **Raise password policy to ASVS L2:** ≥12 chars, max length ≤128 (to prevent bcrypt DoS), AND either complexity OR breached-password check. Mirror the user-creation path's policy (see `modules/user/`) so the rules are consistent.
  2. **Add a body-size limit** at the router level (`axum::extract::DefaultBodyLimit::max(N)` — 16 KB is plenty for the four small fields).
  3. **Add a per-IP rate limit on `/api/app/setup/*`** (e.g. 5 req/min/IP via `tower-governor`). The whole module is unauthenticated and one-shot; rate-limiting is essentially free.
  4. **Reject password longer than 72 bytes pre-hash** — bcrypt silently truncates, which is a footgun (CVE-2023-XXX-class). Either reject or warn explicitly.
  5. (Optional, hardening) **Bind the setup endpoint to localhost-only or to a one-time bootstrap token** delivered out-of-band (env var, container init secret) — eliminates the race window entirely.

---

#### F-02 (M-1): Setup status endpoint leaks app name + version unauthenticated
- **Severity:** Medium (lean Low — flagged because of consistency with broader threat model: the version string is a CVE-lookup primer)
- **ASVS:** V14.3.2 — "Verify that web or application server and application framework debug modes are disabled in production to eliminate debug features, developer consoles, and unintended security disclosures." V13.1.1 — "Verify that all application components use the same encodings and parsers to avoid parsing attacks that exploit different URI or file parsing behavior that could be used in SSRF and RFI attacks."
- **CWE:** CWE-200 (Exposure of Sensitive Information), CWE-1295 (Debug Messages Revealing Unnecessary Information)
- **Location:** `modules/app/handlers.rs:20-35`, `modules/app/types.rs:8-13`
- **Description:** `GET /api/app/setup/status` is **unauthenticated** and returns:
  ```json
  {"needs_setup": false, "app_name": "Ziee Chat", "version": "0.1.0"}
  ```
  - `version` is `env!("CARGO_PKG_VERSION")` — the Cargo version of the server crate. This is enough for an attacker to look up known CVEs against the published version of Ziee Chat (or any of its bundled dependencies if the version is correlated). Note that the audit observed **no commit hash, no build time, no git URL** in the response — which is the *good* part. Only the semver string is exposed.
  - `needs_setup` lets an unauthenticated probe determine whether the system has been bootstrapped. If `true`, an attacker knows the setup endpoint is *active* (see F-01).
  - `app_name` is hardcoded `"Ziee Chat"` — a fingerprint string, but not deployment-specific.
- **Exploitation:** Internet-scanning tool (Shodan, Censys) finds the endpoint, identifies the deployment as Ziee Chat vX.Y.Z, looks up vulnerabilities applicable to that version, and targets them. The `needs_setup: true` case is a *find-deployments-mid-bootstrap* primer for F-01.
- **Impact:** Reconnaissance / fingerprinting only — no direct exploit. Severity is **Medium** because (a) the response cannot be turned off / customized via config, (b) it is on a well-known unauth path, and (c) combined with F-01 it enables targeting deployments in the setup window.
- **Recommendation:**
  - Remove `version` from the unauthenticated response. If a version *must* be exposed, gate it behind an authenticated `/api/app/version` endpoint (current code has **no** such endpoint — the only version surface is here).
  - Consider returning a static `app_name` only when `needs_setup = true` (i.e. the SPA on the bootstrap page needs to render it) and dropping it on subsequent calls.
  - **Alternatively** (operator preference): keep `app_name` for the SPA welcome screen, but **gate `needs_setup`** behind a short-lived deployment-token check or a localhost-only origin check, so the world cannot probe "is this deployment bootstrappable?".

---

#### F-03 (M-2): Setup admin returns raw DB error text on unique-constraint violation (info disclosure)
- **Severity:** Medium
- **ASVS:** V7.4.1 — "Verify that a generic message is shown when an unexpected or security sensitive error occurs … and that … log entries contain details for debugging." V14.3.2 (as above).
- **CWE:** CWE-209 (Information Exposure Through Error Message)
- **Location:** `modules/app/repository.rs:60`, `modules/app/repository.rs:67`, `modules/app/repository.rs:76`, `modules/app/repository.rs:82`, `modules/app/repository.rs:91`, `modules/app/repository.rs:94` (all via `AppError::database_error`); body shape defined at `common/type.rs:109-115`.
- **Description:** Every `sqlx` error is wrapped via `AppError::database_error(err)`, which formats as `"Database error: {err}"` and is returned **verbatim** to the client through `IntoResponse` (`common/type.rs:126-138`). For `INSERT INTO users` this can return:
  - the constraint name (`users_username_key`, `users_email_key`, `unique_root_admin`),
  - the table name,
  - the colliding value (`"already exists with value '<username>'"` — depending on `sqlx` formatting).
  - For other failure paths (e.g. `Administrators` group lookup), the error text reveals internal table/column names.
  - The 503-style errors on connection loss can reveal connection-pool internals.
- **Exploitation:** Anyone hitting `POST /api/app/setup/admin` during the bootstrap window can probe the schema. Even after bootstrap (when the endpoint returns `403 SETUP_ALREADY_COMPLETE` first), this finding still applies to F-01's race — an attacker who *almost* wins the race learns DB internals from the conflict response.
- **Impact:** Schema fingerprinting, framework identification (`sqlx` formatting is distinctive), assistance for follow-on SQLi attempts elsewhere in the app.
- **Recommendation:** Map `sqlx::Error::Database` variants to **opaque** `SYSTEM_DATABASE_ERROR` bodies (no raw text) before serialization. Log the raw error server-side at WARN/ERROR via `tracing::error!` (with a correlation ID), and return only the correlation ID + a generic message to the client. This is a **cross-cutting fix** in `common/type.rs:109-115` — applying it there closes this finding here *and* matches an identical observation in audits 01-auth / 03-user / others.

---

#### F-04 (L-1): No audit log row for "first admin created" — only tracing log
- **Severity:** Low
- **ASVS:** V7.1.3 — "Verify that the application logs security relevant events including … security control activations (e.g. SCA, account creation, password change)." V7.2.1 — "Verify that all authentication decisions are logged, without storing sensitive session tokens or passwords. This should include requests with relevant metadata needed for security investigations."
- **CWE:** CWE-778 (Insufficient Logging)
- **Location:** `modules/app/handlers.rs:90-94`
- **Description:** First-admin creation emits a `tracing::info!` line with `user_id` and `username`, but writes **no row to the audit_log table** (assumed to exist; not verified for this audit). For a forensics-grade trail of "who became root and when, from where", the tracing log is fragile (rotated, possibly truncated, not queryable, no IP captured). Also note: the `tracing::info!` line records `username` but **not** the requesting IP, User-Agent, or whether this came via TLS / a proxy.
- **Exploitation:** Not directly exploitable; the gap is incident-response visibility. If an attacker compromised the bootstrap window (per F-01), the operator has no DB-level row to find them with.
- **Impact:** Forensic blind spot for the single most-critical event in the deployment.
- **Recommendation:** Write an `audit_log` row inside the transaction in `create_admin_user`, capturing `event_type = "admin.bootstrap"`, the `user_id`, the source IP (extract via `axum::extract::ConnectInfo` / `X-Forwarded-For` policy as configured), the User-Agent, and the timestamp.

---

#### F-05 (L-2): `is_valid_email` is hand-rolled and permits malformed addresses
- **Severity:** Low
- **ASVS:** V5.1.3 — "Verify that all input … is validated using positive validation."
- **CWE:** CWE-20 (Improper Input Validation)
- **Location:** `modules/app/utils.rs:37-73`
- **Description:** `is_valid_email` does basic structural validation (one `@`, local ≤64 chars, domain has a `.`, TLD ≥2 chars). It accepts addresses that are technically invalid per RFC 5321/5322:
  - `a@b.c` (TLD = 1 char fails, but `a@b.cc` passes — fine).
  - `a@.com` — domain starts with `.` — passes the current checks (`domain.contains('.')` is true, `domain_parts.len() >= 2` is true with parts `["", "com"]`, TLD = `"com"` ≥2 chars).
  - `a@b..com` — consecutive dots — passes.
  - `..@b.com` — local part with leading dots — passes.
  - Unicode normalization / IDN homograph attacks — not handled.
- **Exploitation:** Not directly exploitable; downstream code (notification email sending, e.g. via SMTP) may behave unexpectedly if it later relies on a syntactically valid address. For the *setup* endpoint specifically, it just lets a careless operator type `admin@.com` as their admin address, which then becomes the password-reset target.
- **Impact:** Operator footgun; not a security boundary by itself.
- **Recommendation:** Use a crate like `email_address` or `validator` for syntactic validation, and (optionally) verify the email with a confirmation flow before granting the admin the `*` permission.

---

#### F-06 (L-3): `display_name`, `username` have no character-set restriction (only length)
- **Severity:** Low
- **ASVS:** V5.1.4 — "Verify that structured data is strongly typed and validated against a defined schema including allowed characters, length and pattern (e.g. credit card numbers or telephone, or validating that two related fields are reasonable, such as checking that suburb and zip/postal district match)."
- **CWE:** CWE-20
- **Location:** `modules/app/utils.rs:10-16`
- **Description:** `username` is validated for length only (3-100 chars). Nothing prevents control characters, NUL bytes, spaces, slashes, or RTL Unicode in usernames. `display_name` (passed straight through) has no validation at all. The DB column is `VARCHAR(100)` for username — so DB-level rejection of >100 chars is a safety net — but byte-vs-char semantics + emoji/multibyte characters can still cause display oddities. For *root admin*, a username with embedded NUL (U+0000) bypasses some downstream filters; a username with RTL override (U+202E) can spoof identifiers in admin UIs.
- **Exploitation:** Self-spoofing — an admin chooses `admin‮XYZ` to render as `XYZ‮admin` and confuse log readers / co-admins. Low impact, but trivial to add character-set restrictions.
- **Recommendation:** Restrict usernames to `[A-Za-z0-9._-]{3,32}` (or similar) at the validator level; for `display_name`, reject control characters (`< 0x20`) and characters in the Unicode "Cf" (format) class, including `U+202E`.

---

#### F-07 (I-1): Setup endpoint trusts JSON body, no `Content-Type` enforcement
- **Severity:** Info
- **ASVS:** V13.1.5 — "Verify that REST services explicitly check the incoming Content-Type to be the expected one, such as application/xml or application/json."
- **Location:** `modules/app/handlers.rs:48-53` — uses `Json(req): Json<SetupAdminRequest>`.
- **Description:** Axum's `Json` extractor *does* check `Content-Type: application/json` by default and rejects other types with 415. This is correct; mentioned here only to confirm the behavior is in place. No action required.

---

### ASVS Coverage — `app`

| ASVS Req | Status | Notes |
|---|---|---|
| V2.1.1 (password length ≥12) | **Fail** | ≥8 only — F-01 |
| V2.1.7 (breached-password check) | **Fail** | None — F-01 |
| V2.2.1 / V11.1.4 (anti-automation) | **Fail** | No rate limit — F-01 |
| V4.1.1 (deny-by-default access control) | Pass | Setup endpoint correctly returns 403 when `has_admin = true` |
| V4.1.5 (race protection on critical state) | Pass | DB partial unique index `unique_root_admin` enforces single root admin even if F-01 race wins |
| V5.1.3 (positive validation) | Partial | Email validator naive — F-05 |
| V5.1.4 (allowed-character / pattern) | **Fail** | Username/display_name unrestricted — F-06 |
| V6.2.x (password storage — bcrypt) | Pass (inherited) | Uses `bcrypt` cost-12 — same as auth module |
| V7.1.3 (security event logging) | Partial | `tracing::info!` only — F-04 |
| V7.4.1 (generic error messages) | **Fail** | Raw DB error returned — F-03 |
| V13.1.1 (uniform encoding) | Pass | Axum + serde-json |
| V13.1.5 (Content-Type enforcement) | Pass | Axum `Json` extractor — F-07 |
| V14.3.2 (info disclosure) | **Fail** | Version + needs_setup leaked unauth — F-02 |

### Positives — `app`
- **DB-enforced single root admin.** The partial unique index `unique_root_admin ON users (is_admin) WHERE is_admin = true` (migration 1, line 32) guarantees only one row can ever have `is_admin = true`. Even if F-01's race window is exploited, the DB rejects the second insert. **This is exactly the right defense for the bootstrap race.**
- **Transactional admin creation** (`repository.rs:25-94`) wraps the user insert, the Administrators-group assignment, and the Users-group assignment in a single transaction with an in-transaction "double-check" (`SELECT EXISTS … FROM users WHERE is_admin = true` at line 28). Both belt and braces.
- **`password_hash` is `#[serde(skip_serializing)]`** on the `User` model (`modules/user/models.rs:21-23`), so the bcrypt hash is **not** echoed back in the `AuthResponse`. Good.
- **JWT issued on setup-admin response** is the same `TokenPair` shape as login — no parallel/alternate token path. Consistent.
- **No `setup_status` endpoint exposes the admin's email/username** — only the boolean. Good.
- **No commit hash / build time / git URL** in the version response — only `CARGO_PKG_VERSION`. Better than typical (`vergen`-style) deployments.
- **`needs_setup`** is computed by a single fast SQL query (`SELECT EXISTS`) — no DB pressure even if probed.

---

## Module: `health`

### Files reviewed
- `modules/health/mod.rs` (63 LOC)
- `modules/health/routes.rs` (10 LOC)
- `modules/health/handlers.rs` (31 LOC)
- `modules/health/types.rs` (10 LOC)

### Routes exposed
| Method | Path | Auth | Permission | Notes |
|---|---|---|---|---|
| `GET` | `/api/health` | none | none | Static `{"status":"ok"}` — no DB query, no internal state |

### Findings

---

#### F-08 (I-2): No `/health/ready` (readiness) probe — only `/health` (liveness)
- **Severity:** Info
- **ASVS:** N/A (operational concern, not security)
- **Location:** `modules/health/routes.rs:7-9`
- **Description:** The module exposes a single `/api/health` endpoint that always returns `200 OK` with body `{"status":"ok"}`. There is **no readiness probe** — i.e. nothing that checks whether the DB pool can serve a query, whether migrations have run, whether the JWT secret has loaded, whether the LLM provider config is valid. An orchestrator (Kubernetes, Nomad, Docker Swarm) cannot distinguish "process is alive" from "process can serve requests".
- **Exploitation:** Not exploitable.
- **Impact:** Availability — a deployment with a wedged DB pool or unloaded JWT secret will still report healthy and receive traffic, leading to 500-flood until manual intervention. Not a security vulnerability *per se*, but combined with the F-03 raw-DB-error pattern, the 500-flood becomes an information-disclosure vector.
- **Recommendation:** Add `GET /api/health/ready` that does a `SELECT 1` on the pool with a short timeout (e.g. 1 s). Keep the current `/api/health` as the liveness probe (no DB). **Do not combine the two** — a slow DB should not fail liveness (which would trigger a process restart and a thundering-herd reconnect).

---

#### F-09 (I-3): Health endpoint is unauthenticated by design — confirmed safe
- **Severity:** Info (positive observation)
- **ASVS:** V14.3.2
- **Location:** `modules/health/handlers.rs:14-22`
- **Description:** `health_check` is unauthenticated and returns a fixed `{"status":"ok"}` body — no version, no commit, no DB info, no hostname, no internal IP, no environment name (`dev`/`prod`), no module list. This is the **textbook safe shape** for an unauthenticated probe. ASVS V14.3.2 (debug/info disclosure) is satisfied.
- **Recommendation:** None. Keep it this way. **Do not** be tempted to add `version`, `commit`, `db_status`, or `uptime` fields — every one of those is an info disclosure (see F-02 in the `app` module for the same anti-pattern done badly).

---

### ASVS Coverage — `health`

| ASVS Req | Status | Notes |
|---|---|---|
| V4.1.1 (deny-by-default access control) | N/A | Unauth by design (intended public endpoint) |
| V7.4.1 (generic error messages) | Pass | Endpoint cannot fail — always returns 200 |
| V13.1.5 (Content-Type enforcement) | Pass | GET only |
| V14.3.2 (info disclosure) | Pass | Static body, no fingerprint — F-09 |
| V14.2.1 (DoS surface minimization) | Pass | No DB query, no external call, no allocation |

### Positives — `health`
- **Static-only response** — no DB, no FS, no external call. Cannot be turned into a DoS amplifier, cannot disclose anything, cannot block on a slow downstream.
- **No version / commit / hostname** in the body. As clean as it gets.
- **GET-only** — no body parsing, no Content-Type matrix to worry about.
- **No internal IP, no port, no environment** in the body.

---

## Module: `onboarding`

### Files reviewed
- `modules/onboarding/mod.rs` (50 LOC)
- `modules/onboarding/routes.rs` (18 LOC)
- `modules/onboarding/handlers.rs` (81 LOC)
- Repository methods at `modules/user/repository.rs:283-332` (cross-module — by reference)

### Routes exposed
| Method | Path | Auth | Permission | Notes |
|---|---|---|---|---|
| `POST` | `/api/onboarding/{guide_id}/complete` | JWT | `profile::read` | Append `guide_id` to `users.completed_onboarding_ids` |
| `POST` | `/api/onboarding/{guide_id}/steps/{step_id}/complete` | JWT | `profile::read` | Append `"{guide_id}/{step_id}"` to `users.completed_onboarding_step_ids` |

### Findings

---

#### F-10 (H-2): Onboarding arrays grow unbounded — authenticated DoS / storage growth
- **Severity:** High
- **ASVS:** V13.1.4 — "Verify that REST services check the incoming Content-Type to be the expected one … and reject mismatched content with appropriate response (e.g. HTTP 406 or 415)." V11.1.4 — "Verify that anti-automation controls are effective at mitigating breached credential testing … or denial of service attacks." V5.1.4 (length/pattern).
- **CWE:** CWE-770 (Allocation of Resources Without Limits or Throttling), CWE-400 (Uncontrolled Resource Consumption)
- **Location:**
  - `modules/onboarding/handlers.rs:17-35` (no length cap on `guide_id`)
  - `modules/onboarding/handlers.rs:48-70` (no length cap on `guide_id` / `step_id`)
  - `modules/user/repository.rs:283-332` (the underlying SQL uses `array_append` with no cardinality cap)
- **Description:**
  - Both handlers accept arbitrary-length URL path segments. `guide_id` and `step_id` are `String` parameters from the URL; the only validation is `.trim().is_empty()`. Axum's path-segment limit is per-segment, but PostgreSQL's `TEXT` type has no length limit, and `array_append` will happily grow the array to gigabytes if the client persists.
  - The de-dup guard (`NOT ($2 = ANY(...))`) only prevents *exact* duplicates. An attacker can submit `guide_id = "a"`, then `"aa"`, then `"aaa"`, …, indefinitely. Each call is O(n) (the `ANY` check is a linear scan), and the array grows by one element each call.
  - There is no maximum cardinality on `completed_onboarding_ids` / `completed_onboarding_step_ids`. After a few thousand calls, the user's row size exceeds the PostgreSQL TOAST threshold; after a few million, the row dominates the table.
  - Every `/api/auth/me` call returns the full `User` struct including these two arrays — so the user's *own* response grows quadratically (the array is O(n), and the user makes O(n) requests, so total bytes sent is O(n²)).
  - There is no per-endpoint rate limit (cross-cutting with F-01).
- **Vulnerable code:**
  ```rust
  // modules/onboarding/handlers.rs:17-35
  pub async fn complete_guide(
      auth: RequirePermissions<(ProfileRead,)>,
      Path(guide_id): Path<String>,        // <-- unbounded length
  ) -> ApiResult<Json<User>> {
      let guide_id = guide_id.trim().to_string();
      if guide_id.is_empty() { return Err(...); }
      // No max-length check; no allowlist; no cardinality check on user's array.
      let user = Repos.user.complete_guide(auth.user.id, &guide_id).await?;
      Ok((StatusCode::OK, Json(user)))
  }
  ```
  ```sql
  -- via modules/user/repository.rs:289-300
  UPDATE users
  SET completed_onboarding_ids = array_append(completed_onboarding_ids, $2::TEXT),
      updated_at = NOW()
  WHERE id = $1 AND NOT ($2 = ANY(completed_onboarding_ids))
  -- No cap on array.length, no cap on $2.length.
  ```
- **Exploitation:**
  1. Any authenticated user with `profile::read` (which is **every authenticated user** — it's the default permission for the `Users` group) can:
     ```
     for i in 1..N:
         POST /api/onboarding/{random_uuid_i}/complete
     ```
  2. After 10^6 calls (~hours at HTTP/2 line speed), the user's row contains a 10^6-element TEXT[]. Every `/api/auth/me`, `/api/user/profile`, etc. now returns that full array.
  3. The `array_append` + `ANY` pattern is **O(n)** per insert — at 10^6 elements, each call takes seconds; at 10^7, the UPDATE blocks long enough to time out and produce zombie rows.
  4. **Storage**: PostgreSQL row-bloat — TOAST out-of-line storage gets a 10^7-element TEXT[] which is several hundred MB per attacker user.
  5. **Egress**: Each subsequent `/api/auth/me` for that user returns the entire array — multi-MB per request → bandwidth amplification.
  6. **Concurrent attacker amplification**: The DoS is per-user, but an attacker with multiple accounts can compound it linearly.
- **Impact:**
  - **Self-DoS** (the attacker's own account becomes unusable — but they don't care).
  - **DB storage exhaustion** (each attacker burns tens to hundreds of MB at row level; PostgreSQL must vacuum the bloat).
  - **`/api/auth/me` egress amplification** for the attacker — they keep getting their multi-MB user object back.
  - **Server-side CPU exhaustion** if multiple attackers run the pattern in parallel (the linear `ANY` scan dominates).
- **Recommendation:**
  1. **Cap path segment length** at the handler level: `if guide_id.len() > 64 { return Err(...); }`. Same for `step_id`. The natural max for a guide identifier is ≤32 bytes.
  2. **Cap the array cardinality**: `WHERE … AND cardinality(completed_onboarding_ids) < 1000`. If the limit is hit, reject with `409 ONBOARDING_LIMIT_REACHED`.
  3. **Allowlist `guide_id` / `step_id`**: maintain a known list of valid guides (e.g. a `onboarding_guides` table or a hardcoded enum) and reject IDs not in the list. This converts the field from "free-form attacker-controlled string" to "constrained-vocabulary value".
  4. **Add a per-user rate limit** on these endpoints (e.g. 60 req/min/user) — at minimum, prevents the "10^6 calls per hour" attack shape.
  5. **(Out of scope but related)** trim the `User` payload returned by `/api/auth/me` so it does not include the *entire* `completed_onboarding_ids` array — paginate or move it to a separate endpoint.

---

#### F-11 (M-3): Permission `profile::read` is used for state-mutation operations
- **Severity:** Medium
- **ASVS:** V4.1.3 — "Verify that the principle of least privilege exists … users should only be able to access functions, data files, URLs, controllers, services, and other resources, for which they possess specific authorization."
- **CWE:** CWE-269 (Improper Privilege Management)
- **Location:** `modules/onboarding/handlers.rs:19`, `modules/onboarding/handlers.rs:50`
- **Description:** Both onboarding endpoints (`POST` — i.e. **state-mutating**) require permission `ProfileRead` (`profile::read`). The permission name and the documented description ("View own profile information") indicate a **read** permission; the operation is a **write**. This is a misnamed-permission / scope-mismatch finding.
  - The semantic gap matters because an admin who wants to forbid a user from *editing* their own profile (e.g. a kiosk account, a read-only audit account, an SSO-only-no-edit account) would typically grant `ProfileRead` and withhold `ProfileEdit`. With this code, they would then discover the user *can* mutate `completed_onboarding_ids` anyway.
- **Exploitation:** Low — the worst case is "a read-only user can mark guides complete on their own account". But the principle-of-least-privilege violation is real, and the fix is one-line.
- **Recommendation:** Either:
  - Use `ProfileEdit` for the onboarding endpoints (correct semantically — they mutate the profile), OR
  - Introduce a distinct permission `OnboardingEdit` (`onboarding::edit`) and grant it to the `Users` group by default.

---

#### F-12 (M-4): Onboarding endpoints return the full `User` object (incl. permissions list)
- **Severity:** Medium
- **ASVS:** V8.3.4 — "Verify that data is only sent over the wire if it is needed."
- **CWE:** CWE-213 (Exposure of Sensitive Information Due to Incompatible Policies)
- **Location:** `modules/onboarding/handlers.rs:21`, `modules/onboarding/handlers.rs:53`
- **Description:** Both `complete_guide` and `complete_guide_step` return the full `User` JSON in their 200 response. This object includes:
  - `permissions` (the user's direct permission list — but **not** the merged group-permission list; that's in `MeResponse` only),
  - `is_admin`,
  - `completed_onboarding_ids` (the full array — bytes scale with cardinality, see F-10),
  - `email_verified`,
  - timestamps,
  - the entire `completed_onboarding_step_ids` array.
  - `password_hash` is correctly omitted via `#[serde(skip_serializing)]` (`modules/user/models.rs:21-23`).
- **Exploitation:** Information disclosure is minor (the user already knows their own permissions via `/api/auth/me`). But the response shape is wasteful (multi-KB after F-10 exploit) and couples the onboarding API to the User model — every future field added to `User` (e.g. `mfa_secret_present`, `recovery_codes_hash_count`) is leaked here.
- **Impact:** Mostly architectural smell. Becomes a real issue when F-10 is exploited.
- **Recommendation:** Return either `204 No Content` (no body) or a minimal `{ completed_onboarding_ids: [...] }` projection. The full `User` belongs on `/api/auth/me`, not on a side-effect-only mutation endpoint.

---

#### F-13 (L-4): `complete_guide_step` concatenates IDs with `/` separator without escaping
- **Severity:** Low
- **ASVS:** V5.3.1 — "Verify that output encoding is relevant for the interpreter and context required."
- **CWE:** CWE-74 (Improper Neutralization of Special Elements in Output Used by a Downstream Component)
- **Location:** `modules/onboarding/handlers.rs:63`
- **Description:** The handler builds the step key with `format!("{}/{}", gid, sid)`. There is no validation that `gid` does not contain `/` itself. An attacker can submit `guide_id = "a/b"` and `step_id = "c"`, producing the key `"a/b/c"` — which collides with `guide_id = "a"` + `step_id = "b/c"`. Identical key for two different logical operations.
- **Exploitation:** A user can spoof completion of guide `"a/b"`'s step `"c"` by submitting `("a", "b/c")` (or vice versa). Trust-boundary issue inside the user's own data — not cross-user — so impact is bounded to "guides UI can be tricked into showing wrong completion state".
- **Impact:** UX bug masquerading as a security boundary. No cross-user impact.
- **Recommendation:** Reject `/` (and other path separators like `\`, `:`) in `guide_id` and `step_id` validators; or use a structured key (e.g. a JSON object stored in a separate `user_onboarding_completions` table with two columns).

---

#### F-14 (L-5): `complete_guide_step` not idempotent under race (TOCTOU on `NOT ANY`)
- **Severity:** Low
- **ASVS:** V11.1.3 — "Verify that the application will only process business logic flows for the same user in sequential step order and without skipping steps."
- **CWE:** CWE-362 (Concurrent Execution using Shared Resource with Improper Synchronization)
- **Location:** `modules/user/repository.rs:289-330`
- **Description:** Both `complete_guide` and `complete_guide_step` use `UPDATE … WHERE … NOT ($2 = ANY(...))`. PostgreSQL handles this atomically at the row level under `READ COMMITTED`, but PostgreSQL evaluates the `NOT ANY` against the row version visible at the start of the UPDATE. Two concurrent calls with the same `guide_id`:
  - Both read `completed_onboarding_ids = ['a']`.
  - Both see `NOT ('b' = ANY(['a']))` → true.
  - Both append `'b'`.
  - Final array: `['a', 'b', 'b']` (duplicate).
  - However: under `READ COMMITTED`, the second `UPDATE` re-reads the row after the first commits, so the duplicate is actually prevented in most cases. This is **PostgreSQL-version-dependent** and depends on whether the row is being concurrently updated by another transaction at the time the first writer commits.
- **Exploitation:** Trivial duplicates in the array — already bounded by F-10's recommendation to cap array cardinality.
- **Recommendation:** Either (a) deduplicate inside the query with `array(SELECT DISTINCT unnest(array_append(...)))`, or (b) use a separate `user_onboarding_completions (user_id, guide_id, completed_at)` table with a `PRIMARY KEY (user_id, guide_id)` — clean schema, no array bloat, no concurrency edge cases.

---

#### F-15 (I-4): Onboarding handlers lack DB error opacity (cross-cutting w/ F-03)
- **Severity:** Info (cross-cutting — already counted under F-03)
- **Location:** `modules/onboarding/handlers.rs:30,67` — both `await?` paths propagate raw DB errors via the cross-module `AppError::database_error` flow.
- **Description:** Same as F-03 — any unexpected DB error returns raw `sqlx::Error` text. Fixed by the same recommendation: opaque body, log internally.

---

### ASVS Coverage — `onboarding`

| ASVS Req | Status | Notes |
|---|---|---|
| V4.1.1 (deny-by-default access control) | Pass | `RequirePermissions<(ProfileRead,)>` blocks unauth/permissionless calls |
| V4.1.3 (least privilege) | **Fail** | Read permission gates write endpoints — F-11 |
| V5.1.4 (length/pattern restriction) | **Fail** | No length cap on `guide_id` / `step_id` — F-10 |
| V5.3.1 (output encoding) | **Fail** | Path-joining `format!("{}/{}", gid, sid)` unescaped — F-13 |
| V7.4.1 (generic error messages) | **Fail** | Raw DB errors propagated — F-15 / F-03 |
| V8.3.4 (minimum data exchange) | **Fail** | Returns full `User` — F-12 |
| V11.1.3 (sequential business logic) | Partial | Concurrent calls can produce duplicates — F-14 |
| V11.1.4 (anti-automation) | **Fail** | No rate limit — F-10 |
| V13.1.4 (Content-Type / payload validation) | Partial | Path-only inputs; no JSON body to validate |
| V14.2.1 (resource limit) | **Fail** | Arrays grow unbounded — F-10 |

### Positives — `onboarding`
- **Authentication is enforced** on both endpoints via `RequirePermissions` — there is no public/unauth onboarding endpoint.
- **Idempotency at the SQL level**: the `NOT ($2 = ANY(...))` guard avoids most duplicate appends. (Race edge case in F-14, but the *intent* is right.)
- **Trim + empty check** at the handler level catches the obvious empty-string case (`if guide_id.is_empty()`).
- **`user_id` is taken from `auth.user.id`**, not from the request body — so a user cannot mark a guide complete *for another user* (IDOR-resistant by construction).
- **Permissions are checked via the same extractor** as the rest of the application — no parallel auth path.
- **The handlers do not log `guide_id` or `step_id`** — which means the F-10 attacker cannot pollute the server logs as a side effect.
- **OpenAPI docs** correctly declare 401 and 400 response shapes.

---

## Cross-Module Observations

1. **No rate limiting anywhere.** `grep -rn "RateLimit\|tower-governor\|RateLimiter"` across the server source returned zero hits. This is a server-wide gap that surfaces here as F-01 (setup-admin brute force) and F-10 (onboarding array-bloat DoS). Recommendation: add `tower-governor` at the router level with per-IP (unauth endpoints) and per-user (auth endpoints) buckets.
2. **`AppError::database_error()` propagates raw `sqlx::Error` text** to clients (`common/type.rs:109-115`). Affects F-03, F-15, and matches identical findings in audit files `01-auth.md`, `03-user.md`, etc. **Single fix point** — change `database_error` to log raw error internally and return opaque message.
3. **Setup-flow design is otherwise sound.** The DB-enforced single-admin invariant (`unique_root_admin` partial index) is the right defense against the bootstrap race; the in-transaction double-check is belt-and-braces. The weak spots are policy (password strength, rate limit) and observability (no audit-log row), not architecture.
4. **Health probe is minimal and correct.** The one improvement (separate readiness endpoint) is operational, not a security gap.
5. **`completed_onboarding_ids` / `completed_onboarding_step_ids` array columns are an anti-pattern.** Migrating to a separate `user_onboarding_completions` table closes F-10, F-13, and F-14 at once.

---

## Out of Scope / Deferred

The following surfaced during the audit but lie outside the three target modules:
- **`modules/auth/password.rs`** — `bcrypt::DEFAULT_COST` (cost 12) is shared between setup and login. Already covered in `01-auth.md`. Bcrypt's 72-byte silent truncation issue (mentioned in F-01) is also a cross-cutting concern best fixed in `password.rs`.
- **`common/type.rs:109-115`** — `AppError::database_error` formatting. Covered above and in earlier audits.
- **Audit logging infrastructure** — referenced in F-04 but not implemented. Tracking whether an `audit_log` table exists and how to write to it is out of scope here; flag for the executive summary.
- **Route-level body size limits** — referenced in F-01. The fix lives at the global router builder (`core/app_builder.rs`), not in the `app` module's `routes.rs`.
- **CORS configuration** (`core/app_builder.rs:100-157`) defaults to fully permissive (`Any`/`Any`/`Any`) if no `[server.cors]` block is set. This impacts whether the unauthenticated `/api/app/setup/*` and `/api/health` endpoints can be probed cross-origin by a malicious page. Out of scope here; flagged for the core-infrastructure audit (`14-core-infrastructure.md`).
- **`MeResponse` payload size** — the `/api/auth/me` endpoint inherits the F-10 array-bloat issue but lives in the `auth` module. Cross-link only.

---

**End of audit.**
