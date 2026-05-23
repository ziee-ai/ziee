# Security Audit — Hub Module
**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/hub/` (~2,116 LOC) — curated marketplace/catalog for models, assistants, MCP servers; embedded JSON + GitHub-refreshable; "create from hub" flows that materialize catalog entries into per-user / system entities
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target

---

## Executive Summary

The "hub" module is a **content marketplace** (not an event hub / subscription bus). It serves three curated catalogs — LLM models, assistants, MCP servers — that ship embedded in the binary, can be refreshed from a public GitHub repo, and can be "instantiated" into per-user entities (assistant, user MCP server) or system entities (LLM model download) via three POST endpoints. The terms "subscribe / replay / cross-user-event-leak" from the audit brief do not apply — there is no SSE/WebSocket surface and the only events emitted are server-internal `AppEvent::Hub(…)` variants consumed by a cleanup handler.

The security model is reasonable in shape:

- Every route is guarded by `RequirePermissions<…>`.
- Catalog data is read-only at runtime — users cannot inject `command`/`args`/`env`/`url` for MCP servers from the hub interface; everything executable comes from the curated catalog on disk.
- `hub::models::*` permissions were correctly stripped from the default Users group in migration 37, so unprivileged users cannot trigger multi-GB downloads onto shared local providers.
- Stdio MCP server creation downstream is constrained by a command allowlist (`npx, uvx, python, python3, node, deno`) and a blocked-env-var list, so even if a hub-supplied stdio server config were attacker-controlled, command execution is bounded.
- The hub never returns another user's data: the three GET endpoints either expose system-wide catalog data (models) or filter `created_ids` by `auth.user.id` (assistants, MCP servers).

Three real risks dominate:

1. **Catalog supply-chain — placeholder GitHub URL.** `GITHUB_HUB_REPO` is `https://raw.githubusercontent.com/YOUR_ORG/ziee-hub/main`, a placeholder. Any party that creates a `YOUR_ORG/ziee-hub` GitHub repo controls the refresh endpoint's response. Refresh is admin-only (no default-group grant), but the consequences of a refresh ranging from "user-visible misinformation in the catalog UI" to "next user who creates a Hub MCP server gets the attacker's stdio config (constrained by command allowlist) and the attacker's environment_variables" are non-trivial. There is no signature verification, no checksum, no fingerprint pin, no response-size cap, and the `version` string from the attacker JSON is then used as a filesystem path component. (Prior audit 06 flagged the hardcoded URL as LOW; given the downstream blast radius, this audit re-rates it MEDIUM.)
2. **Path traversal via the `lang` query parameter** in `/hub/{models,assistants,mcp-servers}` — `HubQuery.lang` is interpolated into a filename without any character/length validation and the resulting path is read via `async_fs::read_to_string`. Exploitation is constrained (must hit a `.json` file that parses as a JSON array) but the read primitive plus the resource-exhaustion variant (`lang=../../../../../dev/zero`) are real. This finding **carries over unfixed** from prior audit 06-§1 — re-confirmed open at audit time.
3. **`provider_id` not constrained to local providers** in `/hub/models/download`. The hub exposes a sibling endpoint `/hub/models/local-providers` listing only local providers, but the download endpoint accepts any `provider_id` and forwards to `initiate_repository_download_internal` (which has no provider-type check). A user with `hub::models::download` can target any provider UUID; the download will most likely fail in the cloud-provider branch, but the path data goes onto disk before failure.

### Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High     | 0 |
| Medium   | 4 |
| Low      | 6 |
| Info     | 5 |

### Top-3 risks

1. **M-1** Placeholder GitHub URL (`YOUR_ORG/ziee-hub`) + no integrity verification on refresh: attacker-squatted repo could push tampered catalog data, an attacker-controlled `version` string is reused as a filesystem path component, and a multi-GB response is buffered fully into memory by `.bytes().await`.
2. **M-2** Path traversal via `?lang=` query parameter — unfixed since prior audit (06-§1), enables arbitrary-`.json`-file disclosure and a `/dev/zero`-style memory exhaustion vector.
3. **M-3** `create_model_from_hub` accepts any `provider_id` and bypasses the public `LlmModelsCreate` permission through `initiate_repository_download_internal`, the explicit "no auth check" internal helper.

---

## Findings

### F-01 — Catalog supply-chain: placeholder GitHub URL + no integrity verification on refresh

**Severity:** **Medium**
**ASVS:** V10.3.2 (verified third-party assets), V12.4.1 (untrusted upload validation), V13.4.1 (GraphQL/REST input validation — applied to fetched content), V14.2.1 (verify components), V10.3.1 (signed update channel)
**Files:** `hub_manager.rs:10, 277-336`, `handlers.rs:152-235`

**Description.**
`GITHUB_HUB_REPO` is hardcoded to a placeholder org:

```rust
const GITHUB_HUB_REPO: &str = "https://raw.githubusercontent.com/YOUR_ORG/ziee-hub/main";
```

The three refresh handlers (`refresh_hub_models`, `refresh_hub_assistants`, `refresh_hub_mcp_servers`) call `hub_manager.refresh_hub_category(category)`, which fetches `{GITHUB_HUB_REPO}/{category}/version.json`, then `{GITHUB_HUB_REPO}/{category}/{version}/base.json`, and writes both into `<app_data>/hub/{category}/{version}/`. There is:

1. No signature verification (no minisign, no cosign, no GPG, no in-binary public key).
2. No SHA-256 / content-hash pin (compare with the sandbox-rootfs flow in `code_sandbox`, which DOES have keyless cosign).
3. No `Content-Length` cap before `.bytes().await` materialises the full response.
4. No timeout on the HTTP fetch.
5. No fingerprint / TLS pin to GitHub.
6. No validation of the `version` field in `version.json` — see F-04 for the path-traversal consequence.

**Why "Medium" and not "Low":**
- The placeholder org **can be registered today** by anyone with a GitHub account. Once registered, refresh fetches their content for every operator who deploys with the source unchanged.
- Refresh is admin-only (no default-group grant), but the *consequences propagate*: tampered `base.json` for `mcp-servers` becomes the `command`/`args`/`env` of every subsequent `create_mcp_server_from_hub` call. Even though stdio command execution is allowlisted at `mcp/client/stdio.rs`, the attacker can:
  - swap legitimate `npx -y @modelcontextprotocol/server-github` args for an attacker-controlled `@org/malicious-package` (npm registry / typosquat),
  - inject `environment_variables` not in `BLOCKED_ENV_VARS` (e.g., `HTTP_PROXY=attacker.com:8080`, `NODE_OPTIONS=--require ./attacker.js`) — note that `NODE_OPTIONS` is NOT in the blocklist and *is* effective against the `node` allowlist entry,
  - replace `url`/`headers` for HTTP-transport MCP servers (effectively a free SSRF vector under the user's identity).
- The "user-visible" attack — swapping `display_name`/`description` of a popular Hub item to phish for credentials — is trivial.

**Memory-exhaustion variant.** `download_hub_file` calls `response.bytes().await` with no size cap. Squatted-repo response of 4 GB → 4 GB allocation in server memory.

**Attack scenario.**
1. Attacker registers `github.com/YOUR_ORG/ziee-hub` (free, takes 2 minutes).
2. Operator deploys a release with the source unchanged.
3. An admin clicks "Refresh hub" in the UI (admins routinely do this when curious about new hub content).
4. The server fetches attacker JSON, writes it to `<app_data>/hub/mcp-servers/{attacker-version}/base.json`.
5. Next user creates an MCP server from hub → attacker's command/args/env land in the user's MCP server row → on first `tools/list`, stdio transport spawns it (with allowlist constraints) or HTTP transport calls the attacker URL (no constraint at all on the URL).

**Recommendation (in order of effort).**
1. **Immediate:** change `GITHUB_HUB_REPO` to a real, owned URL (e.g., the active org). Until that happens, refresh endpoints should be disabled at the route level (or return 503 with a structured error).
2. **Short-term:** add a `reqwest::Client::builder().timeout(30s).redirect(Policy::limited(2))` configured client, an `accept_invalid_certs(false)` insurance, and an explicit content-length cap of e.g. 16 MiB (cur. catalogs are ~50 KiB).
3. **Medium-term:** ship a Sigstore (cosign-keyless or rekor) signature alongside each `base.json` — identical to the working pattern in `code_sandbox` for rootfs releases — and verify in-process via the `sigstore` crate.
4. **Defense in depth:** validate JSON shape with `serde_json::from_str::<Vec<HubModel>>(&content)` *before* writing to disk (cur. order is write-then-load on next request, so a corrupted refresh leaves the hub in a broken state until the operator clears `<app_data>/hub/`).

---

### F-02 — Path traversal via `?lang=` query parameter (carry-over from audit 06-§1, **unfixed**)

**Severity:** **Medium**
**ASVS:** V5.1.5 (input validation: bounded charset for path-significant inputs), V12.3.1 (path traversal — filesystem access via user input), V13.2.1 (RESTful: validate parameters)
**Files:** `types.rs:11-20` (`HubQuery`), `hub_manager.rs:108-157` (`load_hub_data_with_locale`)

**Description.**

```rust
// types.rs
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HubQuery {
    #[serde(default = "default_locale")]
    pub lang: String,  // ← unvalidated, unbounded
}

// hub_manager.rs
let models_override: Option<Vec<serde_json::Value>> = self
    .load_json_file_optional(
        hub_dir
            .join("llm-models")
            .join(&version)
            .join(format!("{}.json", locale)),  // ← user-controlled path component
    )
    .await?;
```

**Constraints on the read primitive:**
- Resolved path must exist.
- The `.json` suffix is appended after the user input, so the user can only read files whose actual path ends in their input + `.json` (i.e., `.json` is a hard constraint).
- The file must parse as `Vec<serde_json::Value>` (JSON array of any objects) for the override-merge codepath to fire; otherwise the call returns an error to the user. The error message includes the parse failure ("Failed to parse JSON from {PathBuf:?}: {…}") and **the resolved PathBuf** — see F-03 for the disclosure consequence.

**Exploitation paths.**

1. **Existence oracle / path disclosure.** `GET /hub/models?lang=../../../../etc/passwd` resolves to `<app_data>/hub/llm-models/{version}/../../../../etc/passwd.json`. The file doesn't end in `.json`, so the optional-load returns `None` and the request succeeds with empty overrides — *but* the request did `stat()` the constructed path, and timing or operator-log presence may distinguish "file exists" from "doesn't". More directly, picking targets that *do* end in `.json`:
   - `lang=../../../config/config` → reads `<app_data>/hub/llm-models/{version}/../../../config/config.json` (could pick up server config files if relative location matches),
   - `lang=../../../../../tmp/attacker_seed` after the attacker first writes a JSON file there (if they can leverage another upload primitive elsewhere).
2. **Resource exhaustion.** `lang=../../../../../dev/zero` (Linux) or `lang=../../../../../dev/urandom` — `async_fs::read_to_string` will read until EOF or OOM. `read_to_string` on `/dev/zero` will allocate until exhaustion.
3. **Information disclosure via error path.** The `Failed to parse JSON from {:?}` error includes the resolved `PathBuf`, leaking app-data-dir layout and confirming `<app_data>` location to unauthenticated… wait, `HubModelsRead` is required, so authenticated users with `hub::models::read` permission. Still useful for a low-privilege user surveying server filesystem.

**Why this is still Medium and not High:** the read is constrained to `.json`-suffix files, the parse must succeed as a `Vec`, and the consumer of the override only reads `id`, `display_name`, `description`, `instructions`, `use_cases`, `example_prompts`. So directly bleeding sensitive secrets requires those secrets to live in a `.json` file in a `Vec<{id: …}>` shape. The DoS variant via `/dev/zero` is more reliable.

**Recommendation.**

```rust
// types.rs
fn validate_locale(locale: &str) -> Result<(), AppError> {
    if locale.is_empty() || locale.len() > 10 {
        return Err(AppError::bad_request("INVALID_LOCALE", "locale length out of range"));
    }
    if !locale.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::bad_request("INVALID_LOCALE", "locale must be [A-Za-z0-9-]"));
    }
    Ok(())
}

// applied in each handler before the HubManager call
validate_locale(&query.lang)?;
```

Or — better — replace the freeform `String` with an enum / `match` of supported locales (`en`, `es`, `fr`, `de`, `zh-CN`, etc.), and 400 on anything else. This is the recommendation already documented in audit 06; it has **not** been applied and the source still matches the prior audit's quoted snippet.

---

### F-03 — Internal filesystem paths leaked in `AppError` messages

**Severity:** **Low**
**ASVS:** V7.4.1 (errors must not reveal sensitive system info), V8.3.4 (sensitive data classification)
**Files:** `hub_manager.rs:31-42, 80-82, 99-101, 327-336, 356-363`

**Description.**
Most internal-error messages embed full `PathBuf` debug formatting:

```rust
AppError::internal_error(format!("Failed to write file {:?}: {}", file_path, e));
AppError::internal_error(format!("Failed to read file {:?}: {}", path, e));
AppError::internal_error(format!("Failed to parse JSON from {:?}: {}", path, e));
```

These can include `<app_data>` location, OS user home directory, version directories, and (via F-02) attacker-supplied path fragments echoed back. Combined with F-02, this turns the path-traversal primitive into a *reliable* file-existence oracle: a non-existent path returns the standard 5xx with the `PathBuf`, confirming the working directory and the user's lang component were processed.

**Impact.**
- Server filesystem topology disclosure.
- Confirmation of `<app_data>` location → useful for chaining with a later write primitive.

**Recommendation.**
Strip paths from outbound error messages. Log the path server-side via `tracing::error!`, return a sanitized error to the client.

```rust
.map_err(|e| {
    tracing::error!(path = %path.display(), error = %e, "failed to read hub file");
    AppError::internal_error("Failed to load hub data")
})?
```

---

### F-04 — Attacker-controlled `version` from refresh used as filesystem path component

**Severity:** **Medium**
**ASVS:** V5.1.5, V12.3.1, V13.4.1
**Files:** `hub_manager.rs:277-292` (`refresh_hub_category`), `:294-313` (`update_category_files_from_github`)

**Description.**

```rust
let latest_version: serde_json::Value = self.fetch_json(&version_url).await?;
let latest_version_str = latest_version["version"]
    .as_str()
    .ok_or_else(|| AppError::internal_error("Invalid version format"))?;
self.update_category_files_from_github(category, latest_version_str).await?;
// ...
hub_dir.join(category).join(version).join("base.json")  // path component is attacker-JSON-supplied
```

If the GitHub repo at `GITHUB_HUB_REPO` is compromised or — see F-01 — squatted, the attacker controls the `version` string. There is **no validation** on its charset or length before it is joined into a `PathBuf` and used both for:
- the URL fragment: `{repo}/{category}/{version}/base.json`,
- the filesystem write target: `<app_data>/hub/{category}/{version}/base.json`,
- the next `get_current_version` read (since `write_version_file` persists it).

A `version` of `..` or `../../../tmp/x` would write to attacker-chosen paths under the server's filesystem permissions. Even `version` of `<10000 chars>` is a DoS for filesystem APIs that enforce path-length limits inconsistently.

**Why Medium not High:** chained with F-01, since refresh needs admin permission AND a compromised/squatted source. But (a) admins routinely refresh; (b) F-01 demonstrates the source IS currently untrusted (placeholder URL).

**Recommendation.**
Validate `version` matches strict semver-or-similar before any further use:

```rust
fn validate_hub_version(version: &str) -> Result<(), AppError> {
    if version.len() > 32 {
        return Err(AppError::bad_request("INVALID_VERSION", "version too long"));
    }
    let ok = version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
    if !ok {
        return Err(AppError::bad_request("INVALID_VERSION", "version contains invalid chars"));
    }
    Ok(())
}
```

---

### F-05 — `provider_id` in `/hub/models/download` not validated against local-provider list

**Severity:** **Medium**
**ASVS:** V4.1.1 (consistent enforcement at every layer), V4.2.1 (object-level authorization), V11.1.2 (business-logic flow integrity)
**Files:** `handlers.rs:407-541` (`create_model_from_hub`), `routes.rs:62-65` (sibling `/hub/models/local-providers` endpoint)

**Description.**
The route surface explicitly distinguishes "list providers eligible for hub downloads" (`get_hub_local_providers`, filters `provider_type='local' AND enabled=true`) from "actually start the download" (`create_model_from_hub`). But the download handler does NOT cross-check that the supplied `provider_id` was in the filtered list:

```rust
// handlers.rs ~478
let download_request = crate::modules::llm_model::handlers::uploads::DownloadFromRepositoryRequest {
    provider_id: request.provider_id,  // ← from user input, no validation
    repository_id: repository.id,
    ...
};

let download = crate::modules::llm_model::handlers::uploads::initiate_repository_download_internal(
    download_request,
)
.await
.map_err(...)?;
```

`initiate_repository_download_internal` is, per its docstring, the *bypass* path: "Internal function to initiate repository download without auth check / Used by both the public API endpoint and the hub module". It does not constrain provider_type.

**Impact.**
- A user with only `hub::models::download` (not `llm::models::create`) can target any `provider_id`. The hub's permission is weaker than the upstream module's normal write permission.
- Side-effect: a download_instance row is created against an arbitrary (possibly cloud-typed) provider before failure. This is a small DoS / database-pollution vector.
- Currently `hub::models::download` is removed from the default Users group (migration 37), but any custom role granted that permission would inherit the weak path.

**Recommendation.**
Either (a) in `create_model_from_hub`, fetch the provider and assert `provider_type == "local" && enabled`, returning 400 otherwise; or (b) move the local-only check into `initiate_repository_download_internal` itself (defense in depth — any future internal caller benefits).

---

### F-06 — Refresh endpoints lack rate limiting (carry-over from audit 06-§3, **unfixed**)

**Severity:** **Low**
**ASVS:** V11.1.4 (anti-automation on costly operations), V5.5.2 (rate-limit untrusted upstream calls)
**Files:** `handlers.rs:152-235`

**Description.**
The three `POST /hub/*/refresh` endpoints each fire one or two HTTPS GETs to `GITHUB_HUB_REPO`. There is no rate-limit middleware, no per-user throttle, no per-IP cooldown. A privileged user (or compromised admin) can hammer GitHub from the server IP, risking GitHub's IP-based rate-limit blocklist for the deployment (which also affects unrelated `git clone` operations against `raw.githubusercontent.com`).

**Compounding with F-01 memory bound:** without rate limiting, a malicious admin (or one whose session is hijacked) can request many concurrent refreshes, each buffering the full HTTP response in memory.

**Recommendation.**
Apply a 1-call-per-60-seconds-per-route limit. The prior audit's snippet using `tower_governor` is a valid approach. Track in `App-Module` middleware so it composes with the existing `RequirePermissions` layer.

---

### F-07 — `download_hub_file` response not size-capped

**Severity:** **Low**
**ASVS:** V5.5.4 (parse with size limits), V11.1.4 (resource controls)
**Files:** `hub_manager.rs:316-337`

**Description.**

```rust
let response = reqwest::get(url).await.map_err(...)?;
let content = response.bytes().await.map_err(...)?;
```

`response.bytes()` reads the entire body into memory with no upper bound. Catalogs are currently ~50 KiB; an attacker (see F-01) or a misbehaving upstream could return GB-scale responses.

**Recommendation.**
Use `response.bytes_stream()` with an accumulator that bails at, e.g., 16 MiB. Or `Content-Length` pre-check after the response head is received. Same fix applies to `fetch_json` (`hub_manager.rs:340-349`).

---

### F-08 — `track_hub_entity` does not include `created_by` in uniqueness constraint

**Severity:** **Info**
**ASVS:** V11.1.2 (business-logic flow integrity)
**Files:** `repository.rs:80-95`, `migrations/00000000000008_create_hub_entities_table.sql:19`

**Description.**

```sql
CONSTRAINT unique_entity_hub_tracking UNIQUE(entity_type, entity_id)
```

```rust
INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (entity_type, entity_id)
DO UPDATE SET hub_id = EXCLUDED.hub_id, hub_category = EXCLUDED.hub_category
RETURNING ...
```

The `ON CONFLICT` clause updates `hub_id`/`hub_category` but **does NOT update `created_by`**. So if (hypothetically) two users could collide on `(entity_type, entity_id)` — they can't in practice because `entity_id` is a `gen_random_uuid()` from the upstream module — the original `created_by` is preserved. This is the safe behaviour, but the conditional shape suggests the developer was thinking about cross-user collisions.

The real risk surface is *zero* given UUID collision probability, so this is informational only.

---

### F-09 — `create_assistant_from_hub` accepts user-controlled override of `instructions` / `parameters`

**Severity:** **Info / Low**
**ASVS:** V5.1.3 (input validation), V8.3.4 (sensitive data handling)
**Files:** `handlers.rs:243-306`, `types.rs:46-74`

**Description.**
The request allows overriding `name`, `description`, `instructions`, and `parameters`:

```rust
let create_request = crate::modules::assistant::types::CreateAssistantRequest {
    name: request.name.unwrap_or(hub_assistant.name.clone()),
    description: request.description.or(hub_assistant.description.clone()),
    instructions: request.instructions.or(hub_assistant.instructions.clone()),
    parameters: request.parameters.and_then(...).or_else(...),
    ...
};
```

Functionally equivalent to calling `POST /assistants/` with these fields — so this is not a *new* security surface beyond what the upstream `assistant` module exposes. The hub is just a convenience layer. No validation is performed by the hub itself (length caps, charset). The upstream `assistant` module is responsible for those checks; auditing that is out of scope for this report.

**Recommendation (defense in depth):**
The hub layer is the right place to enforce a max-length cap on `instructions` (e.g., 16 KiB) and `name`/`description` (e.g., 256 / 4 KiB), independent of upstream. If upstream caps are added/changed, the hub layer's caps act as a backstop.

---

### F-10 — `create_mcp_server_from_hub` accepts `display_name` / `name` without length validation

**Severity:** **Info / Low**
**ASVS:** V5.1.3
**Files:** `handlers.rs:312-399`, `types.rs:77-94`

**Description.**
Same shape as F-09 — `name` and `display_name` flow into the upstream MCP create call. Hub does no validation; upstream is responsible. Recommended defense-in-depth backstop applies.

---

### F-11 — Refresh emits `HubEvent::*Refreshed` without rollback if write partially fails

**Severity:** **Info**
**ASVS:** V11.1.2 (transactional integrity)
**Files:** `handlers.rs:152-235`, `hub_manager.rs:277-313`

**Description.**
Refresh handlers:
1. read `old_version`,
2. call `refresh_hub_category` (which downloads + writes both `base.json` and `version.json`),
3. read `new_version`,
4. emit `HubEvent::*Refreshed { old_version, new_version }` if they differ.

Failure modes:
- If `base.json` write succeeds but `version.json` write fails (e.g., disk full mid-flow), `new_version` returns the old version, no event fires, but the catalog is now poisoned with the new `base.json` against the stale version directory. Wait — actually `update_category_files_from_github` writes `base.json` to `hub_dir/{category}/{version}/base.json` where `{version}` is the new version. So a partial failure leaves an orphan directory with the new `base.json` *not* yet pointed-at by `version.json`. Re-running the refresh fixes it; the system is not poisoned but the disk has stale data.

No security impact under the current model. Worth flagging as an integrity / cleanup concern.

---

### F-12 — `initialize()` writes embedded files unconditionally; no `.json` schema verification

**Severity:** **Info**
**ASVS:** V14.2.2 (verified components)
**Files:** `hub_manager.rs:31-86`

**Description.**
On every server boot, `initialize()` calls `copy_embedded_hub_files()` which writes every file from the `include_dir!`-baked tree into `<app_data>/hub/{category}/{CURRENT_HUB_VERSION}/` **without checking if the file already exists or matches.** This means:

- Embedded files always overwrite any user customisation in `<app_data>/hub/{category}/{CURRENT_HUB_VERSION}/` on restart.
- If an admin previously refreshed to a newer version, the embedded `base.json` is re-written to the `CURRENT_HUB_VERSION` directory (different path), but `write_version_file` does `if !version_path.exists()` so the live version pointer is preserved.

Net: no security regression. But the comment on line 50 says "Copy embedded files if not already present" — the implementation does NOT check "not already present". Documentation drift; worth fixing for behavioural clarity.

---

### F-13 — No moderation / abuse considerations for embedded catalog content

**Severity:** **Info**
**ASVS:** N/A (out of scope for technical controls)
**Files:** `resources/hub/{llm-models,assistants,mcp-servers}/1.0.0/base.json`

**Description.**
The catalogs are entirely curated by the project maintainers (embedded at compile time, refreshed from a project-controlled GitHub repo). There is no user-uploaded content surface — users **cannot publish** to the catalog, only consume. So spam / cross-user content visibility / moderation concerns in the audit brief do not apply.

What does apply: the maintainer-controlled GitHub repo IS the trust anchor (cf. F-01 — currently a placeholder), and any compromise to that anchor is system-wide. The decision NOT to allow user uploads is the correct security choice and should be preserved if a "publish your assistant" feature is later proposed (it would need a separate moderation surface).

---

### F-14 — `recommended_engine` and `recommended_engine_settings` from catalog flow into `EngineType::from_str` without enum-completeness check

**Severity:** **Info**
**ASVS:** V5.1.4 (input validation)
**Files:** `handlers.rs:495-501`

**Description.**

```rust
engine_type: hub_model
    .recommended_engine
    .and_then(|e| crate::modules::llm_model::models::EngineType::from_str(&e)),
engine_settings: hub_model
    .recommended_engine_settings
    .and_then(|s| serde_json::from_value(s).ok()),
```

Both use `Option::and_then(...).ok()` patterns — invalid values silently become `None`. This is *safer* than panicking but does silently drop user-meaningful catalog data. No security impact; UX concern.

---

### F-15 — No CSRF token / double-submit on state-changing POSTs

**Severity:** **Low**
**ASVS:** V4.2.2 (CSRF defense for state-changing requests)
**Files:** `routes.rs` (six POST endpoints)

**Description.**
The hub's six POST endpoints (`refresh_hub_*`, `create_*_from_hub`) rely on `RequirePermissions<…>` (which I assume uses bearer-token auth — see audit 01-§auth). If the bearer-token flow is `Authorization: Bearer …` only (no cookie-based session), CSRF is not exploitable; if a fallback cookie session exists, these state-changing POSTs need CSRF tokens.

Out-of-scope for this audit (auth shape is audited in 01-auth), but flagged because the hub's POST endpoints are higher-impact than a typical resource CRUD endpoint:
- `refresh_hub_*` reaches out to external services and writes to disk.
- `create_mcp_server_from_hub` spawns external processes downstream when the server is later used.
- `create_model_from_hub` initiates GB-scale downloads.

If CSRF is a concern in the auth model, these endpoints are priority targets.

---

## ASVS Coverage Matrix

| ASVS Section | Requirement | Coverage | Notes |
|---|---|---|---|
| **V4.1.1** Access control enforced at every layer | ✓ Pass | All routes wrapped in `RequirePermissions<T>` |
| **V4.1.3** Least-privilege enforcement | ✓ Pass | Separate permission per category × action (read / refresh / create / read_version) |
| **V4.1.5** Access control fails closed | ✓ Pass | Missing permission → 403 via the standard guard |
| **V4.2.1** Object-level authorization | △ Partial | `get_created_*` correctly scopes by `auth.user.id` for assistants & MCP servers (models are intentionally system-wide). F-05 weakens this for `provider_id` |
| **V4.2.2** CSRF defenses on state-changing requests | ? Unknown | Depends on auth shape (audit 01); flagged in F-15 |
| **V5.1.3** Input validation: bounded length | ✗ Fail | `lang`, `hub_id`, `name`, `display_name`, `instructions`, `description`, `version` — none bounded; F-02, F-04, F-09, F-10 |
| **V5.1.4** Strict typing for fields | △ Partial | `HubEntityType` / `HubCategory` enums are tight; `version` and `lang` are freeform strings |
| **V5.1.5** Bounded charset for path-significant inputs | ✗ Fail | F-02, F-04 |
| **V5.5.2** Rate-limit untrusted upstream calls | ✗ Fail | F-06 |
| **V5.5.4** Parse with size limits | ✗ Fail | F-07 (`bytes().await` is unbounded) |
| **V7.1.1** Sensitive data not in logs | ✓ Pass | No secrets logged; debug `tracing::debug!` on embedded-file copies is benign |
| **V7.4.1** Errors do not reveal sensitive system info | ✗ Fail | F-03 (PathBuf leakage) |
| **V8.3.4** Sensitive data classification | △ Partial | App-data path is leaked in error paths (F-03) |
| **V10.3.1** Signed update channel | ✗ Fail | F-01 (no signature, no checksum, no pin on refresh) |
| **V10.3.2** Verified third-party assets | ✗ Fail | F-01 (placeholder URL) |
| **V11.1.2** Business-logic flow integrity | △ Partial | F-08, F-11 (informational), F-05 (real) |
| **V11.1.4** Anti-automation on costly operations | ✗ Fail | F-06 (refresh), F-07 (download size) |
| **V12.3.1** Path traversal — filesystem access | ✗ Fail | F-02 (lang), F-04 (version) |
| **V12.4.1** Untrusted upload validation | ✗ Fail | F-01 (catalog JSON written without schema-validation first) |
| **V13.2.1** RESTful: validate parameters | ✗ Fail | F-02 |
| **V13.4.1** GraphQL/REST input validation | ✗ Fail | F-01, F-04 |
| **V14.2.1** Verify components | ✗ Fail | F-01 (no Sigstore on refresh) |

---

## Positive Findings

1. **`RequirePermissions<…>` on every route.** Twelve endpoints, twelve permission types, no bypass.
2. **Catalog data is read-only for users.** `create_*_from_hub` requests cannot inject `command`, `args`, `environment_variables`, `url`, or `headers` — those fields come exclusively from the on-disk catalog. This is the correct hardening choice and forecloses what would otherwise be a trivial RCE-via-MCP path.
3. **Hub does not bypass upstream module's per-user constraints.**
   - Assistant creation uses `Repos.assistant.create(Some(auth.user.id), …)` — assistants are scoped to the creating user.
   - MCP server creation uses `Repos.mcp.create_user_server(auth.user.id, …)` — user servers, not system servers. The handler docstring at line 367 explicitly calls this out: "hub interface only creates user servers, not system servers".
4. **`get_created_*_ids` filters correctly.**
   - `get_created_assistant_ids` filters `WHERE he.created_by = $1`.
   - `get_created_mcp_server_ids` filters `WHERE he.created_by = $1 AND (ms.user_id = $1 OR ms.is_system = true)` — correctly handles the system-MCP-server case.
   - `get_created_model_ids` is intentionally system-wide (models are shared, see migration 37 rationale).
5. **Cleanup handler is symmetric.** `CleanupHubEntitiesHandler` correctly listens for both `AssistantEvent::Deleted` and the MCP `SystemServerDeleted` / `UserServerDeleted` variants; no orphan hub_entities rows accumulate.
6. **Stdio downstream is allowlisted.** Even if a refresh poisoned the MCP catalog with arbitrary `command`, `mcp/client/stdio.rs:ALLOWED_COMMANDS` constrains spawned binaries to `npx, uvx, python, python3, node, deno`. (Note: `BLOCKED_ENV_VARS` in that file is non-exhaustive — see F-01 for `NODE_OPTIONS` / `HTTP_PROXY` gaps.)
7. **Migration 37 correctly removed `hub::models::*` from the default Users group.** The rationale comment is sound — multi-GB downloads onto a shared local provider are not a default-user action.
8. **No SSE / WebSocket / subscription surface.** The audit brief's concerns about cross-user event leak, replay protection, subscriber backpressure are N/A — events emitted are server-internal and do not flow to users.
9. **No persistent user-generated content.** The catalog is not user-uploadable; moderation / spam concerns do not apply.
10. **SQL queries use parameterised binds throughout.** No string concatenation into queries; `sqlx::query!` provides compile-time verification.

---

## Out of Scope / Deferred

- **Auth & permissions framework.** `RequirePermissions<T>` semantics, JWT validation, session lifecycle — covered in `01-auth-user-permissions-audit.md` (prior audit cycle).
- **Upstream modules:**
  - The `assistant` module's input validation on `name`, `instructions`, `parameters` — out of scope; flagged as backstop in F-09.
  - The `mcp` module's `validate_transport_config`, command allowlist, env blocklist, stdio spawn semantics — out of scope; mentioned only because the hub interface depends on them as defense-in-depth.
  - `llm_model::initiate_repository_download_internal` — its design as an explicit auth-bypass helper is sound for trusted internal callers; the hub's bypass weakens the boundary (F-05) but the helper itself is OK.
  - `llm_repository::find_by_url` — used to resolve hub model repositories; correctness of URL canonicalisation is in scope of the `llm_repository` audit.
- **CSRF / cookie session model** (F-15): depends on auth shape; defer to audit 01.
- **OpenAPI doc correctness.** Aide-generated docs return 401 with `()` body — body shape is not asserted by the auditor.
- **Embedded resource integrity.** `include_dir!` baked-in JSON is part of the build artifact; integrity is owned by the supply chain of the binary itself (covered by `code_sandbox`'s reproducibility / signing flow, not by the hub module).

---

## Remediation Priorities

| Priority | Finding | Effort | Notes |
|---|---|---|---|
| **P0** | F-01 (placeholder URL) | < 1 hr | Change URL string OR disable refresh routes until URL is configured. |
| **P0** | F-02 (lang traversal) | < 1 hr | 10-line `validate_locale` helper, gate on each handler. **This was flagged in the previous audit cycle and remains unfixed.** |
| **P1** | F-04 (version traversal) | < 1 hr | `validate_hub_version` helper. |
| **P1** | F-05 (provider_id not constrained) | < 2 hr | Cross-check against `list_local_providers` or add a `list_local_providers().contains(provider_id)` check. |
| **P2** | F-07 (no size cap on bytes()) | 1-2 hr | Switch to `bytes_stream()` with a 16 MiB accumulator. |
| **P2** | F-03 (path leakage in errors) | 2-3 hr | Replace each `.map_err(|e| AppError::internal_error(format!("... {:?} ...", path, e)))` with a `tracing::error!` + sanitized client error. |
| **P3** | F-06 (no rate limiting on refresh) | 1-2 hr | Plug `tower_governor` per-route. |
| **P3** | F-09, F-10 (backstop length validation) | 2-3 hr | Defense-in-depth caps in hub layer. |
| **P3** | F-11 (refresh atomicity) | 4-6 hr | Tmpfile + rename pattern. |
| **P3** | F-12 (doc/impl drift in initialize) | 30 min | Either honour the "if not already present" comment or update it. |
| **P4** | F-15 (CSRF gap if cookie auth exists) | unknown | Depends on auth shape. |

The combined effort to close F-01 / F-02 / F-04 / F-05 — the four Medium findings — is under one engineer-day.
