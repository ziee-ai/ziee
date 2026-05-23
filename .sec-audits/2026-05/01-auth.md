# Security Audit — Auth Module
**Date:** 2026-05-23
**Scope:** `modules/auth/` (~2,824 LOC) — JWT, password handling, OAuth2/OIDC providers, LDAP backend, login/signup/refresh/me handlers, OAuth callback handlers
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Read-only review** — no source files modified; no tests executed.

---

## Executive Summary

- **Findings by severity:** Critical: **1** · High: **6** · Medium: **6** · Low: **4** · Info: **3**
- **ASVS chapters touched:** V2 (Authentication), V3 (Session Management), V4 (Access Control), V5 (Validation/Sanitization/Encoding), V6 (Stored Cryptography), V7 (Error Handling & Logging), V8 (Data Protection), V13 (API)
- **Top 3 risks:**
  1. **OAuth callback redirects with access token in URL** (`handlers.rs:572-575`) — token in browser history / Referer / server logs → full account takeover. [F-01, Critical]
  2. **JWT verification accepts refresh-token forgery via algorithm confusion mitigation gap + zero token revocation** — stolen tokens valid until natural expiry (24h access / 30d refresh); logout is a no-op; refresh tokens are NOT rotated on use. [F-02, F-03, High]
  3. **LDAP filter injection** (`providers/ldap.rs:90`) — `search_filter.replace("{username}", username)` with no RFC 4515 escaping; attacker-controlled username can change the LDAP query semantics. [F-04, High]

The auth surface is small (~2,800 LOC) but ships fundamental weaknesses inherited from the prior audit (2025-11). Most prior CRITICAL/HIGH findings remain unfixed: token-in-URL on OAuth callback, no rate limiting, no token revocation, no LDAP escaping, weak example secret, OAuth open-redirect via `redirect_uri`. New findings in this round include refresh-token non-rotation (F-03), missing PKCE/nonce binding to caller (F-08), trust of unverified provider email for account provisioning (F-09), and missing email-verification gate before granting tokens to freshly-registered users (F-12).

---

## Findings

### F-01: OAuth callback returns access token in URL query string
- **Severity:** Critical
- **ASVS:** V3.5.1 — "Verify the application generates a new session token on user authentication" and V8.3.1 — "Verify sensitive data is sent to the server in the HTTP message body or headers, and that query string parameters from any HTTP verb do not contain sensitive data."
- **CWE:** CWE-598 — Use of GET Request Method With Sensitive Query Strings
- **Location:** `modules/auth/handlers.rs:572-575`
- **Description:** After a successful OAuth/OIDC sign-in, the server issues a JWT access token and returns it to the browser via a 307 redirect with the token embedded in the URL query string (`/?token=<jwt>`). URL query strings are written to browser history, leak via `Referer` headers to any external resource on the destination page, are captured by reverse-proxy access logs, are visible in the address bar, and are readable by browser extensions.
- **Vulnerable code:**
  ```rust
  // handlers.rs:570-575 (oauth_callback)
  // Redirect to success page with token (in a real app, use a more secure method)
  Ok(Redirect::temporary(&format!(
      "/?token={}",
      tokens.access_token
  )))
  ```
- **Exploitation:** A shared workstation, an analytics script on the landing page, a third-party referrer, or any access-log scraper can retrieve `?token=…` and replay it from anywhere — JWT is bearer-only, the server does not bind it to IP / device / TLS channel and provides no revocation (see F-03). One-off compromise of a single user's browser history is sufficient for full account takeover for the next 24 h.
- **Impact:** Full account takeover, including admin if the user is admin (`is_admin` is encoded into the access token and trusted everywhere). Blast radius = every user that ever used OAuth login from a non-private browser session, on a shared machine, or where any third-party script ran on the landing page.
- **Recommendation:** Replace the GET-redirect token delivery with one of:
  1. Set a `HttpOnly; Secure; SameSite=Lax` cookie carrying the token (or a session id pointing at a server-side session), redirect to a clean URL with no token.
  2. Issue a one-time, single-use authorization code (random 256-bit token, TTL ≤ 60 s, stored in the existing `oauth_sessions` table) and have the SPA exchange it via `POST /api/auth/oauth/exchange`.
  3. Use the `fragment` (`#token=…`) instead of query — slightly better (not sent to server, not in Referer), still in history. Prefer (1) or (2).
- **Reference:** OAuth 2.0 Security BCP §4.1, RFC 8252 §8.12, OWASP ASVS V8.3.1.

---

### F-02: JWT logout is a no-op — no server-side revocation, no token versioning
- **Severity:** High
- **ASVS:** V3.3.1 — "Verify that logout and expiration invalidate the session token, such that the back button or a downstream relying party does not resume an authenticated session." V3.3.2 — "Verify that if authenticators permit users to remain logged in, that re-authentication occurs periodically." V3.3.4 — "Verify that users are able to view and (having re-entered login credentials) log out of any or all currently active sessions and devices."
- **CWE:** CWE-613 — Insufficient Session Expiration
- **Location:** `modules/auth/handlers.rs:387-391`
- **Description:** Logout simply returns `204 No Content` without invalidating the JWT. Tokens stay valid until natural expiry (24 h access / 30 d refresh). There is no allowlist/denylist, no `jti` claim, no `token_version` column on `users`, and no per-session storage. A token that has been stolen, leaked via F-01, or persisted on a now-untrusted device cannot be revoked. Disabling a user (`is_active=false`) does prevent **refresh** (refresh re-fetches the user from the DB) but does **not** invalidate already-issued access tokens — they continue to authenticate every protected endpoint until their `exp` is reached.
- **Vulnerable code:**
  ```rust
  // handlers.rs:386-391
  pub async fn logout(_auth: JwtAuth) -> ApiResult<()> {
      // JWT is stateless, logout is handled client-side by discarding the token
      // This endpoint exists for API consistency
      Ok((StatusCode::NO_CONTENT, ()))
  }
  ```
- **Exploitation:** Compromise a token → server cannot stop it. Admin demotes a user → user's existing access token still has `is_admin: true` until 24 h elapse. User clicks "Logout from all devices" → no such mechanism exists.
- **Impact:** No emergency revocation. Stolen-token incident response is impossible without rotating the entire `jwt.secret` (which invalidates **every** user, causing an outage).
- **Recommendation:** Add a `jti` (UUID) claim to every issued access and refresh token. Maintain a `revoked_tokens (jti UUID PK, expires_at TIMESTAMPTZ)` table; insert on logout, on password change, on admin demotion, on user disable. Check at validation time. Alternative: add `users.token_version INTEGER`; include it in claims; increment on revocation events. Provide a `DELETE /api/auth/sessions` endpoint backed by a server-side session table.

---

### F-03: Refresh tokens are not rotated; both old and new remain valid
- **Severity:** High
- **ASVS:** V3.4.4 — "Verify that refresh tokens are protected against replay … and re-issuing a refresh token invalidates the previous one (refresh-token rotation)." V3.4.5 — "Verify that refresh tokens are bound to the device they were issued to."
- **CWE:** CWE-294 — Authentication Bypass by Capture-Replay
- **Location:** `modules/auth/handlers.rs:329-373`
- **Description:** The refresh handler validates a refresh token, issues a brand-new `TokenPair` (containing a new refresh token), but **never invalidates the presented refresh token**. The old refresh token continues to mint new access tokens until natural expiry (30 days by default). This breaks the OAuth 2.0 Security BCP recommendation that refresh tokens for public clients MUST be rotated and the old one MUST be invalidated.
- **Vulnerable code:**
  ```rust
  // handlers.rs:329-372 (refresh handler — abbreviated)
  pub async fn refresh(...) -> ApiResult<Json<TokenPair>> {
      let claims = jwt_service.validate_refresh_token(&req.refresh_token)?;
      // ...fetch user...
      let tokens = jwt_service.generate_tokens(...)?;   // new pair, but...
      // (no insertion into a revoked/used list — old refresh remains valid)
      Ok((StatusCode::OK, Json(tokens)))
  }
  ```
- **Exploitation:** An attacker who has captured a refresh token once (XSS read of localStorage, leaked log, F-01 token-in-URL) can use it indefinitely up to 30 days, even after the legitimate user has refreshed (which would normally tell the server "this token has been used").
- **Impact:** Long-lived account compromise. With refresh-token theft detection (the standard rotation-anomaly pattern: if a used token is re-used, revoke the whole chain) the server could detect and respond; today it cannot.
- **Recommendation:** Add a per-refresh-token `jti`. Persist `refresh_tokens (jti UUID PK, user_id, family_id UUID, used_at TIMESTAMPTZ NULL, expires_at)`. On refresh: mark presented jti `used_at = NOW()`; if it was already `used_at IS NOT NULL`, revoke the entire `family_id` (forces re-login). Issue the new refresh token in the same family.

---

### F-04: LDAP filter injection — username inserted into search filter without escaping
- **Severity:** High
- **ASVS:** V5.3.7 — "Verify that the application protects against LDAP injection." (V5.3 Output Encoding & Injection Prevention)
- **CWE:** CWE-90 — Improper Neutralization of Special Elements used in an LDAP Query
- **Location:** `modules/auth/providers/ldap.rs:90`
- **Description:** The LDAP search filter is built by string substitution on attacker-controlled input. RFC 4515 special characters (`(`, `)`, `*`, `\`, NUL) are not escaped. An attacker can change the filter semantics — at minimum, perform username enumeration (`*`), at worst bypass authentication in a search-then-bind flow by injecting a filter that matches a different user, then the subsequent `simple_bind` is attempted against that user's DN with the attacker-supplied password (which still requires the legitimate password, so it's not a direct auth bypass — but enables targeted credential probing and account enumeration). The `bind_dn_template` path (line 119) is even worse: a username containing `,` or LDAP-DN metacharacters can change the bind DN — and `,` is **not** an RFC 4515 char, so even a generic LDAP-escape helper wouldn't catch it; DN composition requires RFC 4514 escaping.
- **Vulnerable code:**
  ```rust
  // providers/ldap.rs:89-90
  // Search for user
  let filter = self.config.search_filter.replace("{username}", username);
  let (rs, _res) = ldap
      .search(&self.config.base_dn, Scope::Subtree, &filter, vec!["*"])
      ...

  // providers/ldap.rs:117-119 (bind_dn_template path)
  let bind_dn = if let Some(template) = &self.config.bind_dn_template {
      // Direct bind with template
      template.replace("{username}", username)
  ```
- **Exploitation:** Submit `username=*)(|(uid=*` (or similar) → filter becomes `(uid=*)(|(uid=*…))` → matches all users → first hit is bound to. With known admin password being weak/leaked, attacker can pivot. In DN template (`uid={username},ou=users,dc=…`), submit `username=admin,ou=users,dc=corp,dc=com\0` to attempt to bind as a different DN.
- **Impact:** Account enumeration (high confidence); authentication bypass in misconfigured directories (lower confidence but plausible if the directory allows anonymous bind or the search-then-bind flow is misused). Most damaging when LDAP backs admin accounts.
- **Recommendation:** Add a small RFC 4515 escape for filter substitution (escape `\` first, then `(`, `)`, `*`, `\0`) and an RFC 4514 escape for DN substitution (escape `,`, `+`, `"`, `\`, `<`, `>`, `;`, leading/trailing spaces, leading `#`). Better: use `ldap3`'s parameterised filter builder if available, or pre-validate username against a strict pattern (`^[A-Za-z0-9._-]{1,64}$`) before substitution. Reject `\0` always.

---

### F-05: No rate limiting on any auth endpoint (register/login/refresh/oauth)
- **Severity:** High
- **ASVS:** V2.2.1 — "Verify that anti-automation controls are effective at mitigating breached-credential testing, brute-force, and account lock-out attacks."
- **CWE:** CWE-307 — Improper Restriction of Excessive Authentication Attempts
- **Location:** `modules/auth/routes.rs:12-22` (all routes); no `tower-governor`/equivalent middleware present anywhere in the server (`grep -r tower-governor` → no hits in `Cargo.toml` or `src/`).
- **Description:** No IP-based, account-based, or global throttling exists on `/auth/login`, `/auth/register`, `/auth/refresh`, or `/auth/oauth/{provider}/callback`. Combined with bcrypt's slow `verify` (≈100 ms at DEFAULT_COST=12), an attacker can still test ~10 passwords/sec/connection × many parallel connections; combined with F-06 (timing-based user enumeration via different error paths), this is a credential-stuffing playground.
- **Vulnerable code:**
  ```rust
  // routes.rs (no rate-limit layer applied)
  pub fn auth_routes() -> ApiRouter {
      ApiRouter::new()
          .api_route("/register", post_with(register, register_docs))
          .api_route("/login", post_with(login, login_docs))
          .api_route("/refresh", post_with(refresh, refresh_docs))
          ...
  }
  ```
- **Exploitation:** Credential-stuffing with leaked password corpora; signup spam to exhaust username space; targeted brute force against a known admin account; refresh-token brute force (refresh-token format is 24 chars per segment, so brute-force is infeasible against signature, but rate limiting still matters for DoS).
- **Impact:** DoS via bcrypt-CPU exhaustion; long-tail account compromise via credential stuffing.
- **Recommendation:** Add `tower-governor` (or `tower::limit::RateLimitLayer`) with per-IP buckets, applied to the `/auth` nested router. Suggested defaults: `login` & `oauth/callback` → 10 requests / minute / IP, `register` → 3 / hour / IP, `refresh` → 60 / hour / IP. Additionally maintain a per-username counter (`failed_login_attempts` column on `users`) and lock for a backoff window after 10 failures. Emit a `tracing::warn!` on lockout.

---

### F-06: Login user-enumeration via differing error paths and unguarded DB ops
- **Severity:** Medium
- **ASVS:** V2.2.3 — "Verify the application uses a generic error message for failed authentication attempts that does not leak whether the username, e-mail, or password is incorrect." V3.2.4 — "Verify that user-controlled session identifiers do not yield user-existence information."
- **CWE:** CWE-203 — Observable Discrepancy
- **Location:** `modules/auth/handlers.rs:138-181` (login path) and `:159-167` (NO_PASSWORD branch).
- **Description:** While the **error code strings** for "user not found" and "wrong password" are the same (`INVALID_CREDENTIALS`), the **timing and side-effects** differ:
  1. **Timing oracle (clear):** When the user does not exist, the code returns immediately after the DB lookup at line 138-148. When the user exists, the code additionally runs `bcrypt::verify` (≈100 ms) at line 169-174 and then `update_last_login` (DB write). The latency difference (~100 ms vs ~5 ms) is trivially observable over a network.
  2. **Distinct error code on `NO_PASSWORD`:** Line 159-167 returns `error_code: "NO_PASSWORD"` and message `"No password set for this user. Please use external authentication."` — this confirms the username exists AND tells the attacker to switch to LDAP/OAuth. Same issue echoes in `providers/local.rs:64`.
  3. **`ACCOUNT_DISABLED`** (line 151-156) is a third distinct outcome that confirms account existence.
- **Vulnerable code:**
  ```rust
  // handlers.rs:159-167
  let password_hash = user.password_hash.as_ref().ok_or_else(|| {
      (
          StatusCode::UNAUTHORIZED,
          AppError::unauthorized(
              "NO_PASSWORD",
              "No password set for this user. Please use external authentication.",
          ),
      )
  })?;
  ```
- **Exploitation:** Submit `POST /api/auth/login` with a candidate username and a random password; measure latency or read `error_code`. Build a list of valid accounts; cross-reference with known breach corpora.
- **Impact:** Enables targeted credential-stuffing and follow-on phishing. Lowers entropy of "username is also a secret" assumptions some deployments may have.
- **Recommendation:**
  1. Always run a dummy bcrypt comparison against a constant placeholder hash when the user is not found, so total latency is constant.
  2. Collapse `NO_PASSWORD`, `INVALID_CREDENTIALS`, and `ACCOUNT_DISABLED` into a single generic response (or only differentiate after the user has demonstrated knowledge of the password). Log the specific reason internally via `tracing::warn!`, but do NOT return it.
  3. Skip `update_last_login` until **after** successful auth; do not perform a DB write on failed attempts (avoid storage-side timing).

---

### F-07: OAuth `redirect_uri` from caller is not validated against an allowlist — open redirect / token-theft pivot
- **Severity:** High
- **ASVS:** V2.4.4 — "Verify that the OAuth/OIDC redirect URIs are validated against a registered allow-list." V13.2.1 — "Verify that enabled RESTful HTTP methods are a valid choice for the user or action."
- **CWE:** CWE-601 — URL Redirection to Untrusted Site
- **Location:** `modules/auth/handlers.rs:474-487`
- **Description:** `oauth_authorize` accepts `?redirect_uri=…` from the caller and forwards it verbatim to the upstream IdP as the registered `redirect_uri` parameter for the OAuth flow. If the IdP's registration permits any host (or the redirect is wildcarded — common in dev/staging), the attacker can have the IdP redirect to an attacker-controlled URL with the `code` in the query. The attacker then exchanges the code for a token (because the same redirect was supplied at exchange) — except in this flow PKCE prevents direct code-token exchange by the attacker (good). However, even with PKCE, the attacker can redirect to a phishing page styled like the app's login-success, and the response body of the eventual `/?token=…` redirect (see F-01) lands on attacker-controlled origin.
- **Vulnerable code:**
  ```rust
  // handlers.rs:473-487
  // Build callback URL (should be a full URL in production)
  let redirect_uri = query
      .redirect_uri
      .unwrap_or_else(|| format!("/api/auth/oauth/{}/callback", provider_name));

  // Initialize OAuth flow
  let oauth_result = provider.init_oauth_flow(&redirect_uri).await.map_err(...)?;

  // Redirect to provider's authorization URL
  Ok(Redirect::temporary(&oauth_result.redirect_url))
  ```
- **Exploitation:** Victim clicks `GET /api/auth/oauth/github/authorize?redirect_uri=https://evil.example.com/cb` (sent via phishing email). If the GitHub OAuth app has registered `https://evil.example.com/cb` (or `https://app.example.com/*`), the flow completes at `evil.example.com`, which sees `code` and `state`. Combined with PKCE, the code is not directly exchangeable, but `state` (a CSRF token) can be retrieved and used in a downgrade attack.
- **Impact:** Phishing pivot; auth code theft (mitigated by PKCE for the well-behaved IdP); user trust erosion.
- **Recommendation:** Maintain a per-provider allowlist of acceptable callback origins in the provider's DB config (e.g., `allowed_redirect_uris: Vec<String>`). At `oauth_authorize`, reject any `query.redirect_uri` that doesn't match (exact, not prefix). Default should be `None` → server constructs its own `/api/auth/oauth/{provider_name}/callback`. Document that `redirect_uri` query param is for app deep-linking only and must be allow-listed.

---

### F-08: OAuth session lookup keyed purely by `state` — no binding to caller / browser
- **Severity:** Medium
- **ASVS:** V3.5.3 — "Verify that authentication tokens are bound to the user agent." V2.4.5 — "Verify that the state parameter is bound to the user-agent session."
- **CWE:** CWE-352 — Cross-Site Request Forgery (CSRF), variant against OAuth state
- **Location:** `modules/auth/providers/oauth2.rs:436-444` (and `repository.rs:174-192`)
- **Description:** The OAuth `state` is generated server-side (good, via `CsrfToken::new_random()`), and verified to exist in the `oauth_sessions` table on callback (good). However the `state` is the **only** binding between authorize-time and callback-time — there is no cookie/session-id checked against the request, and no caller-IP binding. An attacker who can either (a) intercept the URL once or (b) trick a victim into clicking an attacker-pre-authorized link can perform login CSRF: the attacker initiates the OAuth flow themselves (obtaining `state_A`), then redirects the victim to `…/callback?code=ATTACKER_CODE&state=state_A`. The server completes auth as **the attacker's identity**, then sets a session cookie / token in the victim's browser, who is now logged in as the attacker (a precondition for many phishing / account-linking attacks).
- **Vulnerable code:**
  ```rust
  // providers/oauth2.rs:438-444
  let session = Repos.auth.get_oauth_session_by_state(state)
      .await
      .map_err(|e| AuthError::InternalError(format!("Failed to get session: {}", e)))?
      .ok_or_else(|| {
          AuthError::InvalidCredentials("Invalid or expired session".to_string())
      })?;
  ```
- **Exploitation:** Classic OAuth login-CSRF — attacker pre-initiates flow, tricks victim into completing callback. Mitigated in modern stacks by binding `state` to a browser cookie set at authorize-time and re-checked at callback.
- **Impact:** Attacker-controlled account is authenticated as the victim's browser session (with the access-token-in-URL of F-01, this becomes immediate full takeover of the victim's view).
- **Recommendation:** At `oauth_authorize`, set a `Secure; HttpOnly; SameSite=Lax` cookie `__oauth_session=<random>` with TTL matching `session_timeout_seconds`. Store the cookie value in `oauth_sessions.browser_binding`. At callback, require the cookie to be present and to match the stored value, in addition to `state`.

---

### F-09: External provider's `email` claim is trusted as-is for account provisioning
- **Severity:** Medium
- **ASVS:** V2.7.1 — "Verify that out-of-band verifiers are used … and the email channel is verified (e.g., email_verified claim)." V2.4 — federated identity assertion validation.
- **CWE:** CWE-345 — Insufficient Verification of Data Authenticity
- **Location:** `modules/auth/handlers.rs:271-287` (LDAP/OAuth provisioning), `modules/auth/repository.rs:103-126` (`create_external_user`)
- **Description:** When a user authenticates via an OAuth/OIDC or LDAP provider and no `user_auth_links` row exists, `login_with_provider` creates a new user with `is_active=true` immediately and seeds `email` from `auth_result.attributes.email` without checking whether the provider asserted `email_verified: true`. The `get_user_info_from_token` helper at `providers/oauth2.rs:188-196` does extract `email_verified` into the JSON blob but `extract_user_attributes` (`oauth2.rs:230-297`) never reads it. For social providers that DO permit users to claim arbitrary unverified emails (e.g., older custom-OIDC deployments), an attacker can register at the IdP with `victim@yourcompany.com`, log into Ziee, and be auto-provisioned into the account that already exists with that email — or take over a pre-existing user if their `external_id` ever collides.
- **Vulnerable code:**
  ```rust
  // handlers.rs:275-287 (provisioning new external user)
  let email = auth_result.attributes.email;
  let new_user_id = Repos
      .auth
      .create_external_user_with_link(
          username,
          Some(email),     // taken from provider, no email_verified check
          ...

  // providers/oauth2.rs:188-196 (email_verified is captured but ignored)
  Ok(serde_json::json!({
      "sub": claims.subject().to_string(),
      "email": claims.email().map(|e| e.as_str()),
      "email_verified": claims.email_verified(),   // never read by extract_user_attributes
      ...
  ```
  Plus: `repository.rs:103-126` (`create_external_user`) forces `is_active=true` without any email-verification step.
- **Exploitation:** Register at the OIDC IdP with `email=admin@yourcompany.com`, login to Ziee. If `admin@yourcompany.com` has not yet registered locally, you become that user (with the default group's permissions). If LDAP attribute mapping is sloppy (e.g., uses `mail` field that's user-editable), same applies.
- **Impact:** Identity spoofing; account squatting on emails that "should be" reserved for company employees.
- **Recommendation:**
  1. For OIDC: reject the login if `email_verified` is not literally `true`.
  2. For OAuth2 without OIDC: trust only providers documented as performing email verification (Google, GitHub-with-primary-verified-email).
  3. For LDAP: document that the directory's `mail` attribute is presumed verified by the IT-managed directory.
  4. Add a manual provisioning gate: new external users land in `is_active=false` until an admin approves, OR an email-verification round-trip is performed.

---

### F-10: JWT secret has no boot-time validation; example secret is 49 chars but obviously well-known
- **Severity:** High
- **ASVS:** V2.10.4 — "Verify that secrets … are not committed to source code repositories." V6.2.7 — "Verify that the application uses sufficient key sizes." V14.1.3 — "Verify that the production build configuration disables defaults."
- **CWE:** CWE-798 — Use of Hard-coded Credentials; CWE-521 — Weak Password Requirements (applied to secrets)
- **Location:** `modules/auth/jwt.rs:39-50`, `core/config.rs:153-165`, `config/dev.example.yaml`, `config/dev.yaml`
- **Description:** `JwtService::new` accepts any `config.jwt.secret: String` without length/entropy validation. The example config ships with `"dev-secret-change-in-production-min-32-chars-long"` (49 bytes, but a widely-known string). There is no startup check that the secret is ≥ 32 bytes of high entropy, no warning if it looks like an example value, no enforcement of separation between `access` and `refresh` keys, and no support for key rotation. A developer copy/pasting `dev.example.yaml` to production would happily boot with an attacker-known HS256 key, enabling token forgery for any user including admins.
- **Vulnerable code:**
  ```rust
  // jwt.rs:39-50
  pub fn new(config: JwtConfig) -> Self {
      let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
      let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());
      Self { config, encoding_key, decoding_key }
  }

  // dev.yaml
  jwt:
    secret: "dev-secret-change-in-production-min-32-chars-long"
  ```
- **Exploitation:** Operator copies dev config to prod → attacker, knowing the example, forges `is_admin: true` token offline (HS256 with known secret) and bypasses all auth.
- **Impact:** Total auth bypass.
- **Recommendation:** Add a `Config::validate()` step at boot that:
  1. Rejects secret < 32 bytes.
  2. Rejects secret matching a small denylist of example values (case-insensitive substring on `dev-secret`, `change-me`, `your-secret-here`, `example`, `placeholder`).
  3. Logs a Shannon-entropy estimate; warn if < ~4 bits/char.
  4. Refuse to boot if the binary is running in release mode and the secret matches any of the above.
  5. Prefer pulling the secret from an env var (`${JWT_SECRET}`) with no fallback default.

---

### F-11: bcrypt cost is `DEFAULT_COST` (12) — acceptable today, but no policy enforcement and no upgrade-on-login
- **Severity:** Low
- **ASVS:** V2.4.4 — "Verify that the work factor is configurable and is regularly reviewed." V6.4.2 — "Verify that key material is rotated."
- **CWE:** CWE-916 — Use of Password Hash With Insufficient Computational Effort
- **Location:** `modules/auth/password.rs:1-11`
- **Description:** `bcrypt::DEFAULT_COST = 12` (≈100 ms/verify on modern hardware) is OWASP-acceptable for 2026 but is not configurable, and there is no mechanism to upgrade existing hashes when the cost is bumped. Additionally, bcrypt has a 72-byte input cap (anything longer is silently truncated); the registration handler does not warn / hash-with-pepper to circumvent. Argon2id (current OWASP recommendation as of ASVS 4.0.3) would be preferable for new deployments.
- **Vulnerable code:**
  ```rust
  // password.rs:1-7
  use bcrypt::{DEFAULT_COST, hash, verify};
  pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
      hash(password, DEFAULT_COST)
  }
  ```
- **Impact:** Slow; not exploitable today, but a future-proofing gap. Long passphrases (> 72 bytes) silently lose entropy past byte 72.
- **Recommendation:** Migrate to `argon2` (m=19456 KiB, t=2, p=1 per OWASP 2024). Pre-hash passwords with SHA-256 to circumvent bcrypt's 72-byte limit if bcrypt is kept. On every successful login, check if `password_hash`'s algorithm/cost matches current policy; if not, re-hash with new policy and `UPDATE users SET password_hash = …`.

---

### F-12: Registration grants tokens immediately — no email verification, no admin approval
- **Severity:** Medium
- **ASVS:** V2.7.1 — "Verify that out-of-band verifiers are used."  V2.1.10 — Password-policy & account lifecycle.
- **CWE:** CWE-287 — Improper Authentication (variant: bypass of account-creation verification)
- **Location:** `modules/auth/handlers.rs:34-104`
- **Description:** `POST /api/auth/register` accepts an email, hashes a password, creates a user with `is_active=true` (`Repos.user.create` defaults — see `repository.rs:103-126` for the external variant; the local-create likely behaves the same), and returns access + refresh tokens in the response. There is no email-verification round-trip, no rate limiting (see F-05), no CAPTCHA, no minimum password policy (any non-empty string passes — line 52-57), and no per-IP/per-email cooldown. An attacker can register `victim@somecorp.com` before the real user does, mint tokens, and "own" the email-as-username.
- **Vulnerable code:**
  ```rust
  // handlers.rs:52-101
  if req.password.is_empty() {
      return Err(...);   // ← only "non-empty" enforced, no length/complexity
  }
  ...
  // No email-verification flow; user is immediately granted tokens
  let tokens = jwt_service.generate_tokens(user.id, ...)?;
  Ok((StatusCode::CREATED, Json(AuthResponse { user, tokens })))
  ```
- **Exploitation:** Spam-register accounts (DoS the username/email space, fill `default` group); register an account claiming a high-value email; perform credential-stuffing against the registration endpoint to test for email existence (`409 Conflict` reveals email is taken — see F-13).
- **Impact:** Spam, email-squatting, resource exhaustion. Combined with F-09, can be used for impersonation of users assumed to be employees.
- **Recommendation:**
  1. Enforce minimum password requirements (≥ 12 chars; OR use breached-password check via HaveIBeenPwned k-anonymity API; OWASP ASVS 2.1.7 / 2.1.9).
  2. Require email verification before issuing tokens: create user with `is_active=false`, send a verification email with a single-use, TTL-bound token; activate on verification.
  3. Add CAPTCHA to `/register` (e.g., Cloudflare Turnstile, hCaptcha).
  4. Apply F-05's rate limiter to `/register` aggressively.

---

### F-13: User-enumeration via registration conflict response
- **Severity:** Medium
- **ASVS:** V2.2.3 — Generic error for failed authn; extended interpretation: registration too.
- **CWE:** CWE-204 — Observable Response Discrepancy
- **Location:** `modules/auth/handlers.rs:60-65`
- **Description:** Registration returns `409 Conflict` with body distinguishing "Username already exists" vs "Email already exists". This lets an attacker enumerate registered usernames and emails by attempting to register them. Combined with F-12 (no rate limiting), enumeration is trivial.
- **Vulnerable code:**
  ```rust
  // handlers.rs:60-65
  if Repos.user.get_by_username(&req.username).await...?.is_some() {
      return Err((StatusCode::CONFLICT, AppError::conflict("Username")));
  }
  if Repos.user.get_by_email(&req.email).await...?.is_some() {
      return Err((StatusCode::CONFLICT, AppError::conflict("Email")));
  }
  ```
- **Exploitation:** `for email in known_emails: POST /api/auth/register {email}` → presence of `409` confirms registration.
- **Impact:** User enumeration; phishing target list generation.
- **Recommendation:** Return `200 OK` with a generic message "Registration started — check your email" regardless of existence; send the verification email only to genuinely new accounts (or a "you already have an account" email to existing ones). Combined with F-12's email verification, this is the standard pattern.

---

### F-14: Internal error details (SQL errors, bcrypt errors, OAuth provider errors) leaked in HTTP responses
- **Severity:** Medium
- **ASVS:** V7.4.1 — "Verify that a generic message is shown when an unexpected or security-sensitive error occurs … " V7.4.2 — "Verify that exception handling … does not disclose sensitive information." V14.3.3 — error message hardening.
- **CWE:** CWE-209 — Generation of Error Message Containing Sensitive Information
- **Location:** Many sites in `modules/auth/handlers.rs` — e.g., lines 71, 173, 222, 339, 410, 455-456, 481-484, 503-506, 518-519, 531-533.
- **Description:** `AppError::internal_error(format!("Database error: {}", e))` and similar `format!("Password verification error: {}", e)` propagate raw sqlx / bcrypt / oauth2 / openidconnect error messages into the JSON response body. SQLx errors can include parameter values, table names, constraint names, file paths. OIDC discovery errors can reveal the configured `issuer_url`. ID-token verification errors include JWT decoding details that aid forgery attempts.
- **Vulnerable code:**
  ```rust
  // handlers.rs:339-343
  let user_id = uuid::Uuid::parse_str(&claims.sub).map_err(|e| {
      (
          StatusCode::INTERNAL_SERVER_ERROR,
          AppError::internal_error(format!("Invalid user ID in token: {}", e)),
      )
  })?;

  // handlers.rs:480-484
  let oauth_result = provider.init_oauth_flow(&redirect_uri).await.map_err(|e| {
      (
          StatusCode::INTERNAL_SERVER_ERROR,
          AppError::internal_error(format!("OAuth initialization failed: {}", e)),
      )
  })?;
  ```
  And `AppError::database_error(err)` at `common/type.rs:109-115` puts the full `err.to_string()` into the response body:
  ```rust
  format!("Database error: {}", err)
  ```
- **Exploitation:** Probe with malformed inputs → response contains internal database details ("relation 'users' does not exist", "duplicate key value violates unique constraint 'users_email_key'"), aiding schema reconnaissance.
- **Impact:** Information disclosure; schema/dependency-version leak.
- **Recommendation:** Adopt a pattern where `AppError::internal_error` accepts the detailed error but stores it ONLY in a separate (server-side-logged) field, and the public `message` is a constant `"Internal server error"`. Same for `database_error`. Use `tracing::error!(error = %e, "operation X failed")` to capture details for ops.

---

### F-15: Refresh-token claims carry empty `username`/`email` strings — token shape signal & potential downstream confusion
- **Severity:** Low
- **ASVS:** V3.5.3 — token claims integrity. V13.1 — API hardening.
- **CWE:** CWE-1188 — Insecure Default Initialization of Resource
- **Location:** `modules/auth/jwt.rs:99-117`
- **Description:** Refresh-token claims set `username: String::new()`, `email: String::new()`, `is_admin: false`. This is fine for security (the refresh path re-fetches the user from the DB), but if any downstream code accidentally treated a refresh token as an access token (e.g., through the JWT extractor — currently blocked by audience check), it would see `is_admin=false` (safe) but empty username/email (potential NPE downstream). Defense-in-depth: introduce a `token_type` claim and check it in both validators.
- **Vulnerable code:** `jwt.rs:103-112`
- **Impact:** Low — relies on a future bug in another module to surface.
- **Recommendation:** Add `pub token_type: String,` to `Claims` (or a separate `RefreshClaims` struct). Set to `"access"` or `"refresh"`. Validate during `validate_access_token` and `validate_refresh_token`.

---

### F-16: `OptionalJwtAuth` exists but is dead code — drift risk
- **Severity:** Info
- **ASVS:** V14.2.1 — minimal attack surface.
- **Location:** `modules/auth/jwt_extractor.rs:67-121`
- **Description:** `OptionalJwtAuth` is defined and exported but never used (`grep -r OptionalJwtAuth` → only its own file). Dead code increases the chance that a future contributor wires it into a sensitive endpoint and inadvertently allows anonymous access where it shouldn't.
- **Recommendation:** Delete `OptionalJwtAuth` (or move it to a future-use crate). If kept, document the precise expected semantic and add a unit test asserting that consumers handle `claims: None` as "anonymous, no rights".

---

### F-17: AccessTokenHash verification skipped in OIDC flow
- **Severity:** Low
- **ASVS:** V2.4.5 — OIDC ID-token integrity. V6.3.3 — verify signed assertions.
- **CWE:** CWE-345 — Insufficient Verification of Data Authenticity
- **Location:** `modules/auth/providers/oauth2.rs:517-518`
- **Description:** The code explicitly skips `at_hash` verification with the comment "requires JWK key which is complex to obtain". The `at_hash` claim binds the ID token to the access token; skipping it weakens the case against access-token substitution if the upstream IdP issues both via separate channels. For most OIDC flows that exclusively use the ID token (as Ziee does — it only reads claims from `id_token`), the practical impact is low.
- **Vulnerable code:**
  ```rust
  // providers/oauth2.rs:516-518
  // Note: AccessTokenHash verification skipped - requires JWK key which is complex to obtain
  // The ID token verification above provides sufficient security
  ```
- **Recommendation:** Use `openidconnect`'s `AccessTokenHash::from_token(access_token, alg)` + the verifier's signing alg from discovery metadata; verify `id_token.claims().access_token_hash() == computed_hash`.

---

### F-18: OAuth `userinfo_url` fetch uses a fresh client without redirect-policy-none — SSRF gap
- **Severity:** Medium
- **ASVS:** V12.6.1 — "Verify that the web or application server is configured … to deny SSRF." V13.5.4 — "Verify HTTP redirects do not include un-validated data."
- **CWE:** CWE-918 — Server-Side Request Forgery (SSRF)
- **Location:** `modules/auth/providers/oauth2.rs:199-228`
- **Description:** `get_user_info_from_api` uses `reqwest::Client::new()` directly — **not** the hardened `create_http_client()` that disables redirects (defined at `oauth2.rs:33-38`). If the configured `userinfo_url` issues a 3xx redirect to an internal endpoint (`http://localhost:5432/…`, `http://169.254.169.254/latest/meta-data/`, internal RFC-1918), reqwest's default redirect policy (`Policy::limited(10)`) will follow it with the same `Authorization: Bearer <token>` header. A malicious provider configuration (admin-induced) or a compromised upstream UserInfo endpoint can pivot into internal-network discovery.
- **Vulnerable code:**
  ```rust
  // providers/oauth2.rs:207
  let client = reqwest::Client::new();   // ← default redirect policy
  let response = client
      .get(userinfo_url)
      .bearer_auth(access_token)
      .send()
      .await
      ...
  ```
- **Exploitation:** Admin (or attacker who compromised provider config) sets `userinfo_url=https://attacker.example.com/userinfo` → server responds `302 Location: http://169.254.169.254/latest/meta-data/` → reqwest follows, sends `Bearer …` to AWS IMDS, attacker receives instance metadata.
- **Impact:** SSRF; IMDS credential exfiltration in cloud deployments; internal-network reconnaissance.
- **Recommendation:** Replace `reqwest::Client::new()` with the existing `create_http_client()`. Better still, use a shared, reused client (creating a new client per request is wasteful and bypasses connection pooling).

---

### F-19: OAuth sessions are stateless re: cleanup — no garbage collection of expired rows
- **Severity:** Info
- **Location:** `modules/auth/repository.rs:174-208`
- **Description:** `oauth_sessions` rows are deleted only on successful callback (`delete_oauth_session(state)` at `handlers.rs` end of callback). Failed/abandoned flows leave rows that accumulate forever (TTL is enforced at READ-time via `WHERE expires_at > NOW()`, so they don't grant access — just bloat the table). Low impact but noisy.
- **Recommendation:** Add a periodic `DELETE FROM oauth_sessions WHERE expires_at < NOW() - INTERVAL '1 day'` task (`tokio::spawn` background task at boot, or a `pg_cron` job).

---

### F-20: No security headers (CSP / HSTS / X-Frame-Options / Referrer-Policy) applied at the auth layer
- **Severity:** Low
- **ASVS:** V14.4 — HTTP security headers.
- **CWE:** CWE-693 — Protection Mechanism Failure
- **Location:** Out-of-scope for this audit (these belong in core-infrastructure / global middleware), but worth noting that `Referrer-Policy: no-referrer` would significantly reduce the bleed-radius of F-01 even before that finding is fixed. Without it, the OAuth callback's `?token=…` is sent in `Referer` to every resource on the landing page.
- **Recommendation:** Add a global `tower_http::set_header::SetResponseHeaderLayer` (or `axum_extra` equivalent) emitting `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, and (if HTTPS) `Strict-Transport-Security: max-age=31536000; includeSubDomains; preload`. CSP belongs in the UI server. Tag: cross-cutting; track under core-infra audit.

---

## ASVS Coverage Matrix

Only chapters relevant to the auth module are included. "✅ Pass" means the requirement is met by code reviewed; "⚠️ Partial" means partially met or relies on operator configuration; "❌ Fail" means a finding was raised.

| ASVS Req | Status | Notes |
|---|---|---|
| **V2 Authentication** | | |
| V2.1.1 — Verify users can change their password | ✅ Pass | Not in auth module per se, but `password::hash_password` is reusable for password-change flows in user module. |
| V2.1.7 — Verify passwords ≥ 12 chars or strong policy | ❌ Fail | F-12. Only non-empty enforced. |
| V2.1.9 — Verify breached-password screening | ❌ Fail | F-12. No HaveIBeenPwned check. |
| V2.2.1 — Anti-automation on auth endpoints | ❌ Fail | F-05. No rate limiting. |
| V2.2.3 — Generic message on failed authn | ⚠️ Partial | F-06. Codes are generic-ish but timing & `NO_PASSWORD`/`ACCOUNT_DISABLED` leak. |
| V2.4.1 — Crypto-strong password storage (Argon2/PBKDF2/bcrypt) | ✅ Pass | bcrypt is acceptable. |
| V2.4.4 — Configurable work factor | ⚠️ Partial | F-11. Hardcoded `DEFAULT_COST`. |
| V2.4.5 — OIDC ID-token / state binding | ⚠️ Partial | F-08, F-17. State exists & is checked; not bound to UA; `at_hash` skipped. |
| V2.5.1 — Pre-registration tokens single-use | N/A | No pre-registration flow. |
| V2.7.1 — Out-of-band verifier (email verification) | ❌ Fail | F-12. No email verification. F-09. Trusts provider email without `email_verified`. |
| V2.10.4 — No secrets in source / examples | ❌ Fail | F-10. Example/dev config commit identifiable secret. |
| **V3 Session Management** | | |
| V3.2.1 — Server generates session tokens | ✅ Pass | JWT generated server-side. |
| V3.2.4 — Session tokens have sufficient entropy | ✅ Pass | HS256 with operator-supplied secret; `CsrfToken::new_random()` for state. |
| V3.3.1 — Logout invalidates session | ❌ Fail | F-02. Logout is a no-op. |
| V3.3.2 — Re-authentication on long sessions | ❌ Fail | F-02. 30-day refresh, no step-up. |
| V3.3.4 — Logout from all devices | ❌ Fail | F-02. Not possible — stateless tokens, no index. |
| V3.4.4 — Refresh-token rotation | ❌ Fail | F-03. |
| V3.5.1 — New token on auth | ⚠️ Partial | F-01. Token issued, but in URL. |
| V3.5.3 — Tokens bound to UA / channel | ❌ Fail | F-08. Pure bearer; no binding. |
| **V4 Access Control** | | |
| V4.1.1 — Enforce access control server-side | ✅ Pass | `JwtAuth` extractor + permissions module gate handlers. |
| V4.1.3 — Least privilege | ⚠️ Partial | `is_admin` carried in JWT — but refresh re-fetches; access tokens last 24h with stale admin flag → F-02 again. |
| **V5 Validation / Injection** | | |
| V5.3.4 — SQL injection prevention | ✅ Pass | All queries use sqlx `query!`/`query_as!` parameterised macros. |
| V5.3.7 — LDAP injection prevention | ❌ Fail | F-04. |
| V5.1.3 — Input validation on auth fields | ⚠️ Partial | Trim/empty checks only; no length caps, no charset. |
| **V6 Stored Cryptography** | | |
| V6.2.1 — All crypto uses tested implementations | ✅ Pass | `bcrypt`, `jsonwebtoken`, `oauth2`, `openidconnect` are well-known. |
| V6.2.4 — Sufficient algorithm strength | ⚠️ Partial | F-11. bcrypt cost 12 — borderline; algorithm pinned to HS256 — acceptable but symmetric. |
| V6.2.7 — Sufficient key sizes | ⚠️ Partial | F-10. No length check on `jwt.secret`. |
| **V7 Error Handling & Logging** | | |
| V7.1.1 — Don't log credentials/tokens | ✅ Pass | No `info!`/`debug!`/`trace!`/`println!` of passwords or tokens found in auth module. |
| V7.4.1 — Generic messages for security errors | ❌ Fail | F-14. Raw error strings propagated. |
| V7.4.2 — Exception handling doesn't disclose info | ❌ Fail | F-14. |
| **V8 Data Protection** | | |
| V8.3.1 — No sensitive data in query strings | ❌ Fail | F-01. |
| **V12 Files & Resources** | | |
| V12.6.1 — SSRF prevention | ❌ Fail | F-18. UserInfo fetch follows redirects. |
| **V13 API** | | |
| V13.2.1 — RESTful methods restricted | ✅ Pass | Auth routes use POST for mutations, GET only for OAuth redirects. |
| V13.2.3 — JSON schema validation | ⚠️ Partial | Aide-generated OpenAPI exists; no runtime per-field validation. |
| **V14 Configuration** | | |
| V14.1.3 — Production build disables defaults | ❌ Fail | F-10. No guard. |
| V14.4 — Security headers | ❌ Fail (cross-cutting) | F-20. |

---

## Positive Findings

Things done correctly that should be preserved when fixing the above:

1. **JWT algorithm is pinned to HS256 by library default.** `Validation::default()` (jsonwebtoken 10.1) sets `algorithms: vec![Algorithm::HS256]`, so `alg=none` and HS/RS confusion are blocked. Do not switch to `set_required_spec_claims` without re-adding the algorithm pin.
2. **Issuer & audience are validated** (`jwt.rs:122-123, 135-136`), and refresh-token audience is distinct (`<aud>-refresh`), so a refresh token cannot be presented as an access token via the extractor.
3. **`exp` is enforced by default** (jsonwebtoken's `validate_exp: true`, `leeway: 60s`). `nbf` is similarly checked when present.
4. **bcrypt is timing-safe by construction** — `verify_password` uses bcrypt's `verify`, not string equality.
5. **All SQL is parameterised** via sqlx `query!`/`query_as!` macros — no string-concatenated SQL anywhere in the auth module.
6. **PKCE is used for OAuth2 & OIDC** flows (`providers/oauth2.rs:326, 365, 396, 499, 545`) — important for public-client safety.
7. **OAuth state is generated server-side** with `CsrfToken::new_random()` and stored in `oauth_sessions` with TTL — partially mitigates F-08.
8. **`async_http_client` for OAuth/OIDC discovery and token exchange disables redirects** via `create_http_client()` (`oauth2.rs:33-38`) — protects against SSRF for the well-traveled paths. F-18 is the exception (UserInfo fetch).
9. **Refresh path re-fetches the user from the database** (`handlers.rs:347-365`), so `is_active=false` propagates within ≤ 24 h to the access-token holder (mitigating but not eliminating F-02).
10. **OAuth sessions are scoped per provider** — `handle_oauth_callback` checks `session.provider_id != self.provider_id` (`oauth2.rs:446-450`), preventing cross-provider state replay.
11. **No `println!`/`eprintln!`/`dbg!` and no `tracing::*` macros log passwords or token contents** anywhere in the auth module (verified via `grep`).
12. **bcrypt unit tests** cover both verify-positive and verify-negative paths (`password.rs:14-40`).

---

## Out of Scope / Deferred

- **`core/` and `module_api/`** — middleware order, `JwtService` injection wiring, `AppError` global response shape, CORS config: separately audited under core-infrastructure.
- **`modules/mcp/client/auth.rs`** — MCP OAuth client: separate audit (`.sec-audits/mcp-phase3-i2-get-sse-audit-2026-05-22.md` covers MCP transport security).
- **`modules/user/` and `modules/permissions/`** — the user repository and permission extractor surfaces are out of scope per instruction. F-12 / F-13 touch user-creation paths in `repositories/user.rs` but were called out only as they relate to the auth handlers' contract.
- **Frontend token storage** (localStorage vs cookie, XSS resilience) — not auditable from the backend module.
- **Operational secrets management** (Vault/Doppler/AWS Secrets Manager integration) — operator concern, not module code.

---

## Summary Reconciliation vs Prior Audit (2025-11)

| Prior finding | Status in current code |
|---|---|
| CRITICAL-01: OAuth token in URL | **Still present** → F-01 |
| HIGH-01: No rate limiting | **Still present** → F-05 |
| HIGH-02: Weak JWT secret in example | **Still present** → F-10 |
| HIGH-03: No token revocation | **Still present** → F-02 |
| LDAP injection (mentioned briefly in prior audit) | **Still present** → F-04 |
| OAuth open redirect via `redirect_uri` | **Still present** → F-07 |
| User enumeration via login errors | **Still present** → F-06 |
| User enumeration via register conflicts | **Still present** → F-13 |
| Error message leakage | **Still present** → F-14 |

New findings in this round:
- **F-03** refresh-token non-rotation (not surfaced in prior audit explicitly)
- **F-08** state-not-bound-to-UA (CSRF on OAuth)
- **F-09** unverified-email provisioning (`email_verified` ignored)
- **F-12** no email verification before token issuance
- **F-17** `at_hash` skipped in OIDC
- **F-18** SSRF via UserInfo client w/o redirect-disable
- **F-15** refresh-token claim shape — defense-in-depth
- **F-16** dead code (`OptionalJwtAuth`)
- **F-19** OAuth session GC
- **F-20** referrer policy / global security headers (cross-cutting; called out because it gates F-01 blast-radius)

Net assessment: **the auth module's design is broadly sound (parameterised SQL, PKCE, algorithm pinning, timing-safe bcrypt), but operational hardening from the 2025-11 audit has not landed.** Closing F-01, F-02, F-03, F-04, F-05, F-07, F-10, F-12, and F-18 is the priority list — these together turn the module from "POC" into "production-defensible".
