# Security Audit — LLM Provider Modules
**Date:** 2026-05-23
**Scope:** `modules/llm_provider/` + `modules/llm_provider_files/` (~2,195 LOC)
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target

---

## Executive Summary

This audit re-evaluates the LLM provider configuration module and its
companion provider-files mapping module against the 2025-11 baseline
(`.sec-audits/04-llm-modules-audit.md` CRIT-1, plus several Highs).

**The headline finding is unchanged:** the 2025-11 Critical "API keys
exposed in responses" is **STILL ACTIVE** — and is in fact reinforced
by the integration test suite, which explicitly asserts that the
plaintext `api_key` echoes back on every list/get/create/update
response. Two separate user-facing endpoints (`/chat/llm-providers`
and `/user-llm-providers`) inline the full `LlmProvider` struct via
`#[serde(flatten)]`, both reachable by any account that holds the
default `Users` group permissions (`conversations::read` +
`user_llm_providers::read`). The system-level API key (a single
shared OpenAI / Anthropic / etc. credential potentially worth real
money) is therefore retrievable by every authenticated tenant.

The user-keyed `user_llm_provider_api_keys` table — added since the
2025-11 audit (migration 28) — partially mitigates the problem for
deployments that disable system keys, but does not close the leak,
because (a) administrators who *do* configure a system key still hand
it out to every user, and (b) the per-user keys are themselves stored
in plaintext at rest.

**Severity counts**

| Severity | Count |
|---|---|
| Critical | 2 |
| High     | 4 |
| Medium   | 7 |
| Low      | 5 |
| Info     | 4 |

**Top-3 risks**

1. **F-01 (Critical).** System provider `api_key` returned verbatim
   to every authenticated user via `LlmProvider` / `ProviderWithModels`
   responses; an attacker who phishes any low-privileged account
   walks away with shared paid-API credentials.
2. **F-02 (Critical).** Per-user `user_llm_provider_api_keys.api_key`
   and admin `llm_providers.api_key` columns are stored in plaintext
   (`TEXT`, no encryption, no KMS, no hashing) and silently returned
   from `get()`/`list_local_providers()` to any caller that has the
   repository wired in. Database compromise = credential blast.
3. **F-03 (High).** SSRF / file: scheme not blocked. Admins who add a
   custom provider can point `base_url` at
   `http://169.254.169.254/latest/meta-data/`, `http://localhost:5432/`,
   or `file:///etc/passwd` — the validator only checks
   `reqwest::Url::parse` succeeds. Combined with reqwest's default
   redirect-follow policy and the absence of any IP-literal /
   private-range allow-list, a malicious admin (or a stolen admin
   token) gains unauthenticated cloud-metadata access via the
   server's egress.

The remaining 8 findings (M/L/I) cover stored-credential hygiene
(`proxy_settings.password` round-trip, TLS-validation toggle that is
plumbed-but-not-enforced, missing rate limits, missing max-length
caps, weak error logging via `eprintln!` instead of structured
`tracing`, and a few defense-in-depth gaps in
`llm_provider_files`).

---

## Findings

---

### F-01 — System provider `api_key` returned in API responses (CRITICAL, OPEN — was 2025-11 CRIT-1)

* **Severity:** Critical
* **ASVS:** V6.2.1 (sensitive data must be protected when transmitted),
  V8.2.2 (sensitive data must not be exposed in API responses),
  V4.2.1 (least privilege)
* **CWE:** CWE-200 (Exposure of Sensitive Information), CWE-359
  (Exposure of Private Personal Information), CWE-522 (Insufficiently
  Protected Credentials)
* **Status:** **OPEN.** Identical to 2025-11 CRIT-1, with one
  aggravating change: the user-facing `/chat/llm-providers` and
  `/user-llm-providers` endpoints now exist and both serve the
  vulnerable struct to non-admin users.

**Location**

* `src/modules/llm_provider/models.rs:28-45` — the `LlmProvider`
  struct derives `Serialize` with `api_key: Option<String>` and only
  uses `#[serde(skip_serializing_if = "Option::is_none")]`, which
  skips the field when *null* but **emits the plaintext key** when
  one is configured.
* `src/modules/llm_provider/handlers/admin.rs:80-95` (`get_provider`),
  `:36-67` (`list_providers`), `:110-130` (`create_provider`),
  `:144-169` (`update_provider`) — all return `Json<LlmProvider>` or
  `Json<LlmProviderListResponse>` directly.
* `src/modules/llm_provider/handlers/user.rs:23-67`
  (`get_user_llm_providers`) — wraps the same struct in
  `ProviderWithModels` via `#[serde(flatten)]` (`types.rs:82-88`).
* `src/modules/chat/core/handlers/providers.rs:23-62` — duplicate
  user-facing endpoint `/chat/llm-providers`, same flattened struct
  (`chat/core/types/providers.rs:9-13`).
* `src/modules/llm_provider/repositories/admin.rs:101-194`,
  `:425-503` — every read path (`get_by_id`, `list`,
  `list_local_providers`, `get_providers_for_group`,
  `get_providers_for_user`) selects `api_key` into the in-memory
  struct.

**Vulnerable code (models.rs:28-45)**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProvider {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,        // <-- plaintext leak
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub built_in: bool,
    pub proxy_settings: ProxySettings,  // <-- includes password field
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_runtime_version_id: Option<Uuid>,
}
```

**Vulnerable code (user.rs:25-67) — user-facing path**

```rust
pub async fn get_user_llm_providers(
    auth: RequirePermissions<(UserLlmProvidersRead,)>,
) -> ApiResult<Json<GetUserProvidersResponse>> {
    let user_id = auth.user.id;
    let providers = Repos.llm_provider.get_for_user(user_id).await?;
    let mut providers_with_models = Vec::new();
    for provider in providers {                        // <-- raw LlmProvider
        ...
        providers_with_models.push(ProviderWithModels {
            provider,                                  // <-- flattened into JSON
            llm_models: enabled_models,
            api_key_configured,                        // <-- redundant when api_key itself leaks
        });
    }
    ...
}
```

**Default-permission reach**

`migrations/00000000000027_fix_default_user_permissions.sql:15` puts
`user_llm_providers::read` on the system `Users` group. Every regular
account in a fresh install can:

```text
GET /user-llm-providers           # 200 OK, JSON contains "api_key":"sk-…"
GET /chat/llm-providers           # 200 OK, same payload via chat module
```

Admin endpoints (`/llm-providers/*`) require `llm_providers::read`
which is admin-scoped, so they're "only" a privilege-escalation
extender; the user endpoints are the catastrophic ones.

**Exploitation (5 lines, no admin needed)**

```bash
TOKEN=$(curl -s -X POST $HOST/api/auth/login \
        -d '{"username":"victim","password":"…"}' | jq -r .access_token)
curl -s -H "Authorization: Bearer $TOKEN" $HOST/api/user-llm-providers \
    | jq -r '.providers[].api_key' | grep -v null
# → prints every system provider's plaintext key
```

**Tests enforce the leak**

`tests/llm_provider/mod.rs:313-417` and `:1300-1333` explicitly assert
that the plaintext `api_key` round-trips through the response. The
existing test suite will *fail* the moment this is fixed correctly,
so any patch must also rewrite those tests to assert *absence*
instead of presence.

**Impact**

* Per-tenant exfiltration of paid-API credentials worth O($100-$10k)
  per month each.
* If any deployment configures a workspace-scoped Anthropic key, the
  attacker also gets file/library access through Anthropic Files
  API (combined with finding F-07 below: anything an admin uploaded
  via `llm_provider_files` is now reachable via the leaked key).
* The OpenAPI / `types.ts` (`api-client/types.ts:798, 1191, 1207,
  1616`) advertises `api_key?: string` as a published response field,
  so the leak is also discoverable from the generated docs without
  blackbox probing.

**Recommendation**

1. **Stop serializing `api_key` from the read path.** Either:
   * (Preferred) Introduce a separate read-DTO
     `LlmProviderResponse` without `api_key`, and a separate
     write-only DTO that accepts (but never echoes) it. Mark the
     `LlmProvider` struct as `pub(crate)` so it can never leave the
     module by accident.
   * Or, at minimum, add `#[serde(skip_serializing)]` on `api_key`
     (drop the `Deserialize` allowance — make it a write-only
     field). Combine with a separate `system_api_key_configured:
     bool` flag for UI display (the existing
     `api_key_configured` flag on `ProviderWithModels` is already
     designed for this purpose — it's just shadowed by the raw
     `api_key` leak).
2. Audit every `get_*` repository function and the two flatten sites
   (`ProviderWithModels` in both `llm_provider/types.rs` and
   `chat/core/types/providers.rs`).
3. Rewrite the 6 affected integration tests to assert
   `body.get("api_key").is_none()` (currently they assert the
   opposite).
4. Add a *guardrail* unit test that does
   `serde_json::to_value(provider).get("api_key")` and panics if a
   key surfaces — this catches future regressions where a developer
   re-adds the field.

---

### F-02 — Provider API keys stored in plaintext (CRITICAL, OPEN)

* **Severity:** Critical
* **ASVS:** V6.2.1 (cryptographic protection of secrets at rest),
  V6.2.5 (encryption keys must be managed), V6.4.2 (no plaintext
  storage of secrets)
* **CWE:** CWE-256 (Plaintext Storage of a Password), CWE-312
  (Cleartext Storage of Sensitive Information), CWE-798 (Use of
  Hard-coded Credentials — by extension, of unwrapped secrets)

**Location**

* `migrations/00000000000003_create_llm_providers_table.sql:9` —
  `api_key TEXT,` no encryption, no `pgcrypto`, no envelope.
* `migrations/00000000000028_create_user_provider_api_keys.sql:5` —
  `api_key TEXT NOT NULL,` same.
* `src/modules/llm_provider/repositories/user.rs:21-39` (`get`),
  `:42-63` (`upsert`), `:115-131` (`has_key`) — keys are passed
  through unmodified to/from the DB.
* `src/modules/llm_provider/repositories/admin.rs:204-235`
  (`create_llm_provider`), `:274-283` (`update_llm_provider`).

**Vulnerable code (`repositories/user.rs:42-63`)**

```rust
pub async fn upsert(
    &self,
    user_id: Uuid,
    provider_id: Uuid,
    api_key: &str,                       // <-- plaintext arrives
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO user_llm_provider_api_keys (user_id, provider_id, api_key)
        VALUES ($1, $2, $3)              -- <-- plaintext stored
        ON CONFLICT (user_id, provider_id)
        DO UPDATE SET api_key = EXCLUDED.api_key, updated_at = NOW()
        "#, ...
    )
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

**Exploitation**

* Anyone with a DB read replica (analytics, backups, dev clones,
  SQL-injection in *any unrelated module*, malicious DBA, container
  escape) walks away with all tenants' keys in cleartext. There is no
  defense-in-depth at the storage layer.
* Backups inherit the plaintext: `pg_dump` of a production database
  is, by itself, a credential-disclosure event.

**Impact**

* Aggregated severity is critical because both tables are involved
  and there is no second factor: one read, all keys.
* Combined with F-01: an attacker that snags a single user account
  gets all *system* keys via the API; an attacker that gets a DB
  dump gets all *user* keys also.

**Recommendation (ordered, minimum → maximum)**

1. **Minimum (this sprint):** wrap `api_key` columns with
   `pgcrypto`'s `pgp_sym_encrypt`/`pgp_sym_decrypt`, keyed by a
   server-held secret loaded from env or a sealed file. This is a
   2-file patch (migration + repository) and immediately neutralizes
   `pg_dump`-style exfiltration.
2. **Better (next sprint):** introduce a dedicated KMS-backed
   `SecretsStore` trait (file-key fallback for dev, AWS KMS / GCP
   KMS / Vault Transit in prod). Each row stores
   `(kid, nonce, ciphertext)` so keys can be rotated.
3. **Best (long-term):** use envelope encryption (DEK per row, KEK
   in KMS) so even an attacker with a DB dump + server memory
   snapshot has to break KMS too.
4. Add a one-shot migration script that re-encrypts existing rows
   on first boot under the new KEK; do *not* commit the migration
   itself to the audit log (it would expose old ciphertext).
5. Document the threat model in `BACKEND_ARCHITECTURE.md` so
   downstream consumers (chat, MCP) explicitly request a
   `SecretView<String>` instead of a raw `String`, making it
   impossible to accidentally `Display` or `Debug` a key.
6. Add a `Drop`-zeroising newtype `ApiKey(SecretString)` (e.g. via
   the `secrecy` crate) — `LlmProvider.api_key: Option<ApiKey>`
   makes the F-01 leak path impossible at the type level (no
   `Serialize` impl).

---

### F-03 — SSRF on provider `base_url` (HIGH, OPEN)

* **Severity:** High
* **ASVS:** V12.6.1 (validate URLs against an allowlist), V9.2.1
  (outbound calls must be controlled)
* **CWE:** CWE-918 (SSRF), CWE-601 (Open Redirect — secondary)

**Location**

* `src/modules/llm_provider/utils.rs:31-41` — `validate_base_url`.
* `src/modules/llm_provider_files/service.rs:102-117` — outbound use.
* `ai-providers/src/provider.rs:110-113` — the shared reqwest client
  is built with default redirect policy (≤10 hops) and default DNS.

**Vulnerable code (`utils.rs:31-41`)**

```rust
pub fn validate_base_url(base_url: &Option<String>) -> Result<(), AppError> {
    if let Some(url) = base_url {
        if !url.is_empty() && reqwest::Url::parse(url).is_err() {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "Invalid base URL format",
            ));
        }
    }
    Ok(())
}
```

`Url::parse` accepts `file://`, `http://169.254.169.254`,
`http://localhost`, `http://127.0.0.1`, `http://[::1]`,
`http://10.0.0.5`, `http://internal.consul`, and DNS names that
resolve to internal IPs.

**Exploitation paths**

1. **AWS / GCP metadata theft.** Admin (or an attacker who phished
   one) submits a custom provider with `base_url:
   "http://169.254.169.254/latest/meta-data/iam/security-credentials/"`.
   First chat or file-upload to that provider issues an outbound GET
   from the server's IAM context. The IMDS response is then returned
   as a "provider error" body in the streaming response (or saved
   into `provider_metadata` in `llm_provider_files`).
2. **Localhost port scan / RCE pivot.**
   `base_url: "http://127.0.0.1:5432/"` (Postgres),
   `http://127.0.0.1:6379/` (Redis), `http://127.0.0.1:8500/`
   (Consul), `http://127.0.0.1:9200/` (Elasticsearch). Provider
   errors will leak banners/connection-refused timing, enabling
   internal-service enumeration.
3. **File scheme read.** `file:///etc/passwd`. reqwest does *not*
   support `file://` by default at the HTTPS layer, but some
   transports (custom h3, http3) and some downstream provider
   implementations may. Even if it currently fails, this is one
   reqwest-version-bump away from being exploitable; an allowlist
   is the structural fix.
4. **DNS rebinding.** `base_url: "http://attacker.example/"` where
   `attacker.example` resolves to a public IP at validation time and
   `127.0.0.1` at request time. No `resolve_and_pin` is done.
5. **Open-redirect chain.** reqwest follows redirects by default (10
   hops). A public-looking host can 302 to a metadata URL on the
   first hop. The auth module's reqwest client *explicitly* sets
   `.redirect(Policy::none())` for SSRF reasons
   (`auth/providers/oauth2.rs:32-35`); the AI-providers client does
   not.

**Impact**

* Cloud-metadata IAM exfiltration → full AWS account takeover where
  the role is broadly scoped.
* Internal service enumeration → lateral movement.
* Per-tenant data exfiltration if any tenant's IAM session has
  access to S3/GCS buckets that hold raw provider files.

**Recommendation**

1. Restrict scheme to `https` (and `http` only when the URL targets
   a configured allow-listed dev/test host).
2. Resolve the hostname at *validation time* (and again at request
   time with a custom DNS resolver) and reject if any resolved IP
   is in:
   * `127.0.0.0/8`, `::1/128` (loopback)
   * `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` (RFC1918)
   * `169.254.0.0/16`, `fe80::/10` (link-local)
   * `100.64.0.0/10` (CGNAT — Tailscale)
   * `fc00::/7` (IPv6 ULA)
   * `0.0.0.0/8`, `::/128` (unspecified)
3. Bind reqwest with
   `.redirect(reqwest::redirect::Policy::limited(3))` *plus* a
   custom `redirect_callback` that re-runs the IP check on each
   hop. Or `Policy::none()` and require the AI provider to encode a
   single canonical URL.
4. For "custom" providers (the only ones where this matters; the
   built-in ones have hard-coded URLs), require an admin permission
   ladder: `llm_providers::create_custom` separate from
   `llm_providers::create` so the SSRF surface is gated.
5. Reuse the SSRF helper from the MCP module (the audit
   `05-mcp-module-audit.md` mentions one is being built); do not
   re-implement.

---

### F-04 — Cross-tenant `llm_provider_files` mapping leakage (HIGH, OPEN)

* **Severity:** High
* **ASVS:** V4.1.1 (per-user authorization on every record),
  V4.2.2 (least privilege on shared resources)
* **CWE:** CWE-639 (Authorization Bypass Through User-Controlled Key),
  CWE-284 (Improper Access Control)

**Location**

* `src/modules/llm_provider_files/repository.rs:11-33`
  (`get_provider_file_mapping`).
* `src/modules/llm_provider_files/service.rs:34-130`
  (`get_or_upload_provider_file`).
* `migrations/00000000000015_create_llm_provider_files_table.sql:3-48`.

**Vulnerable code (`repository.rs:11-33`)**

```rust
pub async fn get_provider_file_mapping(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
) -> Result<Option<LlmProviderFile>, sqlx::Error> {
    sqlx::query_as!(
        LlmProviderFile,
        r#"
        SELECT id, file_id, provider_id, provider_file_id, ...
        FROM llm_provider_files
        WHERE file_id = $1 AND provider_id = $2     -- <-- no user_id check
        "#,
        file_id,
        provider_id
    )
    .fetch_optional(pool)
    .await
}
```

The mapping table has no `user_id` column (see the schema:
`migrations/…000015…`). The service layer (`service.rs:34-130`) is
called from chat/streaming code that already has a `user_id` in
context, but **never passes it to the repository check**. The
implicit assumption is "`file_id` is owned by the user, so the
mapping is too" — but:

* `file_id` is a primary-key UUID from the `files` table. Whoever
  knows the UUID can attempt a lookup. If user B knows (or guesses,
  or harvests via a different module) user A's `file_id`, calling
  the chat endpoint with that file attached will:
  1. Trigger `get_provider_file_mapping(file_id, provider_id)`.
  2. Return user A's `provider_file_id` (which is the *external*
     Anthropic / Gemini file URI).
  3. Send a chat request to the provider that *re-uses* user A's
     uploaded file content (e.g., a private PDF), thereby exfiltrating
     it into user B's chat history.

Even if the file-loading path in `service.rs:73-77` checks
ownership via `file_repo.get_by_id(file_id)`, the early-return on
line 60-66 *does not*. If a `Completed` mapping exists, the function
returns the provider file ID directly without re-loading the
`files` row at all:

```rust
if !is_expired && mapping.upload_status == UploadStatus::Completed {
    if let Some(provider_file_id) = mapping.provider_file_id {
        return Ok(provider_file_id);            // <-- no user check
    }
}
```

So the auth chain here is "trust the caller". The caller (chat
streaming) has user context — but does it verify file ownership *before*
calling `get_or_upload_provider_file`? Out-of-scope to verify in this
audit, but the in-module defense-in-depth is missing either way.

**Exploitation**

```text
1. User A uploads a private file. file_id = UUID-A.
2. User A sends a chat that uploads UUID-A to Anthropic via the
   provider Files API. Mapping table now has
   (file_id=UUID-A, provider_id=anthropic, provider_file_id=file_xyz).
3. User B obtains UUID-A by enumeration, log scrape, or a
   sibling-module bug (e.g., a file listing in an admin endpoint
   reachable to B).
4. User B sends a chat with `file_ids: [UUID-A]`.
5. Chat/streaming calls get_or_upload_provider_file(UUID-A,
   anthropic).
6. Service finds the existing completed mapping, returns file_xyz.
7. Anthropic chat call includes file_xyz; the response contains
   contents of user A's file in B's conversation.
```

**Impact**

* Cross-tenant document exfiltration via a chat side-channel.
* Particularly bad because the provider-side file IDs do not
  expire instantly (Anthropic: 30d, Gemini: 48h) — the window for
  abuse is wide.

**Recommendation**

1. Add `user_id UUID NOT NULL REFERENCES users(id) ON DELETE
   CASCADE` to `llm_provider_files` (new migration). Change the
   unique constraint to `UNIQUE(user_id, file_id, provider_id)`.
2. Update every query in `repository.rs` to filter on `user_id =
   $N`. Pass `user_id` through `service.rs::get_or_upload_provider_file`.
3. Add an integration test:
   `tests/llm_provider_files/cross_tenant_test.rs` that uploads as
   user A and tries to fetch as user B; assert 404.
4. Migration plan: for the existing rows, look up the file's owner
   from `files.user_id` and backfill `llm_provider_files.user_id`
   in the same transaction.

---

### F-05 — Reqwest client built without proxy/TLS hardening, despite `ProxySettings.ignore_ssl_certificates` toggle being persisted (HIGH, OPEN)

* **Severity:** High
* **ASVS:** V9.1.1 (TLS for outbound), V9.1.3 (no insecure TLS
  options), V9.2.4 (validate certificates)
* **CWE:** CWE-295 (Improper Certificate Validation), CWE-757
  (Selection of Less-Secure Algorithm During Negotiation)

**Location**

* `src/modules/llm_provider/models.rs:11-25` —
  `ProxySettings.ignore_ssl_certificates` and `ProxySettings.password`
  fields.
* `ai-providers/src/provider.rs:110-113` — client built with
  `Client::builder().timeout(120s).build()` only.
* No call site applies the `ProxySettings` to the client.

**Issue**

The `ProxySettings` struct **is persisted** through the DB schema
and **is returned in JSON responses** (it's a field of `LlmProvider`),
but it is **never read at outbound-request construction time**. The
field appears to be a half-finished feature where the UI collects
proxy and `ignore_ssl_certificates` values, but the AI-providers
crate ignores them.

This creates two distinct problems:

1. **Stored-but-unused secrets.** `proxy_settings.password` is a
   plaintext field round-tripped to every API consumer, including
   the user-facing `/user-llm-providers` endpoint, just like F-01.
   Even though it never gets used, it shows up in responses (see
   `api-client/types.ts:1221-1228` — `ProxySettings.password?: string`
   is in the public schema).
2. **Toggle-without-effect anti-pattern.** Admins may set
   `ignore_ssl_certificates: true` for a misconfigured upstream and
   *believe* the server now disables TLS verification — when in
   reality the toggle does nothing, so requests fail in a confusing
   way; the admin will then chase the wrong workaround (e.g.,
   adding system-wide CA hacks).

**Exploitation / Impact**

* Same exfiltration path as F-01 for any provider that has proxy
  credentials configured (these are often real corporate proxy
  passwords).
* Once the feature is actually wired up (which there is pressure to
  do — see the UI types being generated), `ignore_ssl_certificates:
  true` becomes a stored CWE-295 — attackers proxying outbound
  traffic for that provider get to MITM the upstream call and steal
  the api_key in transit.

**Recommendation**

1. Remove `proxy_settings.password` from API responses (use the
   same write-only DTO trick as F-01).
2. Decide: either wire the feature up *with safety rails*
   (rejection of `ignore_ssl_certificates: true` unless an explicit
   admin permission `llm_providers::allow_insecure_tls` is held;
   audit log entry on every use; warn-level startup log listing
   providers with TLS off), or remove the toggle entirely.
3. If proxy support is needed, build it on top of
   `reqwest::Proxy::http(url).basic_auth(user, pass)` and verify
   `proxy_url` against the same SSRF allow-list from F-03
   (otherwise admins can ship traffic through `http://localhost:8080`
   and weaponize it).

---

### F-06 — `eprintln!` for all error logging, missing structured tracing, potential secret leak via DB error messages (HIGH, OPEN)

* **Severity:** High
* **ASVS:** V7.1.1 (security-relevant logging),
  V7.1.4 (logging must not leak secrets),
  V7.2.1 (use a structured logger)
* **CWE:** CWE-532 (Insertion of Sensitive Information into Log File),
  CWE-778 (Insufficient Logging)

**Location**

* `src/modules/llm_provider/handlers/admin.rs` — 16 `eprintln!`
  calls (grep: lines 43, 90, 122, 160, 193, 208, 239, 268, 277, 312,
  322, 363, 394, 416, 429, 441, 451).

**Vulnerable code (admin.rs:121-124, 159-161)**

```rust
let provider = Repos.llm_provider.create(request).await.map_err(|e| {
    eprintln!("Failed to create provider: {}", e);
    AppError::internal_error("Database operation failed")
})?;
```

The Display impl of `sqlx::Error` for some failure modes
(`UniqueViolation`, `CheckViolation`, `ForeignKeyViolation`)
includes the SQL fragment and parameter values being bound. SQLx's
`Error::Database` does **not** redact bound parameters — the wrapped
`PgDatabaseError` may surface column values via the "detail"
string, depending on Postgres `log_min_messages` and
`log_error_verbosity` settings.

Concretely: if an admin attempts to create two providers with the
same name and `name` is unique-indexed (not currently, but the
audit recommends it), the Postgres error is "duplicate key value
violates unique constraint … DETAIL: Key (name)=(sk-xxxxx) already
exists." (or whatever the duplicate column was). If an admin
accidentally types the api_key into the name field by paste-error
— which happens — that ends up in the stderr stream of the server.

**Impact**

* Plaintext credentials in stderr / journald / k8s log aggregation /
  CloudWatch. These logs typically have a different access-control
  surface than the API (e.g., readable by SREs who don't have
  customer-data access).
* Loss of structured-logging features: no `trace_id`, no log levels,
  no log-format consistency.

**Recommendation**

1. Replace every `eprintln!("Failed to … {}: {}", id, e)` with
   `tracing::error!(provider_id = %id, error = %e, "operation
   failed")`. The macro will produce structured fields that can be
   redacted at the formatter layer.
2. Add a `Display` wrapper for `sqlx::Error` that strips the
   `DETAIL:` line via regex before formatting.
3. Add a `tracing-subscriber` filter rule that downgrades
   `eprintln!`-equivalent errors to `INFO` level with the secret
   columns redacted.
4. Audit the entire codebase for `format!("{:?}", provider)` — the
   derived `Debug` impl on `LlmProvider` will print the api_key
   plainly.

---

### F-07 — `default_runtime_version_id` set to `None` on create (MEDIUM, OPEN)

* **Severity:** Medium (data-integrity bug; not a confidentiality
  issue but worth a finding because the RETURNING clause silently
  drops the field)
* **ASVS:** V5.1.5 (input/output consistency)
* **CWE:** CWE-665 (Improper Initialization)

**Location**

* `src/modules/llm_provider/repositories/admin.rs:204-235`.

**Vulnerable code**

```rust
let row = sqlx::query!(
    r#"INSERT INTO llm_providers (id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
     RETURNING id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings, created_at, updated_at"#,
    // ^^ no `default_runtime_version_id` in RETURNING
    ...
)
...
Ok(LlmProvider {
    ...
    default_runtime_version_id: None,    // <-- hard-coded
    ...
})
```

This is a latent bug: the `INSERT` does not set
`default_runtime_version_id` (migration 21 added that column with a
DB-side default), but the in-memory struct is then handed to the
event bus and to other modules with the value forced to `None`. If
any downstream caller relies on the returned struct as the source of
truth for the runtime, it will see `None` and may select an
inappropriate runtime version. Re-read after insert.

**Recommendation:** add `default_runtime_version_id` to the
`RETURNING` clause and stop hard-coding `None`.

---

### F-08 — `UpdateLlmProviderRequest` does not deny unknown fields (MEDIUM, OPEN)

* **Severity:** Medium
* **ASVS:** V5.1.2 (strict deserialization), V5.1.3 (reject
  unexpected input)
* **CWE:** CWE-915 (Improperly Controlled Modification of Dynamically-Determined Object Attributes)

**Location**

* `src/modules/llm_provider/types.rs:31-43`.

**Issue**

The `Create` variant uses `#[serde(deny_unknown_fields)]`
(`types.rs:17`); the `Update` variant does not. A
mass-assignment-style attacker could submit fields
(`{"id": "…", "built_in": true, "default_runtime_version_id":
"…"}`) on an update without triggering a 400. Serde will silently
drop them, but if the struct is refactored later to add new
deserializable fields, the missing `deny_unknown_fields` becomes
an open privilege channel.

**Recommendation:** add `#[serde(deny_unknown_fields)]` to
`UpdateLlmProviderRequest`. Optionally to every other request DTO
in the module (`AssignProviderToGroupRequest`,
`UpdateGroupProvidersRequest`, `SaveUserApiKeyRequest`).

---

### F-09 — No max-length validation on `name`, `base_url`, etc. (MEDIUM, OPEN)

* **Severity:** Medium
* **ASVS:** V5.1.4 (validate input length)
* **CWE:** CWE-1284 (Improper Validation of Specified Quantity in Input)

**Location**

* `src/modules/llm_provider/utils.rs:44-91` — only checks empty /
  trim / URL-format / valid provider type. No upper bounds on any
  string.
* DB column lengths: `name VARCHAR(255)`, `base_url VARCHAR(512)`,
  `api_key TEXT` (unbounded).

**Issue**

A request with `"name": "<10MB of A>"` reaches Postgres and only
gets a constraint-violation error there. Same for `base_url`.
`api_key` has no length validation at all at the admin layer (the
*user* path does cap at 500 in `handlers/user.rs:111-113`), so an
admin could set a 100MB key, OOM the JSON deserializer, and trigger
a DoS.

The 500-byte cap on user-facing `api_key` is good but inconsistent —
real provider keys are well under 200 bytes; the cap is arbitrary
and the validation is duplicated rather than centralized.

**Recommendation**

* Add a `MAX_NAME_LEN = 255`, `MAX_URL_LEN = 512`, `MAX_API_KEY_LEN
  = 1024` and apply in `utils::validate_*` for both `Create` and
  `Update` variants.
* Add a request-body size limit middleware at the Axum layer
  (which the 07-core audit also flags in its CRIT-04).

---

### F-10 — Built-in provider deletion check race (MEDIUM, OPEN)

* **Severity:** Medium
* **ASVS:** V4.2.1 (consistent authorization across the operation)
* **CWE:** CWE-367 (TOCTOU Race Condition)

**Location**

* `src/modules/llm_provider/repositories/admin.rs:311-336`
  (`delete_llm_provider`).

**Issue**

```rust
let built_in_result = sqlx::query_scalar!(
    "SELECT built_in FROM llm_providers WHERE id = $1",
    provider_id
).fetch_optional(pool).await?;
// race window
sqlx::query!("DELETE FROM llm_providers WHERE id = $1", provider_id)
    .execute(pool).await?;
```

Two queries in two separate transactions; an attacker can race a
`UPDATE … SET built_in = false WHERE id = ?` to slip the delete.
This requires an attacker who already has `llm_providers::edit`, so
the practical impact is low — but it violates the documented intent
("built-in providers cannot be deleted").

**Recommendation**

```sql
DELETE FROM llm_providers WHERE id = $1 AND built_in = false RETURNING built_in;
```

— or wrap in a single transaction with `SELECT … FOR UPDATE`.

---

### F-11 — `created_at` / `updated_at` use lossy `from_timestamp(_, 0)` (MEDIUM, OPEN)

* **Severity:** Medium (data integrity, low-impact security)
* **ASVS:** V8.3.1 (consistent timestamps for audit trails)
* **CWE:** CWE-697 (Incorrect Comparison)

**Location**

* `src/modules/llm_provider/repositories/admin.rs:127-128`,
  `:157-158`, `:189-190`, `:233-234`, `:455-456`, `:498-499`.

**Issue**

```rust
created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
```

This truncates sub-second precision *and* `unwrap()`s a fallible
conversion. Audit-trail timestamps lose millisecond precision —
important when correlating with PostgreSQL `log_*` logs for
incident-response. The `.unwrap()` is theoretically panic-able if
the timestamp is outside the i64 range (year ~9999) — not exploitable
but a code-smell.

**Recommendation:** keep the `time::OffsetDateTime` from sqlx and
convert at the API boundary, or use `chrono::Utc.timestamp_nanos_opt`.

---

### F-12 — Validation order: `validate_provider_type` after `validate_name` not robust to empty input (MEDIUM, OPEN)

* **Severity:** Medium
* **ASVS:** V5.1.1 (input validation completeness)
* **CWE:** CWE-20

**Location**

* `src/modules/llm_provider/utils.rs:44-74`.

**Issue**

The validation chain rejects empty `name` and invalid `provider_type`,
but: it does not reject `\n`, `\r\0`, `\x00` in `name` (a name like
`"My Provider\0evil"` will be accepted and stored — the PG TEXT type
will reject `\0` but `\n` survives and can confuse log parsers,
embed in CSV export, etc.). Same for `description` (n/a here; only
`name`).

**Recommendation:** add `is_control_char` rejection on `name`,
analogous to the check on `api_key` at `handlers/user.rs:114-118`:
```rust
if name.bytes().any(|b| b < 0x20 && b != b'\t') { ... }
```

---

### F-13 — No rate limiting on any provider endpoint (MEDIUM, OPEN)

* **Severity:** Medium
* **ASVS:** V11.1.1 (rate-limit security-relevant endpoints)
* **CWE:** CWE-307 (Improper Restriction of Excessive Authentication Attempts — by extension)

**Issue**

No rate-limiter middleware exists in the whole server
(`grep -rn "tower_governor\|RateLimit"` returned nothing). The
provider create/update endpoints are an excellent place for an
attacker who has stolen an admin token to brute-force test
`api_key` candidates against an outbound provider, observing
success/failure timings via the response.

**Recommendation:**

* Add `tower-governor` or a custom middleware. Suggested limits:
  - `POST /llm-providers`: 10/min/admin
  - `POST /llm-providers/{id}`: 30/min/admin
  - `POST /user-llm-providers/api-keys`: 20/hour/user
* The 07-core audit also flags this globally.

---

### F-14 — Masked-key prefix exposes 4 bytes of the key (LOW, OPEN)

* **Severity:** Low
* **ASVS:** V8.2.2 (sensitive data minimization)
* **CWE:** CWE-200

**Location**

* `src/modules/llm_provider/repositories/user.rs:97-108`
  (`list_for_user`).

**Issue**

```rust
let masked_key = if r.api_key.len() > 4 {
    format!("{}***", &r.api_key[..4])
} else {
    "***".to_string()
};
```

Most provider keys are prefixed (OpenAI `sk-`, Anthropic `sk-ant-`,
Google `AIza`). Showing 4 characters reveals the provider type
(already known via `provider_id`) but reveals zero entropy for
OpenAI (always `sk-x`, where `x` ∈ {`p`, `o`, ...}) and approximately
one character of entropy for Anthropic. The user UX benefit is
marginal (the user knows which key is theirs). Also: indexing by
character `[..4]` is byte-indexing — if a key has multibyte
characters (it shouldn't, but isn't validated), this panics.

**Recommendation:** show the last 4 characters instead of the first
4 (industry standard), or both with a `…` separator. Use
`r.api_key.chars().rev().take(4)` to be UTF-8-safe. Also explicitly
reject non-ASCII in the key on save (currently the only check is
the `0x20` filter).

---

### F-15 — `built_in` provider can have its `api_key` overwritten by anyone with `llm_providers::edit` (LOW, OPEN)

* **Severity:** Low (admin-only privilege escalation — defense-in-depth)
* **ASVS:** V4.1.3 (least privilege on built-in resources)
* **CWE:** CWE-269 (Improper Privilege Management)

**Location**

* `src/modules/llm_provider/repositories/admin.rs:238-309`
  (`update_llm_provider`) — no `built_in` check.

**Issue**

There is no separation between "edit a custom provider" and "edit a
built-in (system) provider's key." Anyone with `llm_providers::edit`
can:
* Disable the built-in OpenAI provider.
* Overwrite the system api_key with their own (sneaking their personal
  key into all users' chats, which they then bill).
* Toggle `enabled: false` on a critical provider to deny service.

**Recommendation:** add a separate permission
`llm_providers::edit_builtin`, or refuse PUT on built-in records
except when the request only updates the api_key field.

---

### F-16 — Provider type allowlist hard-coded in 2 places (LOW, OPEN)

* **Severity:** Low (consistency / maintainability)
* **ASVS:** V14.2.1 (single source of truth for configuration)
* **CWE:** CWE-1188 (Initialization of a Resource with an Insecure Default)

**Location**

* `utils.rs:9-19` lists 9 valid provider types.
* `migrations/00000000000003_create_llm_providers_table.sql:5-7`
  has the same `CHECK` constraint.
* `chat/core/ai_provider/mod.rs:70-74` has its own type switch
  (`anthropic | gemini → typed; else → openai-compatible`).

**Issue:** triple-source-of-truth invariant. Easy for one to drift.

**Recommendation:** define `pub enum ProviderType` with sqlx Type
derive, use it everywhere; remove the string allowlist.

---

### F-17 — `delete_user_api_key` does not verify the provider exists (LOW, OPEN)

* **Severity:** Low (info disclosure of provider existence)
* **ASVS:** V7.4.1 (uniform error responses to avoid enumeration)
* **CWE:** CWE-204 (Observable Response Discrepancy)

**Location**

* `src/modules/llm_provider/handlers/user.rs:139-150`.

**Issue:** `DELETE /user-llm-providers/api-keys/{provider_id}` returns
204 No Content regardless of whether the provider exists, whether
the user had a key for it, or whether the user has access to that
provider. This is timing-stable but allows the attacker to
preserve cookies / not pollute audit log unconditionally.

Not strictly a bug (idempotent delete is the REST norm) but worth
noting that no permission check on `provider_id` is performed: a
user can submit any UUID and the delete is silently a no-op. Audit
logs will fill with delete-attempts for non-existent providers.

**Recommendation:** add a `where_exists` check and log enumeration
attempts at debug-level.

---

### F-18 — `provider_metadata` JSONB accepts arbitrary structure (LOW, OPEN)

* **Severity:** Low (defense-in-depth)
* **ASVS:** V5.5.1 (deserialization restrictions)
* **CWE:** CWE-502 (Deserialization of Untrusted Data)

**Location**

* `src/modules/llm_provider_files/service.rs:139-156`
  (`save_upload_response`).
* `src/modules/llm_provider_files/models.rs:13-22`.

**Issue**

The provider's `FileUploadResponse.metadata` is a free-form
`serde_json::Value` written into the JSONB column with two extra
fields injected (`uploaded_at`, `filename`, `expires_at`). If a
provider (or an attacker who controls a custom provider's HTTP
response) returns arbitrary JSON, that JSON is stored verbatim. It
will then be GIN-indexed and queried, but never validated. An
unbounded blob (e.g., 10MB of `{"junk":"x"}`) can be saved per
file-provider pair.

**Recommendation:** size-limit `provider_metadata` to 16KB
(`if metadata.to_string().len() > 16384 { strip ;}` or reject).

---

### F-19 — Hardcoded fallback base URL `http://localhost:8000/v1` for "unknown" provider types in upload service (INFO, OPEN)

* **Severity:** Info
* **ASVS:** V14.1.1 (no hardcoded development URLs in production paths)
* **CWE:** CWE-547 (Use of Hard-coded, Security-relevant Constants)

**Location**

* `src/modules/llm_provider_files/service.rs:102-110`.

```rust
let base_url = provider.base_url.as_deref().unwrap_or_else(|| {
    match provider.provider_type.as_str() {
        "anthropic" => "https://api.anthropic.com/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta",
        "openai" => "https://api.openai.com/v1",
        _ => "http://localhost:8000/v1",   // <-- HTTP, localhost
    }
});
```

**Issue:** if an admin somehow creates a "groq"/"deepseek"/etc.
provider without a `base_url`, file uploads default to a plaintext
localhost URL. Not exploitable today (the API contract requires
base_url) but a footgun.

**Recommendation:** return an error instead of falling back to
localhost (this matches `chat/core/ai_provider/mod.rs:86-90` which
correctly errors).

---

### F-20 — `unwrap_or_default` on `proxy_settings` deserialization swallows JSON errors (INFO, OPEN)

* **Severity:** Info
* **Location:** `repositories/admin.rs:123-126`, `:153-156`, etc.

**Issue:** If `proxy_settings` JSON is corrupted in the DB, the code
silently returns `Default::default()` rather than logging or
erroring. Hides DB corruption.

**Recommendation:** `tracing::warn!` when the deserialize fails.

---

### F-21 — `Repos.llm_provider.create` doesn't normalize URL trailing slash, causing duplicate-but-equivalent providers (INFO, OPEN)

* **Severity:** Info
* **Location:** `utils.rs:31-41`, `repositories/admin.rs:196-236`.

**Issue:** No URL normalization. `https://api.openai.com/v1` vs
`https://api.openai.com/v1/` create two provider rows. Doesn't
affect security directly but increases the attack surface from F-01
(more keys to leak).

**Recommendation:** trim trailing slashes; lowercase scheme + host.

---

### F-22 — Empty `Group_Provider_Files` cleanup on provider delete drops only the mapping rows, not the *remote* provider files (INFO, OPEN)

* **Severity:** Info
* **Location:** `migrations/00000000000015_create_llm_provider_files_table.sql:8` (`ON DELETE CASCADE`).

**Issue:** When a provider row is deleted, the local mapping rows
cascade-delete, but the *remote* Anthropic/Gemini file IDs are left
dangling on the provider side, still billed to the admin's
account, still containing user content. There is no background
job that issues `DELETE /v1/files/{id}` to the provider before the
local mapping is dropped.

**Recommendation:** add a "graceful provider deletion" path that
iterates the mapping table, calls `ai_provider.delete_file(&id)`
on each row, then deletes the provider. Or: schedule a background
job to drain orphaned remote files based on a tombstone table.

---

## ASVS Coverage Matrix

| Chapter | Control | Status | Finding |
|---|---|---|---|
| V4.1.1 | Per-record authorization | FAIL | F-04 |
| V4.1.3 | Built-in resource least-privilege | FAIL | F-15 |
| V4.2.1 | Access control consistency | FAIL | F-01, F-10 |
| V4.2.2 | Least privilege on shared resources | FAIL | F-04 |
| V5.1.1 | Input validation completeness | PARTIAL | F-12 |
| V5.1.2 | Strict deserialization | PARTIAL | F-08 (Create OK, Update fails) |
| V5.1.3 | Reject unexpected input | PARTIAL | F-08 |
| V5.1.4 | Validate input length | FAIL | F-09 |
| V5.1.5 | Input/output consistency | FAIL | F-07 |
| V5.5.1 | Deserialization restrictions | PARTIAL | F-18 |
| V6.2.1 | Crypto protection of secrets at rest | FAIL | F-02 |
| V6.2.5 | Encryption key management | FAIL | F-02 |
| V6.4.2 | No plaintext storage of secrets | FAIL | F-02 |
| V7.1.1 | Security-relevant logging | PARTIAL | F-06 |
| V7.1.4 | Logging must not leak secrets | FAIL | F-06 |
| V7.2.1 | Structured logger | FAIL | F-06 |
| V7.4.1 | Uniform error responses | PARTIAL | F-17 |
| V8.2.2 | Sensitive data minimization in responses | FAIL | F-01, F-14 |
| V8.3.1 | Consistent audit timestamps | PARTIAL | F-11 |
| V9.1.1 | TLS for outbound | PASS (reqwest default) | — |
| V9.1.3 | No insecure TLS options | FAIL | F-05 |
| V9.2.1 | Outbound calls controlled | FAIL | F-03 |
| V9.2.4 | Certificate validation | PASS (current), AT-RISK | F-05 |
| V11.1.1 | Rate-limit security-relevant endpoints | FAIL | F-13 |
| V12.6.1 | URL allowlist | FAIL | F-03 |
| V13.1.1 | API security baseline | PARTIAL | composite |
| V14.1.1 | No hardcoded dev URLs | PARTIAL | F-19 |
| V14.2.1 | Single source of truth | PARTIAL | F-16 |

**ASVS Level 2 verdict:** FAIL. The module cannot meet Level 2
until F-01, F-02, F-03 are resolved.

---

## Positive Findings

1. **All SQL queries use parameterized `sqlx::query!` macros.** No
   string concatenation found in the audited paths.
2. **Per-user API key feature exists.** The `user_llm_provider_api_keys`
   table + `UserKeyRepository` (added migration 28) is a sound
   design: each user maintains their own key, the system key is a
   fallback, and the resolution priority is correctly documented at
   `chat/core/ai_provider/mod.rs:17`. This *can* eventually become
   the closing fix for F-01 if the system key is removed entirely.
3. **Permission system is granular and well-named.** Five distinct
   permissions (`read`, `create`, `edit`, `delete`, `assign_groups`)
   on `llm_providers` plus a separate user-facing
   `user_llm_providers::read`. Defines a clean RBAC surface
   (compromised by F-01's response shape, not by missing permissions).
4. **Built-in provider deletion is refused.** `delete_llm_provider`
   correctly rejects (`Err("Cannot delete built-in provider")`)
   when `built_in = true`, modulo the F-10 race.
5. **Foreign-key cascade on user delete.** `user_llm_provider_api_keys.user_id
   ON DELETE CASCADE` correctly cleans up user keys when the user
   is purged. (Provider deletion cascades the mapping but not the
   remote files — see F-22.)
6. **Per-user API key write path validates input.** `handlers/user.rs:106-118`
   trims, rejects empty, caps at 500 bytes, rejects control
   characters. Good defensive coding. (Why this same care isn't
   applied to admin paths — see F-09 — is unclear.)
7. **API keys flushed from update path when empty.** `repositories/admin.rs:274-283`
   correctly converts a blank string update to `NULL` rather than
   storing an empty key. Prevents a "I'm configured but with empty
   key" silent-failure footgun.
8. **`#[serde(deny_unknown_fields)]` on the create-DTO.** Good
   hygiene; needs the same on the update-DTO (see F-08).
9. **Event emission is consistent.** Every mutation
   (`create`/`update`/`delete`/`assign_to_group`) emits an
   `LlmProviderEvent` to the event bus, enabling downstream cache
   invalidation. The event bodies do NOT include `api_key` — the
   `LlmProviderEvent::Created { provider: LlmProvider }` does
   serialize it via `serde::Serialize`, but the events are internal
   pub/sub (in-process) and not exposed externally.
   **Note:** if the event bus is ever extended to cross-process
   (Redis, NATS), the api_key will become a wire-leak — flag for
   future review.
10. **Repository struct wrapping.** Repositories take `&self`
    (`LlmProviderRepository::get_by_id(&self, ...)`) and hold a
    cloned `PgPool`. Idiomatic Rust, no global mutable state.

---

## Out of Scope / Deferred

The following were in this module's directory but explicitly
deferred to other audits per the scope statement at the top of this
document:

* **`llm_model/`** — model file management and the download pipeline
  (audit `04` covered SSRF in repository URLs, HIGH-2).
* **`llm_repository/`** — Hugging Face / Git repository auth (also
  audit `04`, HIGH-1).
* **`llm_local_runtime/`** — local model server binary management.
* **Auth itself** — `01-auth-user-permissions-audit.md` covers the
  `RequirePermissions` extractor and the JWT chain. We verified
  only that the permissions defined in this module's
  `permissions.rs` are wired up and used in handler signatures; we
  did not re-verify the extractor's correctness.
* **`chat/core/ai_provider/mod.rs`** — the API-key resolution helper
  was read for context (key priority chain looks correct) but is
  part of the chat module and audited there.
* **`ai-providers/` crate** — the reqwest client construction
  (`provider.rs:110-113`) was reviewed only for SSRF /
  TLS-validation defaults. The full surface of the crate (request
  signing, stream parsing, error handling) is out of scope.
* **OAuth-style providers (Bedrock / Vertex AI assume-role).** Not
  implemented in the current code (the provider_type allowlist is
  `local|openai|anthropic|groq|gemini|mistral|deepseek|huggingface|custom`,
  none of which involve STS / service-account JSON). If/when added,
  re-audit.

### Items revisited from 2025-11 audit (04-llm-modules-audit.md)

* **CRIT-1** (API keys in responses): **STILL OPEN.** See F-01.
* **HIGH-1** (Repository credentials in responses): out of scope
  (llm_repository module).
* **HIGH-2** (SSRF in repository downloads): out of scope but the
  same root cause (no URL allow-list) recurs here as F-03.
* **HIGH-3 / HIGH-4** (per the 2025-11 numbering, weren't present in
  llm_provider): see the original audit.

---

## Suggested remediation order

1. **F-01 (Critical, 1 day):** Stop serializing `api_key`. Bug-for-bug
   patch of 4 handlers + 2 DTOs + 6 tests.
2. **F-03 (High, 2-3 days):** SSRF allow-list + redirect-policy
   tightening + IP-literal check.
3. **F-04 (High, 1 day + migration):** Add `user_id` to
   `llm_provider_files`; backfill from `files.user_id`; add
   per-user filter in repository.
4. **F-02 (Critical-but-bigger, 1-2 weeks):** at-rest encryption.
   Start with `pgcrypto` for v1, then plan KMS for v2.
5. **F-05, F-06 (High, 2 days):** strip `proxy_settings.password`
   from responses + replace `eprintln!` with `tracing::error!`.
6. The remaining Mediums/Lows/Infos in any order.

---

## Reference files (absolute paths)

* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/mod.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/models.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/types.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/routes.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/permissions.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/utils.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/events.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/handlers/admin.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/handlers/user.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/repositories/admin.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider/repositories/user.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider_files/mod.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider_files/models.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider_files/repository.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/llm_provider_files/service.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/chat/core/handlers/providers.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/chat/core/types/providers.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/src/modules/chat/core/ai_provider/mod.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/ai-providers/src/provider.rs`
* `/home/pbya/projects/ziee-chat/src-app/server/migrations/00000000000003_create_llm_providers_table.sql`
* `/home/pbya/projects/ziee-chat/src-app/server/migrations/00000000000015_create_llm_provider_files_table.sql`
* `/home/pbya/projects/ziee-chat/src-app/server/migrations/00000000000027_fix_default_user_permissions.sql`
* `/home/pbya/projects/ziee-chat/src-app/server/migrations/00000000000028_create_user_provider_api_keys.sql`
* `/home/pbya/projects/ziee-chat/src-app/ui/src/api-client/types.ts` (generated)

— *End of audit.*
