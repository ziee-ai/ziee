# Security Audit — LLM Model Module
**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/llm_model/` (~5,045 LOC) — model downloads, versioning, HF integration, metadata, file uploads
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target

---

## Executive Summary

The `llm_model` module exposes endpoints for: (a) CRUD over `llm_models` rows, (b) multipart **upload** of model weight files from disk, (c) **download** of models from arbitrary Git/LFS repositories (the canonical case being Hugging Face but the URL is fully under user control via the linked `llm_repository`), (d) **SSE** progress streaming for active downloads, and (e) per-download cancellation / deletion. The actual byte transport is delegated to `src-app/server/src/utils/git/{service,lfs/service}.rs`, both of which are exercised only from this module and llm_repository, so they are reviewed here as cross-boundary collaborators.

The previous (2025-11) audit `.sec-audits/04-llm-modules-audit.md` covered four modules at once and rated the LLM surface as **MODERATE RISK**. Re-walking the source seven months later finds that **every finding the 2025-11 audit raised against `llm_model` still applies unchanged**, *and* uncovers several new issues the prior pass missed:

1. The LFS service writes temp files into the **server's current working directory** with the filename derived from the LFS pointer's `oid` field — pointers come from a remote Git repo the attacker can control, and the path is built by `PathBuf::from("./").join(format!("{oid}.lfstmp"))` with no validation that `oid` is a hex string.
2. The LFS HTTP client follows redirects with `Authorization: Bearer <HF_TOKEN>` attached on the original request, and the HF token also lands in the URL credential slot (`url.set_password(token)`) — so any redirect the LFS server returns receives the credential.
3. There is **no per-user ownership** anywhere in the schema (`llm_models`, `download_instances`, `llm_model_files` have no `created_by` column). Every authenticated user with `llm_models::read` can list, observe and download-progress-stream **every** other user's models and downloads. There is no notion of "my downloads".
4. The download endpoint allows arbitrary Git URLs — anything stored in `llm_repositories.url` — and the only validation is `reqwest::Url::parse(...).is_ok()` (out-of-scope llm_repository module — but the attack surface lands here). `file://`, `http://169.254.169.254/`, `http://localhost:8080/admin`, and intranet hosts all parse as valid URLs.
5. The multipart `/llm-models/upload` route runs under the globally-applied `DefaultBodyLimit::disable()` (see `main.rs:172` / `lib.rs:197`) and reads each field with `field.bytes().await` into memory — *no per-file size, no per-request size, no field-count limit, no disk-space pre-check*.
6. The LFS HTTP client builds with `reqwest::Client::builder().build()?` (no timeout, no redirect cap, no TLS pinning). A slowloris-style LFS server can hold many downloads open forever, exhausting Tokio tasks.

**Risk: HIGH (Critical: 1, High: 6, Medium: 6, Low: 4, Info: 3)**

### Top 3 risks
1. **F-01 (Critical) — SSRF + arbitrary-host token leak in repo download.** `LlmRepository.url` is fully attacker-controlled (validated only as "parses as a URL"). The download path passes this URL straight to libgit2's `RepoBuilder::clone(&repository_url, …)` with the HF auth token attached as `Cred::userpass_plaintext(...)`. There is no allow-list of hosts, no block of `127.0.0.0/8` / `169.254.169.254` / RFC-1918, no block of `file://` / `git+ssh://` / etc. Combined with the LFS client's `url.set_password(token)` step that injects the token into *whatever* URL the LFS batch-API response says to fetch from (server-controlled `action.download.href`), the HF/Bearer token is exfiltratable by any repository the user can point us at.
2. **F-02 (High) — LFS path-traversal via attacker-controlled `oid`.** `lfs/service.rs:301-313` writes temp files using `PathBuf::from("./").join(format!("{oid}.lfstmp"))`. If a remote repo serves an LFS pointer whose `oid` line is `../../../etc/cron.d/runme`, the temp filename traverses out of the working directory. The pointer parser (`metadata.rs:48-65`) reads the `oid` as the last whitespace-split token of the line without any hex-character check.
3. **F-03 (High) — Multipart upload has no body / per-file / disk-space limits.** `main.rs:172` and `lib.rs:197` apply `DefaultBodyLimit::disable()` to the whole router (commented in source: "Disable body size limit for model uploads"). The upload handler then buffers each field with `field.bytes().await` into a `Vec<u8>` — one curl-streamed gzip-bomb-style field will OOM the server. There is no `free_space()` probe before starting a multi-GB download/upload either.

The codebase keeps the strong points the 2025-11 audit found: every endpoint uses `RequirePermissions<...>`, all DB access is via `sqlx::query!` macros (compile-time-checked), and the SSE auth happens before the stream is opened. Those mitigations are noted in `## Positive Findings`.

---

## Findings

### F-01 — SSRF + HF-token exfiltration via attacker-controlled repository URL
- **Severity:** **Critical**
- **ASVS:** V12.6.1 (SSRF defence), V9.2.1 (Outbound TLS to trusted endpoints), V5.2.5 (URL validation includes scheme & host)
- **CWE:** CWE-918 (Server-Side Request Forgery), CWE-200 (Sensitive Data Exposure)
- **Location:**
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:1044-1045` —
    `GitService::build_repository_url(&repository.url, &request.repository_path)`
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:1049-1063` —
    extracts `api_key` / `token` / `username:password` from the `LlmRepository.auth_config`
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:1167-1176` —
    `git_service.clone_repository(&repository_url, …, auth_token.as_deref(), …)`
  - `src-app/server/src/utils/git/service.rs:194-201, 367-376` — `callbacks.credentials(...)` attaches token to whatever host git2 contacts
  - `src-app/server/src/utils/git/lfs/service.rs:190-198, 232-240, 283-285` —
    `url_with_auth(url, access_token)` injects HF token into batch URL **and** into the server-returned `action.download.href`
  - Cross-boundary: `src-app/server/src/modules/llm_repository/utils.rs:14-23` — `validate_url(...)` only checks `reqwest::Url::parse(...).is_ok()`
- **Description:**
  Flow:
  1. An authenticated user with `llm_repositories::create` (or `llm_repositories::edit`) creates / edits a repository row with `url = http://169.254.169.254/...` (or `http://10.0.0.5:6379/`, or `file:///etc/`, or anything else that parses as a URL). The validator at `llm_repository/utils.rs:14` returns `Ok(())` for all of these.
  2. The same or a different user (with `llm_models::create`) calls `POST /llm-models/download` with `repository_id` pointing at that row.
  3. `initiate_repository_download_internal` reads `repository.url`, concatenates `repository_path` and feeds the result to libgit2's `RepoBuilder::clone`. libgit2 will happily attempt cloning from `http://169.254.169.254/...` — and **`callbacks.credentials` returns the user's `auth_token` to whatever credential prompt comes back** (the `_url` argument to the credential callback is ignored at `service.rs:195-200, 368-375`).
  4. For LFS files the situation is worse: the LFS batch endpoint at `<repo_url>/info/lfs/objects/batch` returns JSON containing `actions.download.href` — **a server-controlled URL**. The code at `lfs/service.rs:283-285` then does `let url = Self::url_with_auth(&action.download.href, access_token)?` followed by `client.get(url).headers(headers).send().await`. So a hostile LFS server can redirect the second hop (the actual download) to `https://attacker.tld/?bear=` and the HF token will be sent in `https://oauth2:<token>@attacker.tld/?bear=...`.
  5. Combine with the absence of any `is_loopback() / is_private()` check anywhere on the URL path and you have:
     - **SSRF against cloud metadata** (`http://169.254.169.254/latest/meta-data/iam/security-credentials/`)
     - **SSRF against intranet services** (`http://internal-redis:6379/...` — libgit2 will speak HTTP, so the response body is what comes back)
     - **HF / GitHub token theft** via crafted LFS server response
     - **`file:///` scheme** — libgit2 supports the `file://` transport and will happily clone from the local filesystem (`file:///root/.ssh/id_rsa` won't work directly because it's not a git repo, but `file:///tmp/attacker-staged-repo/` would).
- **PoC:**
  ```bash
  # Step 1: create a "repository" pointing at AWS metadata IP
  curl -X POST /api/llm-repositories \
    -H "Authorization: Bearer $JWT" \
    -d '{"name":"x","url":"http://169.254.169.254/latest","auth_type":"none","enabled":true}'

  # Step 2: trigger a download from it
  curl -X POST /api/llm-models/download \
    -H "Authorization: Bearer $JWT" \
    -d '{"provider_id":"...","repository_id":"...","repository_path":"meta-data/iam/","name":"x","display_name":"x","file_format":"gguf","main_filename":"creds.json"}'

  # Step 3: poll /api/llm-models/downloads — the error_message field will leak the AWS response body in many cases
  ```
- **Recommendation:**
  - Centralise outbound-URL validation in a single helper: parse with `url::Url::parse`, require `scheme ∈ {"https"}` for production / `{"https","http"}` for dev, reject `parsed.host_str()` that resolves to `is_loopback() || is_private() || is_link_local() || is_multicast() || is_documentation()` for IPv4 *and* IPv6 (also block `0.0.0.0/8`, `100.64.0.0/10` CGNAT, `169.254.0.0/16`, `192.0.0.0/24`, `198.18.0.0/15`).
  - **Resolve the host once, pin the IP, and pass that IP to libgit2** to defeat DNS rebinding (`reqwest` supports custom DNS resolvers; libgit2 does not — wrap libgit2 inside a TCP proxy or pre-resolve and `force-replace` the host in the URL).
  - Add an **explicit host allow-list** (`huggingface.co`, `*.huggingface.co`, `github.com`, `*.githubusercontent.com`, `gitlab.com`, etc.); deny everything else by default. Make this configurable per deployment.
  - For the LFS server-controlled `action.download.href`: **re-validate** it against the same allow-list / private-IP block before issuing the second request. Do **not** attach the bearer token unless the redirect target is in the allow-list.
  - Forbid `file://`, `git://` and `ssh://` schemes outright at the `LlmRepository.url` validator.

---

### F-02 — LFS path-traversal via attacker-controlled `oid` field in pointer
- **Severity:** **High**
- **ASVS:** V12.3.1 (File-path sanitisation), V5.1.5 (Reject deserialised input that contains path separators)
- **CWE:** CWE-22 (Path Traversal), CWE-73 (External Control of File Name or Path)
- **Location:**
  - `src-app/server/src/utils/git/lfs/service.rs:300-313`
    ```rust
    const TEMP_SUFFIX: &str = ".lfstmp";
    const TEMP_FOLDER: &str = "./";
    let tmp_path = PathBuf::from(TEMP_FOLDER).join(format!("{}{TEMP_SUFFIX}", &meta_data.oid));
    if randomizer_bytes.is_none() && tmp_path.exists() {
        fs::remove_file(&tmp_path).await?;
    }
    let temp_file = tempfile::Builder::new()
        .prefix(&meta_data.oid)
        .suffix(TEMP_SUFFIX)
        .rand_bytes(randomizer_bytes.unwrap_or_default())
        .tempfile_in(TEMP_FOLDER)
        .map_err(|e| LfsError::TempFile(e.to_string()))?;
    ```
  - `src-app/server/src/utils/git/lfs/metadata.rs:46-65` —
    ```rust
    let mut oid = *lines.get(OID_PREFIX).ok_or("Could not find oid-entry")?;
    let mut hash = None;
    if oid.contains(':') {
        let lines: Vec<_> = oid.split(':').collect();
        if lines.first().ok_or(...)? == &"sha256" { hash = Some(Hash::SHA256); }
        oid = *lines.last().ok_or(...)?;
    }
    ```
  - `src-app/server/src/utils/git/lfs/service.rs:172-188` — `get_cache_dir` also uses the `oid` to build a path: `.join(oid_1).join(oid_2)` where `oid_1 = &metadata.oid[0..2]` and `oid_2 = &metadata.oid[2..4]`.
- **Description:**
  An LFS *pointer file* in a remote Git repo has the canonical form:
  ```
  version https://git-lfs.github.com/spec/v1
  oid sha256:1234567890abcdef…
  size 12345
  ```
  The parser at `metadata.rs:48-58` extracts `oid` as the last whitespace-split token of the line containing the substring `oid`. **There is no character-set check.** If a hostile repo serves a pointer file like
  ```
  version https://git-lfs.github.com/spec/v1
  oid sha256:../../../../../../etc/cron.d/payload
  size 0
  ```
  then `meta_data.oid` becomes `../../../../../../etc/cron.d/payload`. The downstream `tmp_path = PathBuf::from("./").join(format!("{oid}.lfstmp"))` resolves to `./../../../../../../etc/cron.d/payload.lfstmp`. The code then **removes** that file (line 306: `fs::remove_file(&tmp_path).await?`) if it exists, and `tempfile::Builder::new().prefix(&meta_data.oid).suffix(".lfstmp").tempfile_in("./")` will *also* try to use the `oid` as a prefix — `tempfile` does validate that the prefix doesn't contain a path separator and will return an error there, so the *write* is blocked, but the **prior `remove_file` is not** — meaning a hostile pointer can delete any file the server-uid can write *before* the create fails.
  Additionally, `get_cache_dir` slices the first 4 bytes of `oid` as directory names. For an `oid` of `../etc`, `oid_1 = ".."` and `oid_2 = "/e"` — `create_dir_all` will happily create `.git/lfs/objects/../e/…`, which when canonicalised escapes the cache directory.
  The "current working directory" is whatever directory the operator launched the server from — for a typical systemd unit that's `/`, so the traversal is rooted at the filesystem root.
- **Recommendation:**
  - At parse time: reject any `oid` that is not exactly `[0-9a-fA-F]{64}` for SHA-256, or `[0-9a-fA-F]{40}` for SHA-1. Anything else → `LfsError::InvalidFormat`.
  - For the temp directory: use `std::env::temp_dir()` or a configured cache root, never `"./"`. Combine with `tempfile_in` and a fixed prefix string, putting the `oid` only into the *suffix* and only after validation.
  - For `get_cache_dir`: assert `oid.len() == 64 && oid.chars().all(|c| c.is_ascii_hexdigit())` before slicing.

---

### F-03 — No body / file / disk-space limits on the upload route
- **Severity:** **High**
- **ASVS:** V12.1.1 (Per-route upload size limit), V12.3.5 (Reject if free space < expected)
- **CWE:** CWE-770 (Allocation Without Limits), CWE-400 (Uncontrolled Resource Consumption)
- **Location:**
  - `src-app/server/src/main.rs:172` — `.layer(axum::extract::DefaultBodyLimit::disable())`
  - `src-app/server/src/lib.rs:197` — same
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:514-865` —
    `upload_multiple_files_and_commit` reads multipart fields with `field.bytes().await` and stores in `Vec<u8>` (line 561, 571). There is no per-field size cap, no total-request cap, no file-count cap.
  - `src-app/server/src/modules/llm_model/storage.rs:101-176` — `save_temp_file` writes the entire `data: &[u8]` to disk via `tokio::fs::write(&file_path, data).await` with no atomic-write or disk-space check.
- **Description:**
  `DefaultBodyLimit::disable()` is applied to the *entire* router because model files can be very large (multi-GB). However:
  1. Each multipart field is buffered into memory before being persisted to disk. A 50 GB single-field upload allocates 50 GB of resident heap.
  2. No `tokio::fs::metadata` / `nix::sys::statvfs` check before write — the server happily fills the disk.
  3. No file-count limit (you can post 100 000 fields named `files`, each tiny, and exhaust file descriptors / inode budget).
  4. No global counter of concurrent uploads (the 2025-11 audit raised MED-2 for downloads; uploads have the same hole).
- **Attack scenarios:**
  - One curl `POST /llm-models/upload` with a 50 GB body → kernel OOM-kills the server.
  - 1 000 small uploads in a loop → disk fills, services dependent on `/var/log` die.
- **Recommendation:**
  - Replace `DefaultBodyLimit::disable()` with **per-route** `RequestBodyLimit` layers: keep the default 2 MiB for everything except `/llm-models/upload` and `/files/upload`, then apply a higher (configurable, e.g. 50 GB) limit only on those.
  - Stream uploaded fields **directly to disk** using `axum`'s `Multipart::next_field` + `Field.chunk()` (returns `Option<Bytes>` chunks) and `tokio::io::AsyncWriteExt::write_all`, never materialising the field in memory.
  - Before starting the write, call `statvfs` (`fs2::available_space(&base_path)`) and reject if free space < expected_size × 1.5.
  - Cap concurrent uploads per user and globally (semaphore in `LlmModelModule`).
  - Cap total field count per request (e.g., 256).

---

### F-04 — No per-user ownership on models or downloads
- **Severity:** **High**
- **ASVS:** V4.1.3 (Per-user ACL on resources), V4.2.1 (Tenant isolation)
- **CWE:** CWE-639 (Authorization Bypass Through User-Controlled Key), CWE-284 (Improper Access Control)
- **Location:**
  - `migrations/00000000000004_create_llm_models_tables.sql` — `llm_models` and `llm_model_files` have no `created_by` / `user_id` column
  - `migrations/00000000000005_create_download_instances_table.sql` — `download_instances` has no `created_by` column
  - `src-app/server/src/modules/llm_model/handlers/models.rs:35-67` —
    `list_models` returns *all* models for any user with `llm_models::read`
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:124-147` —
    `list_all_downloads` returns *all* downloads
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:163-178` —
    `get_download` returns any download by id
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:194-284` —
    `cancel_download` lets any user with `llm_models::downloads_cancel` cancel any download
  - `src-app/server/src/modules/llm_model/handlers/models.rs:172-219` —
    `delete_model` lets any user with `llm_models::delete` delete any model and its on-disk files
- **Description:**
  This is more than a missing feature — it is a deliberate design assumption that models are a **shared, admin-curated** resource pool. That assumption is fine for a single-tenant deployment, but it breaks the moment you have two non-admin users on the same server:
  - User A starts a 50 GB download. User B with `downloads_cancel` cancels it (e.g., to free bandwidth for their own work, or maliciously).
  - User B uploads a model named `"my-secret-finetune"`. User A can `GET /llm-models` and discover the existence + display_name + provider_id, then read every file at `<app_data>/models/<provider_id>/<model_id>/` (no per-user ACL on disk either).
  - User B's `DownloadRequestData` (which includes `description`, `display_name`, `engine_settings` JSON) is fully visible to every user via the SSE subscribe endpoint.
- **Recommendation:**
  - Add a `created_by UUID NOT NULL REFERENCES users(id)` column to `llm_models` and `download_instances`. Backfill existing rows to a dedicated "system" user owned by the bootstrap admin.
  - In every handler that returns a model or download by id, check `model.created_by == auth.user.id || auth.user.is_admin`. For lists, filter by `created_by`.
  - On `delete_model`: refuse unless the caller is the owner *or* an admin.
  - Alternative if "shared model pool" is the desired product behaviour: keep the global view but introduce a separate `llm_models::admin_manage` permission that gates *all* state-changing routes, and downgrade `llm_models::edit/delete` to per-owner semantics.
  - Either way: per-user-isolated *download progress* (drop the SSE broadcast pattern at `downloads.rs:486-517` in favour of a per-client filter).

---

### F-05 — Database error contents leaked to client
- **Severity:** **High**
- **ASVS:** V7.4.1 (Generic error responses), V8.2.3 (No sensitive info in error pages)
- **CWE:** CWE-209 (Information Exposure Through an Error Message)
- **Location:**
  - `src-app/server/src/common/type.rs:109-115` —
    ```rust
    pub fn database_error(err: impl std::error::Error) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SYSTEM_DATABASE_ERROR",
            format!("Database error: {}", err),  // ❌ raw sqlx::Error formatted into client-visible message
        )
    }
    ```
  - `src-app/server/src/modules/llm_model/repository.rs:42-43, 49-50, 66-69, 73-77, 84-88, 90-95` — every repository method wraps `sqlx::Error` via `AppError::database_error` which then becomes the response body
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:294-308` —
    ```rust
    let model_db = repo.create(create_request).await.map_err(|e| {
        let error_str = e.to_string();
        tracing::warn!("Database error during model creation: {}", error_str);
        if error_str.contains("llm_models_provider_id_name_unique") || ... { ... }
        else { e } // ❌ raw AppError (which contains the formatted sqlx::Error) returned to client
    })?;
    ```
- **Description:**
  When a SQL query fails (constraint violation, syntax error, type-mismatch, connection-lost-with-snippet, etc.), the `sqlx::Error::Database(db_err)` Display impl includes the **raw Postgres error message**, which contains table names, column names, constraint names, and sometimes (for syntax errors) the SQL fragment that was rejected. `AppError::database_error` formats that into the response message as `"Database error: <full sqlx error>"`, returned with HTTP 500 and `error_code = "SYSTEM_DATABASE_ERROR"`. This is the same pattern flagged in the 2025-11 audit (HIGH-4) — **still unfixed**.
  The duplicate-key-special-case at `uploads.rs:296-304` *does* sanitize one specific case, but **echoes the user-supplied `model_name` back into the error message** (`format!("A model with the name '{}' already exists ...", model_name)`), which means a malicious request body can choose error-message strings.
- **Recommendation:**
  - Change `AppError::database_error` to never include the err in the user-visible message: `format!("Database error: {}", err)` → `"Database operation failed"` for the public field, and log the detail server-side only.
  - In each handler's `.map_err`, never return `e` unchanged from a DB call — always rewrap into a generic `AppError`.
  - For the duplicate-key path, don't echo user input back: return a stable message `"Model name conflicts with an existing model for this provider."`.

---

### F-06 — Bearer token leak via libgit2 credential callback ignoring host
- **Severity:** **High**
- **ASVS:** V9.2.3 (Credentials are scoped to the host they were issued for), V2.10.4 (Don't send credentials to redirect targets without revalidation)
- **CWE:** CWE-200 (Information Exposure), CWE-601 (URL Redirection to Untrusted Site)
- **Location:**
  - `src-app/server/src/utils/git/service.rs:195-201` —
    ```rust
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        if let Some(token) = auth_token.as_deref() {
            Cred::userpass_plaintext(username_from_url.unwrap_or(""), token)
        } else {
            Cred::default()
        }
    });
    ```
  - `src-app/server/src/utils/git/service.rs:367-376` — identical code path for the clone branch
- **Description:**
  libgit2 calls the credential callback **every time it hits HTTP authentication** during a clone/fetch — including for HTTP redirects to a different host. The closure here ignores the `_url` argument and unconditionally returns the user's `auth_token`. If the configured `repository_url` (which can be a deliberately misleading domain because there is no host allow-list, see F-01) redirects to `https://evil.example.com/`, libgit2 will re-issue the same credentials there. This is the same class of bug as `curl --location` without `--location-trusted` semantics.
  Hugging Face's tokens are *write*-scoped for many users; GitHub tokens often have `repo` scope. Leaking either gives the attacker the ability to push code under the victim's identity.
- **Recommendation:**
  - In the credential callback, inspect the `url` parameter against the allow-list established by F-01. If the URL is outside the allow-list, return `Err(git2::Error::from_str("untrusted host"))` instead of credentials.
  - Disable HTTP redirects globally on the libgit2 client (`git_config.set_str("http.followRedirects", "false")` if upstream supports it; otherwise wrap in a HTTP proxy that strips `Location` headers).

---

### F-07 — No timeout / redirect cap on LFS HTTP client
- **Severity:** **Medium**
- **ASVS:** V9.2.1 (Outbound HTTP clients use timeouts), V12.3.6 (Retries / backoff have a ceiling)
- **CWE:** CWE-400 (Resource Exhaustion), CWE-770
- **Location:**
  - `src-app/server/src/utils/git/lfs/service.rs:210` — `let client = Client::builder().build()?;`
- **Description:**
  `reqwest::Client::builder().build()` returns a client with **no** read timeout, **no** request timeout, default 10 hops of redirect, **no** TCP connect timeout, **no** TLS configuration. A hostile LFS server can:
  - Open the TCP connection then drip-feed 1 byte / minute → the task hangs until the OS times out (hours) and the cancellation token is the only escape.
  - Return an infinite chunked-encoding stream → fills disk (combined with F-03) and never finishes.
  - Return HTTP 301 → HTTPS redirect-chain back to the same URL → 10 hops then fails (acceptable) but with the credential attached at every hop (see F-06).
- **Recommendation:**
  ```rust
  let client = Client::builder()
      .connect_timeout(Duration::from_secs(10))
      .timeout(Duration::from_secs(30 * 60))         // 30 min absolute cap per file
      .read_timeout(Duration::from_secs(60))         // (reqwest 0.12+)
      .redirect(Policy::limited(3))
      .https_only(true)                              // unless intentionally permitting HTTP
      .build()?;
  ```
  Also: enforce a **min-bytes-per-second** floor by tracking progress and aborting if speed < 1 KB/s for > 60 s.

---

### F-08 — Filename sanitization is incomplete and trivially bypassed
- **Severity:** **Medium**
- **ASVS:** V12.3.1 (Filename normalisation), V5.1.5
- **CWE:** CWE-22, CWE-178 (Improper Handling of Case Sensitivity)
- **Location:**
  - `src-app/server/src/modules/llm_model/storage.rs:127-131` —
    ```rust
    let safe_filename = filename
        .replace('/', "_")
        .replace('\\', "_")
        .replace("..", "_");
    ```
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:554-559` — multipart `file_name()` taken, `Path::file_name()` applied, but the *original* `field.file_name()` (not the post-`Path::file_name()` form) is what's later passed to `save_temp_file`. The line is:
    ```rust
    let filename = std::path::Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_name)  // ⚠ falls back to the *raw* file_name on any error
        .to_string();
    ```
- **Description:**
  - The `replace("..", "_")` step does **not** prevent `....//` → after replacing `..` with `_` you get `__//` → after replacing `/` you get `____` (this particular composition is safe), **but** Unicode dot-look-alikes (`\u{2024}` one-dot leader, `\u{ff0e}` full-width period) and URL-encoded variants pass through:
    - `..%2f..%2fetc%2fpasswd` → on a decode pass (none currently exists in this path) would become `../../etc/passwd`. Multipart filename is sent raw, so direct `..` is caught, but the multipart parser does NOT URL-decode → the literal string `%2e%2e/` lands as the filename and bypasses the `replace("/", "_")` (the `/` was sent as `%2f`).
    - Backslash-encoded variants (`%5c`) similarly bypass.
  - Filenames starting with `.` are not blocked → `.git/config` (after `/` replaced becomes `.git_config`, then after `..` replaced is unchanged — leaks a "hidden" file into the model directory but doesn't escape it; lower-impact).
  - No length cap → 4096-char filenames pass through.
  - The fallback at `uploads.rs:558` (`.unwrap_or(file_name)`) defeats the `Path::file_name()` extraction if the path string happens to have no terminal component (e.g., the multipart sender sent `filename="../"` → `Path::file_name` returns `None` → fallback returns the raw `"../"`).
- **Recommendation:**
  - Replace the ad-hoc sanitizer with a single helper:
    ```rust
    fn sanitize_filename(raw: &str) -> Result<String, AppError> {
        if raw.is_empty() || raw.len() > 255 { return Err(...); }
        // Percent-decode in case sender encoded it
        let decoded = percent_encoding::percent_decode_str(raw).decode_utf8()
            .map_err(|_| ...)?;
        // Take final path component only
        let p = std::path::Path::new(decoded.as_ref());
        let name = p.file_name().and_then(|n| n.to_str())
            .ok_or(AppError::bad_request("INVALID_FILENAME", "Invalid"))?;
        // Reject hidden, traversal, control chars, separators
        if name.starts_with('.') || name.contains('/') || name.contains('\\')
            || name.chars().any(|c| c.is_control())
            || name == ".." || name == "."
        {
            return Err(AppError::bad_request("INVALID_FILENAME", "Invalid"));
        }
        Ok(name.to_string())
    }
    ```
  - Reject (not sanitize) — surfacing the error to the client is fine here.

---

### F-09 — `validate_file_content` returns issues but never blocks
- **Severity:** **Medium**
- **ASVS:** V12.3.2 (Reject mismatched content-type / magic-byte combinations)
- **CWE:** CWE-434 (Unrestricted Upload of File with Dangerous Type)
- **Location:**
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:797-798, 916-960`
    ```rust
    let _file_type = determine_model_file_type(&filename);
    let _validation_issues = validate_file_content(&filename, &file_data);
    // ... no check that issues.is_empty()
    storage.save_temp_file(...).await
    ```
- **Description:**
  `validate_file_content` checks for: empty files, weight files under 1 KB, invalid JSON in `*config.json` and `tokenizer.json`, and an HTML-magic-bytes prefix (`<!DOCTYPE`, `<html`, `<HTML`). When any issue is detected it is appended to a `Vec<String>` that is then **discarded** (`let _validation_issues = ...`). Upload proceeds unconditionally. So:
  - HTML error pages from a misconfigured upstream get stored as `model.safetensors`.
  - 0-byte and 100-byte "model weight files" land on disk.
  - Random binary garbage marked as `.gguf` becomes a "model" that will crash the local runtime on activation.
  - There is no actual magic-byte check for the formats that *should* validate (`gguf` has the magic `GGUF` at byte 0; safetensors has a JSON header preceded by an 8-byte little-endian length; pytorch `.bin` files start with the pickle magic `\x80\x02`). None of these are checked.
- **Recommendation:**
  Make `validate_file_content` block on any issue, **and** add magic-byte checks per `FileFormat`:
  ```rust
  fn assert_magic(file_format: FileFormat, data: &[u8]) -> Result<(), AppError> {
      match file_format {
          FileFormat::Gguf => {
              if data.len() < 4 || &data[..4] != b"GGUF" {
                  return Err(AppError::bad_request("FORMAT_MISMATCH", "Not a GGUF file"));
              }
          }
          FileFormat::Safetensors => {
              if data.len() < 8 { ... }
              let header_len = u64::from_le_bytes(data[..8].try_into().unwrap());
              if header_len as usize > data.len() - 8 { ... }
              // header must be JSON
              serde_json::from_slice::<serde_json::Value>(&data[8..8+header_len as usize])
                  .map_err(|_| ...)?;
          }
          FileFormat::Pytorch => {
              // pickle protocol 2 magic
              if data.len() < 2 || data[0] != 0x80 { ... }
          }
      }
      Ok(())
  }
  ```
  Also reject HTML-prefixed files outright (it's a misconfigured download, never a legitimate model).

---

### F-10 — `find_existing_in_progress` lets users hijack other users' downloads
- **Severity:** **Medium**
- **ASVS:** V4.2.2 (Resource binding to caller), V13.1.4 (Idempotency keys are caller-scoped)
- **CWE:** CWE-639 (Authorization Bypass Through User-Controlled Key)
- **Location:**
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:992-1006` —
    ```rust
    if let Some(existing_download) = Repos.download_instance.find_existing_in_progress(
        request.repository_id, request.provider_id,
        &request.repository_path, &request.main_filename,
    ).await? {
        return Ok(existing_download);
    }
    ```
  - `src-app/server/src/modules/llm_model/repository.rs:927-961` — `find_existing_in_progress_download` looks up by `(repository_id, provider_id, repository_path, main_filename, status IN ('pending','downloading'))` — *no user filter*.
- **Description:**
  When user A starts a download and user B subsequently submits a `POST /llm-models/download` with the same `(repository_id, provider_id, repository_path, main_filename)`, the response handed back to user B is **user A's `DownloadInstance`** — including its id. User B can then:
  - Cancel it (`POST /llm-models/downloads/{id}/cancel`, since they have `downloads_cancel`) — denial of A's work.
  - Observe its full request_data (display_name, description, engine_settings) — info disclosure.
  - Inherit a download they never started, then claim "I downloaded that model".
  Combined with F-04 (no per-user ownership), this gives a stranger the ability to interfere with anyone's downloads given knowledge of the (repo, provider, path, filename) tuple — which is observable via `GET /llm-models/downloads`.
- **Recommendation:**
  - Once F-04 lands, add `AND created_by = $5` to the dedup query.
  - Until then: rather than returning the *existing* download, return `409 Conflict` with a generic "a similar download is already in progress" message and no body details.

---

### F-11 — SSE broadcasts every download's state to every connected client
- **Severity:** **Medium**
- **ASVS:** V4.2.1 (Tenant isolation in streaming endpoints)
- **CWE:** CWE-200
- **Location:**
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:351-402` — `subscribe_download_progress`
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:409-484` — `start_download_monitoring`
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:486-517` — `broadcast_event` sends *the same* `Event` to every entry in `SSE_CLIENTS`.
- **Description:**
  Every user with `llm_models::downloads_read` who hits `GET /llm-models/downloads/subscribe` joins a single broadcast pool. The background monitor task (`start_download_monitoring`) polls `Repos.download_instance.get_all_active()` every 2 s and broadcasts the entire list to all clients. So user B sees user A's `display_name`, `provider_id`, `progress_data.message` (which can include the repository URL — confirm at `service.rs:188, 447`).
  In addition, `SSE_CLIENTS` is held behind `std::sync::Mutex` (not `tokio::sync::Mutex`) and locked across `.await` points indirectly — at `downloads.rs:362-365` the lock is dropped before the await, which is correct, but the broadcast at `downloads.rs:500-504` holds a borrowed `clients.iter()` while sending; if any `tx.send()` blocks on a full channel (it's unbounded so it won't, but switching to bounded later would deadlock), this becomes a problem.
- **Recommendation:**
  - Filter the broadcast per-client: in the per-client `tokio::sync::mpsc::UnboundedSender`, additionally store the `user_id` and the user's `is_admin` flag; in `broadcast_event`, send each event only to clients whose user owns the download or is an admin (this requires F-04 first).
  - Move `SSE_CLIENTS` from `Mutex` to `tokio::sync::RwLock` to allow concurrent reads.

---

### F-12 — Download `request_data` JSON is unconstrained in size and depth
- **Severity:** **Medium**
- **ASVS:** V5.1.4 (Deserialised JSON depth / size is bounded)
- **CWE:** CWE-400, CWE-770
- **Location:**
  - `src-app/server/src/modules/llm_model/models.rs:837-852` — `DownloadRequestData` includes `Option<ModelEngineSettings>` which itself contains arbitrary strings and nested options; serialised as JSONB into Postgres
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:1015-1033` — the request body is deserialised directly via `Json<DownloadFromRepositoryRequest>` without a per-field length cap
- **Description:**
  - `engine_settings.mistralrs.chat_template` is `Option<String>` with no max length. A 1 GB chat template payload deserialises fine (memory-bounded only by `DefaultBodyLimit`, which is disabled — see F-03).
  - `parameters.stop: Option<Vec<String>>` is validated to have ≤ 4 entries ≤ 32 chars each, which is correct.
  - `description`, `display_name`, `name`, `repository_path`, `main_filename`: all unbounded `String`. The DB enforces `VARCHAR(255)` on `name` and `display_name` so insert will fail late, but the JSON deserialisation has already happened.
- **Recommendation:**
  - Validate every string field length at deserialisation boundary (e.g., custom `Deserialize` impl or a `validate()` method called early in the handler).
  - Cap `chat_template` to ≤ 64 KiB.
  - Cap JSON nesting depth via `serde_json::de::Deserializer::from_slice(...).disable_recursion_limit()` set to a sane 16.

---

### F-13 — Temp directory cleanup is dead code
- **Severity:** **Low**
- **ASVS:** V12.4.2 (Stale files cleaned)
- **CWE:** CWE-459 (Incomplete Cleanup)
- **Location:**
  - `src-app/server/src/modules/llm_model/storage.rs:180-246` — `clear_temp_directory` is defined but a repo-wide search shows zero callers.
- **Description:**
  The temp directory `{APP_DATA_DIR}/temp/{session_id}/` accumulates session dirs from every upload that succeeded *or failed*. There is no scheduled cleanup, no on-startup wipe, no on-shutdown wipe. Disk usage grows monotonically.
  Additionally, on a successful upload, the temp session dir is not deleted after `create_model_with_files` copies the files out (the code at `uploads.rs:818-822` and `create_model_with_files` at `uploads.rs:130-341` copy from the temp dir but never remove it).
- **Recommendation:**
  - Wire `ModelStorage::clear_temp_directory()` into the server startup sequence in `lib.rs` (call it after `init_app_data_dir`).
  - After `create_model_with_files` succeeds, delete the temp session directory.
  - Add a scheduled task that wipes temp session dirs older than 24 h.

---

### F-14 — Static `Mutex` and `lazy_static` SSE state leaks across logical scopes
- **Severity:** **Low**
- **ASVS:** V8.3.1 (Shared state has clear ownership)
- **CWE:** CWE-362 (Race Condition), CWE-457 (Use of Uninitialised Variable, here: stale shared state)
- **Location:**
  - `src-app/server/src/modules/llm_model/handlers/downloads.rs:112-115` —
    ```rust
    lazy_static::lazy_static! {
        static ref SSE_CLIENTS: Mutex<HashMap<ClientId, ...>> = Mutex::new(HashMap::new());
        static ref MONITORING_ACTIVE: Mutex<bool> = Mutex::new(false);
    }
    ```
- **Description:**
  - The two statics are module-level. Tests that exercise SSE will pollute each other through the shared `SSE_CLIENTS` map; integration tests that re-init the module won't clear it. Adding `--test-threads=1` per CLAUDE.md helps but doesn't fix the leak between sequential tests.
  - `lazy_static` is unmaintained in favour of `once_cell::sync::Lazy` (which the codebase already uses for `CANCELLATION_TRACKER` at `cancellation.rs:81`). Inconsistency.
  - `MONITORING_ACTIVE` is a `Mutex<bool>` — toggling it requires holding the mutex, but the code drops the guard mid-function (`downloads.rs:415, 435, 477`), opening a small window where two concurrent subscribe calls both observe `false`, both flip to `true`, both spawn monitoring tasks. With one user this is benign (two 2 s polls instead of one); under load it's resource-wasting.
- **Recommendation:**
  - Replace `lazy_static!` with `once_cell::sync::Lazy` for consistency.
  - Replace `Mutex<bool>` with `std::sync::atomic::AtomicBool` and use `compare_exchange` for the start-monitoring transition.
  - Bind the SSE client pool to the `LlmModelModule` instance and inject via `Extension`, so test isolation works.

---

### F-15 — `println!` calls in production code paths
- **Severity:** **Low**
- **ASVS:** V7.1.2 (No `println` / `eprintln` in production)
- **CWE:** CWE-532 (Insertion of Sensitive Information into Log File)
- **Location:**
  - `src-app/server/src/utils/git/service.rs:209, 384` — `println!("Git fetch cancelled by user")` / `println!("Git clone cancelled by user")`
  - `src-app/server/src/utils/git/service.rs:578-582` — `println!("Pulling LFS files from repository: {} with paths: {:?}", ...)`
  - `src-app/server/src/utils/git/lfs/service.rs:578` — repo path printed (potentially user-derived)
- **Description:**
  These bypass the `tracing` log filter, write to stdout (which may be captured to journald with no severity), and the LFS variant prints **file paths** that include the cache-dir name → leaks the absolute path of the app data dir, which aids exploitation of F-02 / F-04.
- **Recommendation:**
  Replace each `println!` with `tracing::debug!` (cancellation message) or `tracing::trace!` (paths).

---

### F-16 — `unwrap()` on parsed enum values can crash a worker
- **Severity:** **Low**
- **ASVS:** V8.3.1
- **CWE:** CWE-754 (Improper Check for Unusual or Exceptional Conditions)
- **Location:**
  - `src-app/server/src/modules/llm_model/repository.rs:244, 248, 297, 301, 356, 361, 429, 433` —
    `EngineType::from_str(&r.engine_type).unwrap()` and `FileFormat::from_str(&r.file_format).unwrap()`
  - Same panics at every `get_llm_model_by_id` / `list_all_llm_models` / `list_llm_models_by_provider` / `create_llm_model` row mapping
  - `src-app/server/src/modules/llm_model/repository.rs:235, 236, 288, 289, 347, 348, 419, 420` — `DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap()` — the only way this can fail is for years outside `±262144` which is unreachable, so this one is fine; the enum `unwrap()`s are the issue.
- **Description:**
  The DB column has a `CHECK` constraint enforcing `engine_type IN ('mistralrs','llamacpp','none')` and `file_format IN ('safetensors','pytorch','gguf')`. If a future migration adds a value and the DB gets ahead of the binary (rolling-deploy scenario), the binary will panic-unwind every read against the table — not just one request, *every* read, because every list/get returns at least one row. That's a hot crash loop.
- **Recommendation:**
  Replace `unwrap()` with `unwrap_or(EngineType::None)` and `unwrap_or(FileFormat::Safetensors)` and log a warning at read time, **or** return `Err(AppError::internal_error(...))` to fail the request cleanly without unwinding.

---

### F-17 — Stale `clear_cache: bool` flag is reachable in production
- **Severity:** **Low**
- **ASVS:** V14.1.2 (Test-only configuration not reachable from prod)
- **CWE:** CWE-489 (Active Debug Code)
- **Location:**
  - `src-app/server/src/modules/llm_model/handlers/uploads.rs:507-510` —
    ```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Clear cached repository before downloading (for testing)")]
    pub clear_cache: Option<bool>,
    ```
- **Description:**
  The `clear_cache: true` flag forces re-clone (`uploads.rs:1107-1119`), defeating the deduplication of repo data on disk. Any authenticated user can set this and trigger a full re-clone of a multi-GB repo every time, amplifying any other DoS by Nx. The schema description openly says "for testing" yet there is no `cfg!(debug_assertions)` gate.
- **Recommendation:**
  - Gate behind `if cfg!(debug_assertions) && request.clear_cache.unwrap_or(false)` so the flag is a no-op in release builds, **or**
  - Require `is_admin` to honour the flag.

---

### F-18 — No content-type / file-extension allow-list on uploaded fields
- **Severity:** **Info**
- **ASVS:** V12.3.2
- **CWE:** CWE-434
- **Location:** `src-app/server/src/modules/llm_model/handlers/uploads.rs:553-572`
- **Description:**
  The handler accepts any filename / any content. While `determine_model_file_type` categorises the file by extension (weight/index/config/tokenizer/vocab/unknown), files of type `UnknownFile` are stored anyway. Combined with F-08 sanitisation gaps, you could upload `payload.sh` and it would be stored as-is at `<model_dir>/payload.sh`. There is no execution path on these files from the server side (mistralrs/llamacpp won't `exec` them), so impact is currently zero — but defence-in-depth recommends restricting to a known set of extensions.
- **Recommendation:**
  Allow-list of extensions per `FileFormat`:
  ```rust
  fn allowed_for(fmt: FileFormat) -> &'static [&'static str] {
      match fmt {
          FileFormat::Safetensors => &[".safetensors", ".json", ".txt", ".model"],
          FileFormat::Pytorch     => &[".bin", ".pt", ".pth", ".json", ".txt", ".model"],
          FileFormat::Gguf        => &[".gguf", ".json", ".txt"],
      }
  }
  ```
  Reject everything else.

---

### F-19 — Concurrent identical downloads still allowed when one is in `Failed` state
- **Severity:** **Info**
- **Location:** `src-app/server/src/modules/llm_model/repository.rs:927-961`
- **Description:**
  `find_existing_in_progress_download` matches only `status IN ('pending', 'downloading')`. If a download exists with `status='failed'`, a new identical download is created instead of being deduped to the failed one. The DB row count grows. This is more an interaction concern than a security one — flagged for completeness.
- **Recommendation:** Either widen the dedup window to include `failed` (with a TTL window) or surface a "retry" semantic in the API instead of letting the client spam re-create.

---

### F-20 — Missing audit log for sensitive operations
- **Severity:** **Info**
- **ASVS:** V7.1.3 (Audit log includes user, action, resource, timestamp)
- **Location:** module-wide
- **Description:**
  None of `create_model`, `delete_model`, `enable_model`, `disable_model`, `initiate_repository_download`, `cancel_download`, `delete_download` write to an audit log. `tracing::info!` lines exist but are mixed with debug noise and have no structured user/resource fields. The 2025-11 audit raised this (LOW-2) as a backlog item; still relevant.
- **Recommendation:**
  Adopt a single `audit::log_event(user_id, action, resource_type, resource_id, json_details)` helper and call it from every state-changing handler.

---

## ASVS Coverage Matrix

| Chapter | Control | Status | Evidence |
|---|---|---|---|
| V4.1.3 | Per-user resource ACL | **FAIL** | F-04 — no `created_by` |
| V4.1.5 | Permission check on every protected route | PASS | all routes wrap `RequirePermissions<...>` |
| V4.2.1 | Tenant isolation | **FAIL** | F-04, F-10, F-11 |
| V4.2.2 | Resource binding to caller | **FAIL** | F-10 (download hijack) |
| V5.1.4 | Bounded deserialisation | **FAIL** | F-12 |
| V5.1.5 | Reject path-separator input | **PARTIAL** | F-02 (oid), F-08 (filename) |
| V5.2.3 | Sanitise input to interpreters | n/a | no interpreter invocation in this module |
| V7.1.2 | No `println` in prod | **FAIL** | F-15 |
| V7.1.3 | Audit log | **FAIL** | F-20 |
| V7.4.1 | Generic error responses | **FAIL** | F-05 |
| V8.2.3 | No internal info in errors | **FAIL** | F-05 |
| V8.3.1 | Shared state ownership | **PARTIAL** | F-14 (static SSE pool) |
| V9.2.1 | Outbound HTTP timeouts | **FAIL** | F-07 |
| V9.2.3 | Credentials scoped to host | **FAIL** | F-06 |
| V12.1.1 | Per-route upload size limit | **FAIL** | F-03 |
| V12.3.1 | Filename normalisation | **FAIL** | F-02, F-08 |
| V12.3.2 | Content/magic-byte verification | **FAIL** | F-09, F-18 |
| V12.3.5 | Disk-space pre-check | **FAIL** | F-03 |
| V12.3.6 | Retry / backoff ceiling | **FAIL** | F-07 |
| V12.4.2 | Stale temp file cleanup | **FAIL** | F-13 |
| V12.5.2 | File processing isolation | n/a | model files are not interpreted in this module |
| V12.6.1 | SSRF defence | **FAIL** | F-01 |
| V13.1.4 | Idempotency keys caller-scoped | **FAIL** | F-10 |
| V13.2.x | API tokens not in logs | PASS | tokens are not logged (verified via grep) |
| V14.1.2 | Debug code not reachable in prod | **PARTIAL** | F-17 |

---

## Positive Findings

The following defensive practices in this module are working as intended and should be preserved through any remediation:

1. **SQL is fully parameterised via `sqlx::query!` / `sqlx::query_as!` macros.** Every query in `repository.rs` (and `llm_repository/utils.rs` outside scope) is compile-time-checked. No string-formatted SQL anywhere in the module. (V5.3.5)
2. **Every API route requires a typed permission tuple via `RequirePermissions<(...)>`.** No anonymous endpoints exist on the router (`routes.rs:13-72`). Permission enforcement runs in the extractor, before the handler body. (V4.1.5)
3. **Permission keys are correctly namespaced** under `llm_models::*` and use the `PermissionCheck` trait properly. (`permissions.rs:1-62`)
4. **JWT validation in `RequirePermissions` checks the `is_active` flag on the loaded user** (`extractors.rs:104-110`) — inactivated users can't bypass the permission check.
5. **Cancellation tokens are scoped per-download** and removed after terminal states (`uploads.rs:1380, 1388, 1435`). Combined with `tokio::spawn` background tasks, this prevents orphaned downloads from running after their DB row terminates.
6. **`is_terminal()` / `can_cancel()` state-machine helpers** on `DownloadInstance` (`models.rs:879-892`) prevent illegal state transitions (e.g., cancelling a `Completed` download → 400 at `downloads.rs:212-218`).
7. **Cascade delete on `provider_id` / `repository_id` foreign keys** in both migration tables — orphan rows can't accumulate after provider/repo deletion.
8. **`#[serde(deny_unknown_fields)]` on `CreateLlmModelRequest`** (`types.rs:19`) blocks attacker-injected extra JSON fields.
9. **The disk path for a model is `<APP_DATA>/models/<provider_uuid>/<model_uuid>/`** with both UUIDs server-generated (`uploads.rs:154`, `repository.rs:370, 703`). The provider_id comes from the request but is type-validated as `Uuid` by the multipart parser, so it can't traverse out (`Uuid::Display` is always 36 hex+dashes).
10. **SHA-256 checksum verification on LFS downloads** (`lfs/service.rs:357-362`) — the downloaded byte stream is hashed and compared to the OID; mismatch returns `LfsError::ChecksumMismatch`. **However the OID comes from the pointer file in the repo**, which the attacker also controls — so this only validates "the file we downloaded matches what the pointer says", not "the file is what we expected". For the *trusted-repo* threat model (HF) this is sufficient; for the *attacker-can-create-repos* threat model (current with F-01) it is not.
11. **Built-in resource protection in `llm_repository`** prevents deletion of the HF default repository (out-of-scope but a useful guardrail referenced by this module).
12. **No `unwrap()` on `RepositoryAuthConfig` fields** when constructing the auth token — the `match` at `uploads.rs:1049-1063` cleanly handles missing fields.

---

## Out of Scope / Deferred

- **llm_provider:** the `provider.provider_type == "local"` check at `uploads.rs:146-151` is the only cross-boundary call from this module that touches sensitive provider state. The `LlmProvider.api_key` exposure issue (CRIT-1 in the 2025-11 audit) lives in that module; **re-verify in the dedicated llm_provider audit**.
- **llm_repository:** the SSRF-relevant `validate_url` is in `llm_repository/utils.rs` but the *consumer* is this module. F-01 must be fixed in **both** locations.
- **llm_local_runtime:** how downloaded model files are launched as subprocesses is reviewed in `08-llm-local-runtime.md` (TBD). Key concern: the model files we wrote in F-09 / F-18 are passed to mistralrs/llamacpp as arguments — argv injection via filename is the natural follow-up.
- **assistant / chat / hub:** modules that *consume* `LlmModel` rows (via `model_id` FK) inherit the F-04 ownership gap — flagged for the dedicated audits.
- **`hub` module's use of `initiate_repository_download_internal`:** the function comment at `uploads.rs:986-988` says "Used by both the public API endpoint and the hub module". Hub's call path needs a separate review to ensure permission propagation is correct (deferred).
- **Test infrastructure:** the existing `.sec-audits/08-test-security-audit.md` covers test data exposure; no new findings during this re-walk.

---

## Remediation priority

### Immediate (Sprint 1, blocks production)
1. **F-01 SSRF** — add host allow-list + private-IP block at `llm_repository/utils.rs::validate_url` and in libgit2's credential callback. Block `file://`, `git://`, `ssh://` schemes.
2. **F-02 LFS path traversal** — strict `[0-9a-fA-F]{40,64}` validation on `oid` at `metadata.rs:48`.
3. **F-03 Body limits** — per-route limit + streaming writes + disk-space pre-check.
4. **F-05 DB error leak** — generic message for clients, full detail to `tracing::error!` only.

### Short term (Sprint 2-3)
5. **F-04 Per-user ownership** — migration adds `created_by`; handlers filter by it.
6. **F-06 Token leak via redirect** — host check in credential callback.
7. **F-07 LFS HTTP timeouts** — `reqwest::Client::builder()` with timeout / redirect cap / `https_only`.
8. **F-08 Filename sanitiser** — strict allow-list, percent-decode, reject hidden / control / separator.
9. **F-09 Magic-byte validation** — block uploads when content doesn't match declared format.

### Medium term (Sprint 4-6)
10. **F-10 Idempotency key scoping** — filter `find_existing_in_progress` by `created_by`.
11. **F-11 SSE per-user filtering** — broadcast only events the recipient owns.
12. **F-12 JSON size bounds** — per-field length caps at deserialisation.
13. **F-13 Temp cleanup** — wire `clear_temp_directory` into startup, delete-after-success, scheduled GC.
14. **F-14 Static state cleanup** — `Lazy`/atomic, scope to module.
15. **F-15 Replace `println!`** with `tracing::debug!`.
16. **F-16 `unwrap()` removal** — graceful enum fallback or `AppError::internal_error`.
17. **F-17 Gate `clear_cache`** behind debug builds or `is_admin`.

### Long term (backlog)
18. **F-18 Extension allow-list** — minor defence-in-depth.
19. **F-19 Dedup window** — include `Failed`.
20. **F-20 Audit log** — module-wide structured event recording.

---

**Report end.**
