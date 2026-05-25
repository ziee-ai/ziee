# Security Audit — LLM Repository Module
**Date:** 2026-05-23
**Scope:** modules/llm_repository/ (~1,169 LOC) — repository metadata curation, git LFS downloads
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target

---

## Executive Summary

The `llm_repository` module manages an admin-curated catalogue of
external model registries (Hugging Face and GitHub seeded as
"built-in" rows; operators with `llm_repositories::create` can add
custom ones). Each row carries a base URL, an `auth_type`
(`none` / `api_key` / `basic_auth` / `bearer_token`), and an
`auth_config` JSON blob holding the matching secret. Five HTTP
routes mount under `/llm-repositories` (list, get, create, update,
delete, plus a connection-test endpoint). All routes are gated by
`RequirePermissions<…>`; the permission scopes are coarse
(`read|create|edit|delete`) and there is **no per-user ownership** —
all rows are global, shared across tenants.

The module itself contains no git or LFS code — it owns the table
that supplies URLs and credentials to the downstream consumers in
`llm_model::handlers::uploads` (`GitService::build_repository_url`,
`clone_repository`, `pull_lfs_files_with_cancellation`) and
`modules::hub` (`find_by_url`). The git2 cloner
(`utils/git/service.rs`) and LFS batch-API client
(`utils/git/lfs/service.rs`) are technically "utils" but the only
producer of URLs/tokens that reaches them is this module, so the
SSRF / network / clone-safety findings against those utils are
reported here per the audit scope's "flag if you find vulns"
clause.

The previous audit (`.sec-audits/04-llm-modules-audit.md`,
2025-11-21) flagged credentials-in-responses and SSRF. **Both
remain unfixed.** This audit additionally identifies: missing
URL-scheme allowlist (validator accepts `file://`, `ssh://`,
`gopher://`, `data:`), unvalidated `auth_test_api_endpoint` SSRF
amplifier, embedded URL userinfo logged via `println!`, no
clone-depth or LFS-size cap (disk-fill DoS), no OID hex-validation
in LFS metadata (directory-traversal in cache path), git2
credential callback that hands tokens to redirect targets, and an
editable URL field on "built-in" rows.

**Risk: HIGH (Critical: 1, High: 6, Medium: 5, Low: 4, Info: 4)**

### Top 3 risks

1. **F-01 (Critical):** SSRF in `test_repository_connection` —
   `validate_url` accepts any URL `reqwest::Url::parse` accepts (no
   scheme allowlist, no host/IP block), so any authenticated user with
   `llm_repositories::read` can probe arbitrary internal addresses
   including the AWS IMDS at `http://169.254.169.254/`, intranet
   admin panels, and (because the `auth_type` is user-supplied) can
   send a `Bearer <attacker-controlled>` token to those internal
   hosts. The "Hugging Face contains `huggingface.co`" string check
   on the **user-supplied** URL means a payload of
   `http://169.254.169.254.huggingface.co.attacker.com/x` (DNS
   rebinding bait) flips the codepath that sends a Bearer header to
   an attacker-chosen origin. **Same SSRF exists in
   `clone_repository`** — `git2` reaches the URL with no host/IP
   filtering and additionally passes the saved auth credential to
   whatever host the URL points at.

2. **F-02 (High):** Plaintext credentials returned in every list/get
   response. `RepositoryAuthConfig`'s `api_key`, `password`, `token`
   fields use `skip_serializing_if = Option::is_none` only — when
   present they are serialised verbatim. The route requires only
   `llm_repositories::read`, so any user granted that permission sees
   every other tenant's tokens (the built-in Hugging Face row is
   shipped with `api_key: ""`, but operators are *expected* to fill
   it in — once they do, the token is in every API response). The
   2025-11 audit flagged this; it is still present. No "secret
   redaction" layer wraps the response. Combined with F-01 this is
   a credential-theft pipeline: any user reads the Hugging Face
   token, then uses F-01 to send it to an attacker-controlled host.

3. **F-03 (High):** `validate_url` is `reqwest::Url::parse(url).is_ok()`
   — that is, the validation is **"is it any URL at all"**. There is
   no scheme allowlist, no host validation, no loopback/private-IP
   block. Schemes `file:`, `ssh:`, `gopher:`, `data:`, and the
   git-protocol `git:` all pass. `file:///root/.ssh/id_rsa` round-trips
   through the database into `GitService::clone_repository`, which
   passes it to `git2::Repository::clone` — git2's smart-protocol
   layer happily handles `file://` (local-clone), `git://`
   (unauthenticated TCP), and `ssh://` (asks the system keyring for
   an SSH key). The `file://` scheme alone lets a repository's "URL"
   point at arbitrary local directories, which the cloner then mirrors
   into the cache dir under the server uid's read access.

---

## Findings

### F-01 — SSRF in `test_repository_connection` and `clone_repository` — internal network exposure
- **Severity:** **Critical**
- **ASVS:** V12.6.1 (SSRF protection on outbound URL fetches), V5.2.6
  (URL validation), V9.1.1 (TLS for sensitive data)
- **CWE:** CWE-918 (Server-Side Request Forgery), CWE-441 (Unintended
  Proxy)
- **Location:**
  `src-app/server/src/modules/llm_repository/utils.rs:14-23`
  (validate_url),
  `src-app/server/src/modules/llm_repository/utils.rs:179-263`
  (test_repository_connectivity),
  `src-app/server/src/utils/git/service.rs:96-545`
  (clone_repository),
  `src-app/server/src/utils/git/lfs/service.rs:200-363`
  (download_file)
- **Description:**
  `validate_url` is a one-line wrapper around `reqwest::Url::parse`:
  ```rust
  pub fn validate_url(url: &str) -> Result<(), AppError> {
      if reqwest::Url::parse(url).is_ok() { Ok(()) } else { ... }
  }
  ```
  `reqwest::Url::parse` is a *syntactic* check (the `url` crate's
  RFC-3986 parser); it accepts any valid URL of any scheme. There is
  no scheme allowlist (it accepts `file:`, `ssh:`, `git:`, `gopher:`,
  `data:`, `ws:`, `chrome-extension:`, etc.), no host validation, no
  IP-literal check, no DNS-resolution sanity, no
  loopback/private-IP/link-local/IMDS/RFC1918 block.

  The same `validate_url` gates both `POST /llm-repositories` and
  `POST /llm-repositories/test`. For the latter, the URL is
  immediately fed to `reqwest::Client::get(test_url).send()`. For
  the former, the URL is stored in the database and later used by
  `GitService::clone_repository` (which calls
  `git2::Repository::clone`) and by `LfsService::download_file`
  (which POSTs to `<url>/info/lfs/objects/batch`). Three different
  reach-out paths, none of which filter the destination.

  **Where the auth_test_api_endpoint amplifies the bug:** in
  `test_repository_connectivity` the actual URL fetched is taken
  preferentially from `auth_config.auth_test_api_endpoint` if
  non-empty:
  ```rust
  let test_url = if let Some(auth_config) = &request.auth_config {
      if let Some(ref test_endpoint) = auth_config.auth_test_api_endpoint {
          if !test_endpoint.trim().is_empty() { test_endpoint }
          else { &request.url }
      } else { &request.url }
  } else { &request.url };
  ```
  **`auth_test_api_endpoint` is never validated against `validate_url`**
  — only `request.url` is validated in the handler. An attacker can
  set `url: "https://huggingface.co"` (a clean-looking URL that passes
  every cursory review) and `auth_config.auth_test_api_endpoint:
  "http://169.254.169.254/latest/meta-data/iam/security-credentials/<role>"`,
  and the connection-test handler will fetch the IMDS instead. Bonus:
  **the codepath that decides whether to send a `Bearer` token vs an
  `X-API-Key` header is gated on `request.url.contains("huggingface.co")`**
  — so the attacker controls which header is sent to the IMDS by
  choosing the cosmetic `url` field. The Bearer header is the more
  dangerous one (some legacy IMDS endpoints reflect Authorization).

  **DNS-rebinding amplification:** even if a host allowlist were
  added, no DNS pinning is done — the validator parses the URL once;
  `reqwest` (and `git2`'s libcurl backend) resolves the host *again*
  at request-issue time. A first-resolve-public, second-resolve-private
  rebinding attack is open.

  **`git2` follows whatever scheme the URL specifies.** `git2` ships
  with the smart and dumb HTTP transports, the local-`file://`
  transport, and the SSH transport (libssh2). `clone_repository`
  passes `&repository_url` straight to `RepoBuilder::clone(&url, &dest)`
  with no scheme inspection. Recall the cloner is called from
  `llm_model::handlers::uploads::initiate_repository_download_internal`
  with `auth_token = repository.auth_config.api_key/token/...` — so
  not only is the destination attacker-controlled, **the stored
  credential is shipped to it** (`Cred::userpass_plaintext(username,
  token)`). An attacker who can create a repository row with URL
  `https://attacker.example/exfil.git` and a stale legitimate
  Hugging Face token in `auth_config` causes the next download to
  hand the HF token to attacker.example over a normal HTTPS POST.
- **Vulnerable code:**
  ```rust
  // utils.rs:14-23 — the entire URL validator
  pub fn validate_url(url: &str) -> Result<(), AppError> {
      if reqwest::Url::parse(url).is_ok() { Ok(()) }
      else { Err(AppError::bad_request("VALIDATION_ERROR", "Invalid URL format")) }
  }
  ```
  ```rust
  // utils.rs:190-205 — test_url chosen from auth_test_api_endpoint
  // without re-validation
  let test_url = if let Some(auth_config) = &request.auth_config {
      if let Some(ref test_endpoint) = auth_config.auth_test_api_endpoint {
          if !test_endpoint.trim().is_empty() { test_endpoint } ...
  };
  let mut req_builder = client.get(test_url);
  ```
  ```rust
  // utils.rs:213-220 — Hugging Face detection on user-controlled url
  if request.url.contains("huggingface.co") {
      req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
  } else {
      req_builder = req_builder.header("X-API-Key", api_key);
  }
  ```
- **Exploitation (test_repository_connection):**
  1. Authenticated user with `llm_repositories::read` POSTs to
     `/llm-repositories/test`:
     ```json
     {
       "name": "HF",
       "url": "https://huggingface.co",
       "auth_type": "api_key",
       "auth_config": {
         "api_key": "any value",
         "auth_test_api_endpoint":
           "http://169.254.169.254/latest/meta-data/iam/security-credentials/"
       }
     }
     ```
  2. Handler validates only `request.url` (= huggingface.co — OK),
     then fetches `test_url` (= 169.254.169.254) with
     `Authorization: Bearer any value`. **The response status code
     leaks back to the client** (the handler returns `success:
     true` only on HTTP 200, otherwise `success: false` with the
     status string — both responses confirm the host is reachable
     and discriminate by status code; a connect-refused vs
     unauthorised-vs-200 lattice fingerprints internal services).
- **Exploitation (clone_repository / SSRF + cred theft):**
  1. Attacker has `llm_repositories::create` permission.
  2. Attacker creates a repository row with `url:
     "https://attacker.example/repo.git"` and `auth_config.token:
     "<stolen GitHub PAT>"` (in the realistic case, the attacker
     edits an existing repository row to swap the URL while keeping
     the token — `llm_repositories::edit` permission). On the next
     `initiate_repository_download_internal` (any user with
     `llm_models::create`), the cloner ships the GitHub PAT to
     attacker.example via HTTPS basic auth.
- **Impact:** **Internal network reconnaissance and SSRF-driven
  data exfiltration.** The combination of (i) arbitrary outbound
  fetch with attacker-controlled scheme and host, (ii) stored
  credentials transmitted alongside the request, and (iii) admin-level
  cloning context that runs as the server uid with read access to the
  cache directory and the system keyring (libssh2 for `ssh://`),
  makes this Critical. On EC2/GCE/Azure, IMDS access on its own is
  game-over (steal IAM/instance-account credentials). On a normal
  intranet host, attacker can scan and call any internal HTTP service
  bound on the same network.
- **Recommendation:**
  1. **Strict allowlist** in `validate_url`: `https` scheme only,
     non-empty host, reject IP-literal hosts. Apply to **both**
     `request.url` and `auth_config.auth_test_api_endpoint`.
  2. **Resolver pinning:** bind the reqwest client (and the cloner,
     where possible) to a custom resolver that filters
     loopback / 169.254.169.254 / fc00::/7 / RFC1918 — defeats DNS
     rebinding.
  3. **Re-validate on read** in `GitService::build_repository_url`
     so legacy DB rows written before the tightening still get
     rejected at use time.
  4. **Do not echo upstream status code** in the connection-test
     response body — return a binary success/fail only.
  5. Stop deriving Hugging Face header behaviour from
     `request.url.contains("huggingface.co")`; store a
     `provider_kind` enum (`huggingface`/`github`/`generic`) at row
     level so the header decision is not under attacker control.

---

### F-02 — Plaintext credentials returned in every list/get/create response — cross-tenant disclosure
- **Severity:** **High**
- **ASVS:** V8.3.4 (sensitive data not returned to the client unnecessarily),
  V6.1.1 (Cryptographic storage of secrets), V14.5.3 (no secret echo)
- **CWE:** CWE-200 (Information Exposure), CWE-312 (Cleartext Storage),
  CWE-359 (Exposure of Private Personal Information)
- **Location:**
  `src-app/server/src/modules/llm_repository/models.rs:14-40`
  (RepositoryAuthConfig),
  `src-app/server/src/modules/llm_repository/handlers.rs:37-95, 110-135`
  (list / get / create return `LlmRepository` whole)
- **Description:**
  `RepositoryAuthConfig` holds `api_key`, `username`, `password`,
  `token`, and `auth_test_api_endpoint` (which is itself an
  attacker-controllable URL — see F-01 and F-08). All four secret
  fields are `Option<String>` with `#[serde(skip_serializing_if =
  "Option::is_none")]` — present-with-value is serialised verbatim:
  ```rust
  pub struct RepositoryAuthConfig {
      #[serde(skip_serializing_if = "Option::is_none")]
      pub api_key: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub username: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub password: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub token: Option<String>,
      ...
  }
  ```
  All routes that return a `LlmRepository` or
  `LlmRepositoryListResponse` therefore include the credential
  plaintexts:
  - `GET /llm-repositories` — list (paginated)
  - `GET /llm-repositories/{id}` — single
  - `POST /llm-repositories` — create echoes the just-stored row
  - `POST /llm-repositories/{id}` — update returns the new row

  The only authorisation gate is `llm_repositories::read`. The
  module has no `user_id`/`owner_id` column — repositories are
  **global resources**. So every user with the read permission sees
  every other tenant's Hugging Face / GitHub / private-mirror
  credentials. In practice this often includes service accounts and
  CI-bot tokens that map to higher privileges than the reader has on
  the target repository.
- **Vulnerable code:** see snippet above; the same model is reused
  for the response type (no separate `LlmRepositoryResponse`).
- **Exploitation:**
  1. Operator seeds the Hugging Face row with their org's
     `hf_xxx...` token (so that the cloner can pull gated models).
  2. Any user granted `llm_repositories::read` issues `GET
     /llm-repositories` and reads the token from the JSON body.
  3. Attacker uses the token to clone the org's gated models, list
     private repositories, etc., outside the application.
- **Impact:** Cross-tenant credential disclosure. Severity depends
  on the tokens stored, but operators routinely store powerful tokens
  (HF org admin, GitHub PAT with `repo`-scope) here.
- **Recommendation:**
  1. **Separate response and storage types.** Define
     `RepositoryAuthConfigResponse` with the secret fields elided
     (e.g., `api_key_present: bool` instead of `api_key: String`).
     Return that type from list/get; only the create/update flows
     accept the storage type, and even then the response should not
     echo what was just stored.
  2. **Encrypt at rest.** The migration stores `auth_config` as a
     JSONB column in plaintext. Wrap the secrets with the existing
     application-level encryption layer (the `core` module ships one
     — see `Secret` / encrypted-field machinery used by `llm_provider`
     and `mcp` for OAuth secrets) so that a DB dump does not leak
     them.
  3. **Tighten the read permission.** Two scopes: `read_metadata`
     (anyone) returning the redacted view, and `read_credentials`
     (operator-only) returning the unredacted view for the rare
     edit-flow. The current handler uses one scope for both.

---

### F-03 — `validate_url` allows non-HTTPS schemes (`file:`, `ssh:`, `git:`, `data:`)
- **Severity:** **High**
- **ASVS:** V5.1.5 (URL scheme allowlist), V12.6.1 (SSRF)
- **CWE:** CWE-20 (Improper Input Validation), CWE-940 (Improper
  Verification of Source of a Communication Channel)
- **Location:** `src-app/server/src/modules/llm_repository/utils.rs:14-23`,
  consumed downstream by `utils/git/service.rs:471`
  (`builder.clone(&repository_url, &repo_cache_dir_clone)`).
- **Description:** `reqwest::Url::parse` is the `url` crate's
  RFC-3986 parser. Schemes `file:///root/.ssh/id_rsa`,
  `ssh://git@attacker/x.git`, `git://attacker/x.git`,
  `gopher://internal:6379`, `data:text/plain;base64,...` all parse
  successfully. There is no allowlist enforcement.

  Once the row is persisted with a non-HTTPS URL, **git2's
  `RepoBuilder::clone` accepts whatever transport scheme the URL
  specifies**:
  - `file://` — git2 spawns the local-clone smart transport,
    reading whatever directory the URL points at. The server uid's
    filesystem is the limit — `/etc/`, `/var/lib/ziee/files/<other
    user>/`, the application's own source.
  - `ssh://` — git2 (with libssh2 enabled) attempts SSH; if the
    server has an agent socket or `~/.ssh/id_rsa`, those credentials
    are used.
  - `git://` — plaintext git protocol over TCP, no auth, no TLS,
    no integrity. Attacker can MITM (it's not over TLS) and serve a
    crafted pack.

  Even the LFS POST in `lfs/service.rs:236` (`client.post(request_url).json(...)`)
  does not check the scheme of the derived URL — `repo_remote_url`
  is read out of `.git/config` of the locally-cloned repo, which the
  attacker controls if they controlled the original URL.
- **Vulnerable code:**
  ```rust
  pub fn validate_url(url: &str) -> Result<(), AppError> {
      if reqwest::Url::parse(url).is_ok() { Ok(()) } else { ... }
  }
  ```
- **Exploitation:**
  1. Attacker with `llm_repositories::create` POSTs a row with
     `url: "file:///root/.ssh/"`. Validator accepts.
  2. Any user with `llm_models::create` issues a download against
     this repository: `GitService::clone_repository` calls
     `git2::Repository::clone("file:///root/.ssh/", <cache>)`. git2
     will refuse this specifically (must be a bare/non-bare repo,
     not an arbitrary directory) — but the same trick with
     `file:///tmp/<attacker-staged-git-repo>` succeeds, because
     attacker can place a real bare repo in `/tmp/` first (any user
     can write to `/tmp/`).
  3. Alternative: `url: "ssh://git@attacker.example/x.git"`. git2
     uses the server's SSH credentials to authenticate; attacker
     receives an SSH connection signed with the server's host key
     or user key. If `~/.ssh/id_rsa` is on the server, that key is
     used; the corresponding public key fingerprint leaks to the
     attacker, and (if attacker controls the network path) the
     private key fingerprint is usable for replay against any other
     host that trusts it.
- **Impact:** Arbitrary local-file read via `file://`, credential
  leakage via `ssh://`, MITM-able cleartext via `git://`. Combined
  with F-01 (no DNS/IP filtering) this is a multi-scheme SSRF.
- **Recommendation:**
  Whitelist `https` only (and possibly `http` for self-hosted
  intranet GitLab if the deployment requires it — in which case
  combine with F-01's IP/host allowlist):
  ```rust
  if !matches!(parsed.scheme(), "https") {
      return Err(AppError::bad_request("INVALID_URL", "Only https:// allowed"));
  }
  ```
  Re-validate on read in `GitService::build_repository_url` so
  legacy rows are rejected at use time even if they were written
  pre-fix.

---

### F-04 — Embedded URL credentials are not stripped before logging
- **Severity:** **High**
- **ASVS:** V7.1.1 (No secrets in logs), V8.3.5 (Sensitive data
  cleansing)
- **CWE:** CWE-532 (Insertion of Sensitive Information into Log File),
  CWE-200
- **Location:** `llm_repository/utils.rs:207`
  (`println!("Testing connection to: {}", test_url);`),
  `llm_model/handlers/uploads.rs:1094-1098`
  (`tracing::info!("Starting download for repository: {}", repository_url, ...)`),
  `utils/git/lfs/service.rs:190-198` (`url.set_password(access_token)`)
- **Description:** `https://user:topsecret@github.com/x.git` parses
  successfully through `reqwest::Url::parse` (RFC-3986 userinfo is
  conformant). The validator does not strip or warn on a populated
  userinfo component, and downstream consumers do not strip it either:
  `utils.rs:207` prints the entire URL via `println!` to stdout
  (container log driver); `uploads.rs:1094` renders it through
  `tracing::info!`. Worse, `lfs/service.rs:190-198`'s `url_with_auth`
  **actively constructs** `https://oauth2:<token>@host/path` URLs
  internally — those flow through `tracing::debug!/error!` in the
  LFS download path.
- **Exploitation:** Attacker submits
  `https://attacker:correct-horse@github.com/x.git`; the cred lands
  in `println!` stdout, container logs, and `tracing::info!` structured
  logs. Even without attacker-embedded userinfo, the LFS service's
  internal `oauth2:<token>` URLs leak through debug-level logs.
- **Impact:** Credential leak via logs. Severity comparable to F-02
  but via a different sink (logs vs API responses).
- **Recommendation:**
  1. In `validate_url` reject URLs whose userinfo is populated
     (`parsed.username() != "" || parsed.password().is_some()`).
  2. Replace `println!` with `tracing::debug!` and route every
     URL-bearing log line through a `redact_url(&Url)` helper that
     calls `.set_username("")` / `.set_password(None)` before
     formatting.
  3. Audit every URL-interpolating log site in
     `uploads.rs:1094-1184` and `lfs/service.rs:240-296`.

---

### F-05 — No clone-depth, repository-size, or LFS-size cap — disk-fill DoS
- **Severity:** **High**
- **ASVS:** V12.1.1 (Resource exhaustion limits on uploads/downloads),
  V12.4.1 (File-size limits)
- **CWE:** CWE-400 (Uncontrolled Resource Consumption), CWE-770
  (Allocation of Resources Without Limits)
- **Location:**
  `src-app/server/src/utils/git/service.rs:148-545`
  (clone_repository — no depth/size limits),
  `src-app/server/src/utils/git/lfs/service.rs:319-346` (chunk loop
  with no size cap)
- **Description:** `RepoBuilder::clone(&url, &dest)` is invoked with
  default fetch options:
  ```rust
  let mut builder = RepoBuilder::new();
  builder.fetch_options(fetch_options);
  if let Some(branch_name) = branch.as_deref() {
      builder.branch(branch_name);
  }
  // ...
  match builder.clone(&repository_url, &repo_cache_dir_clone) { ... }
  ```
  There is **no `--depth` shallow clone**, no maximum object count,
  no maximum bytes-received limit, and no diskspace pre-check.
  `RemoteCallbacks::transfer_progress` is set but its callback only
  records progress and checks cancellation — it does not enforce a
  byte ceiling. A repository with multi-GB pack files (or worse, a
  malicious server replying with an infinite stream — see
  CVE-2020-1971-style "git bomb" — a small repo expands to TB of
  delta'd objects on the client) will fill the cache disk and crash
  the server. The LFS path is even more exposed: the batch API
  returns one Object containing the file size, but the
  `download_file` chunk loop (lines 319-346) writes every received
  chunk into a NamedTempFile without comparing against
  `metadata.size` at all (the SHA-256 verification at line 358
  catches *integrity* failure post-facto but the disk is already
  full).

  The cache directory is `~/.cache/ziee-chat/models/git/` (per-user
  `dirs::cache_dir`); a single download that overflows that
  filesystem brings down the server.

  This DoS is reachable by any user with `llm_models::create` who
  can identify the (always-readable) Hugging Face repository — and
  Hugging Face does host multi-hundred-GB models. A user requesting a
  pathological model path can exhaust disk without being authenticated
  as anything more than a regular member.
- **Vulnerable code:** see locations above. Note absence of
  `set_depth(1)` (`git2` exposes `FetchOptions::depth(i32)` as of
  git2 0.18 — the project uses git2 — so this is a one-line fix at
  the API layer).
- **Exploitation:**
  1. User has `llm_models::create`. (Default? Check
     `permissions/seed.rs`. If yes, this is unauthenticated-tier.)
  2. User asks the hub or `initiate_repository_download_internal`
     to clone a 300 GB model from Hugging Face. Cache disk fills.
  3. Server crashes when the OS denies new writes; downloads stuck;
     postgres fails (if the data and cache volumes are the same);
     the bwrap rootfs cache (separate but possibly same volume)
     unmounts.
- **Impact:** Denial of service. Repeat to keep service down.
- **Recommendation:**
  1. **Mandatory shallow clone** (`fetch_options.depth(1)`).
  2. **Bytes-received cap** in `transfer_progress`: track cumulative
     `received_bytes()`, return `false` past a configurable ceiling
     (e.g. 50 GB).
  3. **LFS pre-flight + running-counter cap:** sum `metadata.size`
     across all LFS pointers identified by
     `pull_lfs_files_with_cancellation` and reject if total exceeds
     cap; compare cumulative bytes inside the chunk loop and abort on
     overshoot (the current SHA check post-facto is too late).
  4. **Disk-free pre-check:** `statvfs` on `cache_dir`, refuse to
     start if free space < (estimated size × safety factor).

---

### F-06 — Git submodules not disabled; LFS pointer `oid` not sanitised into filesystem path
- **Severity:** **High**
- **ASVS:** V12.3.4 (Path traversal in file storage), V10.3.2
  (Subprocess hardening — analogous: third-party-controlled metadata)
- **CWE:** CWE-22 (Path Traversal), CWE-829 (Inclusion of
  Functionality from Untrusted Control Sphere)
- **Location:**
  `src-app/server/src/utils/git/service.rs:451-471`
  (RepoBuilder::clone — no submodule control),
  `src-app/server/src/utils/git/lfs/service.rs:174-188`
  (get_cache_dir — uses `metadata.oid[0..2]` and `[2..4]` directly)
- **Description:**

  **Submodule recursion:** `RepoBuilder::clone()` in libgit2 does
  *not* fetch submodules by default — so the immediate
  `git submodule update --init --recursive` is not opened — **but**:
  - There is no later step that explicitly *blocks* submodule
    initialization elsewhere in the codebase. Tools or scripts in
    `llm_model::download` that later read files from the cloned
    repo (e.g., `std::fs::read_dir(&cache_path)` at uploads.rs:1207)
    will dereference symlinks and walk into submodule placeholders
    if anything *else* (a future cron, a sync script, an operator
    `git submodule update`) recurses them.
  - More importantly, a malicious repository can include
    `.gitmodules` pointing at `file:///etc/passwd` or `ssh://...`
    URLs. The cloner ignores these *for now*; if the cloner ever
    grows submodule support (the docs hint at it via "LFS files not
    included in initial clone"), the existing F-03 unchecked-scheme
    bug will resurface for submodule URLs.

  **LFS oid path injection:** `LfsService::get_cache_dir` constructs:
  ```rust
  let oid_1 = &metadata.oid[0..2];
  let oid_2 = &metadata.oid[2..4];
  Ok(Self::get_real_repo_root(repo_root).await?
      .join(".git").join("lfs").join("objects")
      .join(oid_1).join(oid_2))
  ```
  `metadata.oid` is parsed from the LFS *pointer file*
  (`LfsMetadata::parse_from_string` at `metadata.rs:31-66`). That
  pointer file is **inside the cloned repo** — content the remote
  controls. Today the parsed `oid` is the value after the
  `sha256:` prefix; the code accepts any non-empty value at that
  position. The slices `[0..2]` and `[2..4]` will **panic** on an
  OID shorter than 4 chars (no length check) — that's the
  availability angle. The **path-traversal** angle: if a remote
  serves a pointer with `oid` = `../../../../etc/passwd`, the
  first slice `[0..2]` = `..`, the second `[2..4]` = `/.`, and
  `Path::join` will happily resolve up two directories and write
  the LFS cache files there. The remote then sends the actual
  bytes (which the SHA mismatch rejects) — but the directory has
  already been **created** by `fs::create_dir_all(&cache_dir)`
  before the download:
  ```rust
  // lfs/service.rs:383
  fs::create_dir_all(&cache_dir).await ...
  ```
  Arbitrary directory creation under the server uid is the worst
  case; arbitrary file write requires the SHA to match (the temp
  file is renamed into `cache_file` only on integrity check pass),
  but the directory traversal alone can be used to plant a
  `.well-known/` path or interfere with other application state.
- **Vulnerable code:**
  ```rust
  // utils/git/lfs/metadata.rs:48-58
  let mut oid = *lines.get(OID_PREFIX).ok_or("Could not find oid-entry")?;
  let mut hash = None;
  if oid.contains(':') {
      let lines: Vec<_> = oid.split(':').collect();
      if lines.first()... == &"sha256" { hash = Some(Hash::SHA256); } ...
      oid = *lines.last()...;
  }
  // no length / character validation
  Ok(LfsMetadata { size, oid: oid.to_string(), hash })
  ```
- **Exploitation:** Attacker hosts a repo with an LFS pointer whose
  `oid sha256:` value starts with traversal-shaped chars (e.g.,
  `..` / `.x`). After clone, `get_cache_dir` joins `oid[0..2] = ".."`
  and `oid[2..4]` — the `..` escapes the LFS objects directory and
  `fs::create_dir_all` follows.
- **Impact:** Arbitrary directory creation as the server uid;
  index-time panic on a too-short OID (DoS via slice index OOB).
- **Recommendation:**
  1. **Validate the OID in `LfsMetadata::parse_from_string`:**
     ```rust
     if oid.len() != 64 || !oid.chars().all(|c| c.is_ascii_hexdigit()) {
         return Err(LfsError::InvalidFormat("OID must be 64 hex chars"));
     }
     ```
     SHA-256 is always 64 hex digits — anything else is malformed.
  2. **Use `Path::components()` to defensively reject any `..`
     component when constructing the cache path.**
  3. **Explicitly disable submodules** by checking the cloned
     working tree for a `.gitmodules` file and either refusing or
     warning (and never recursing — the current code does not
     recurse, but lock that in with a unit test).

---

### F-07 — `git_phase`-only cancellation; no per-process cgroup or filesystem isolation around git2/curl
- **Severity:** **Medium**
- **ASVS:** V10.3.2 (Subprocess argv / sandboxing — analogous: third
  party library isolation)
- **CWE:** CWE-913 (Improper Control of Dynamically-Managed Code
  Resources)
- **Location:** `src-app/server/src/utils/git/service.rs:148-545`
- **Description:** `clone_repository` runs git2 inside
  `tokio::task::spawn_blocking`. git2 in turn drives libgit2, which
  links libcurl/libssh2 — all C dependencies. There is no resource
  isolation around the blocking task: a libcurl bug, a libgit2 OOM,
  or a libssh2 buffer mishandling becomes a server-wide crash. (The
  code_sandbox module exists precisely to isolate untrusted commands
  — that isolation is *not* applied here.) The "cancellation
  monitor" (lines 132-143) polls every 100 ms and sets an atomic
  flag — fine for cooperative cancellation but does not interrupt
  a blocking syscall stuck inside libgit2; the spawn_blocking
  thread leaks until the OS aborts it.

  This is less severe than F-01—F-06 because libgit2 itself is
  well-fuzzed and exploiting it requires a server-side bug, but
  the lack of *any* outer boundary means the security posture is
  "as good as libgit2 is on this day" and there is no defence in
  depth.
- **Recommendation:**
  - Consider running the git clone in a separate OS process (a
    subprocess that the server can `kill -9`) bounded by `prlimit`
    (RLIMIT_AS, RLIMIT_FSIZE, RLIMIT_NOFILE, RLIMIT_CPU). The
    `code_sandbox` infrastructure could be reused — it already
    has the bwrap path wired in.
  - Set a wall-clock timeout on `clone_repository` (current code
    has none; the only timer is the 10 s connection-test in
    `utils.rs:183`).

---

### F-08 — `auth_test_api_endpoint` is unvalidated free-form URL stored in DB
- **Severity:** **Medium**
- **ASVS:** V5.1.5 (URL allowlist), V8.3.4
- **CWE:** CWE-20 (Improper Input Validation)
- **Location:** `src-app/server/src/modules/llm_repository/models.rs:27-28`,
  consumed by `utils.rs:190-203`
- **Description:** `auth_config.auth_test_api_endpoint: Option<String>`
  is stored as part of the JSONB blob without ever passing
  through `validate_url`. Even after F-01 is fixed by adding a URL
  allowlist to `request.url`, the secondary field is still
  unconstrained because the create/update handlers do not iterate
  into the auth-config to validate URL-shaped fields. The same SSRF
  problem applies through this side door at every connection test.
- **Recommendation:** Add an explicit `validate_url` call on
  `auth_test_api_endpoint` in both `validate_auth_config_for_create`
  and `validate_auth_config_for_update`. Reject if it does not pass
  the strict allowlist.

---

### F-09 — Cloning destination path is server-uid-shared, not per-user — race condition + cross-tenant tampering
- **Severity:** **Medium**
- **ASVS:** V12.3.4 (Path traversal / shared storage isolation),
  V4.3.1 (Access control to resources)
- **CWE:** CWE-362 (Race Condition), CWE-732 (Incorrect Permission
  Assignment), CWE-552 (Files Accessible to External Parties)
- **Location:**
  `src-app/server/src/utils/git/service.rs:61-72, 96-120`,
  `src-app/server/src/modules/llm_model/handlers/uploads.rs:992-1040`
- **Description:** The cache key is
  `DefaultHasher` (SipHash-1-3, process-local random seed) over
  `(repository_id, repository_url, branch)`. Two issues:
  - **Restart-instability:** the seed changes per process, so cache
    hits don't survive restarts. Stale directories accumulate
    (operational, not security).
  - **No user_id component:** the cache dir is shared across all
    users. Two concurrent downloads for the same repo race at
    `is_existing_repo = exists() && .git.exists()` (line 117) — user
    B may enter the "open existing repository + pull" branch while
    user A is still mid-clone, ending in `git2::Repository::open`
    failure or a `reset --hard` against a half-clone. There is no
    `flock` on `repo_cache_dir`.
- **Recommendation:**
  1. Use a deterministic hash (SHA-256 of `(repo_id, url, branch)`)
     so cache hits survive restarts.
  2. Acquire a per-`cache_key` file lock
     (`fs2::FileExt::try_lock_exclusive`) before entering the
     clone/open branch.
  3. If repository ownership is ever introduced, include `owner_id`
     in the cache path so concurrent downloads from different users
     do not collide.

---

### F-10 — Repository `name` not size-limited; XSS surface if rendered as HTML
- **Severity:** **Medium**
- **ASVS:** V5.1.4 (Input length), V5.3.3 (Context-aware output
  encoding)
- **CWE:** CWE-79 (XSS), CWE-1284 (Improper Validation of Specified
  Quantity in Input)
- **Location:** `src-app/server/src/modules/llm_repository/types.rs:13-37`,
  `src-app/server/src/modules/llm_repository/utils.rs:38-91`
- **Description:** `CreateLlmRepositoryRequest::name` is `String`
  with no max-length and no content sanitisation. The migration
  caps the column at `VARCHAR(255)` (`migrations/00000000000002`,
  line 6) — Postgres will reject overlength inserts with a 22001
  string_data_right_truncation, surfacing as a 500 from
  `AppError::database_error`, but that is a soft DoS rather than
  a hard limit (the API will accept a 1 MB JSON name field, buffer
  it, and bounce on insert).

  More importantly the `name` is rendered into the UI list (see the
  hub flow at `hub/handlers.rs:427-441` and the standard list
  response). If the UI does not HTML-escape it, an attacker who
  creates a repo named `<img src=x onerror=alert(1)>` triggers
  stored XSS against every user with `llm_repositories::read`.
- **Recommendation:**
  1. Enforce `max_length = 100` (or smaller) in the request type
     via `validator::Validate` or a manual check in
     `validate_auth_config_for_create`.
  2. Strip control characters and HTML metacharacters
     (`<>'"&`) — or simply rely on the UI to HTML-encode, but the
     contract should be documented.
  3. Apply the same to `auth_config.username` (rendered in the
     credentials editor).

---

### F-11 — `pagination.page` and `pagination.per_page` are unbounded `i32` — large-list memory pressure
- **Severity:** **Medium**
- **ASVS:** V12.1.1 (Resource limits), V5.1.3 (Numeric range
  validation)
- **CWE:** CWE-770 (Resource Allocation Without Limits)
- **Location:**
  `src-app/server/src/modules/llm_repository/handlers.rs:37-67`,
  `src-app/server/src/common/type.rs:178-201`
- **Description:** `PaginationQuery` (common type) declares
  `page: i32` and `per_page: i32` with defaults of 1 and 20. No
  upper bound: a client can submit `per_page=2147483647`. The
  handler also *reads everything in memory first*:
  ```rust
  let all_repositories = Repos.llm_repository.list().await?;
  // ...
  let total = all_repositories.len() as i64;
  let start = ((params.page - 1) * params.per_page) as usize;
  let end = (start + params.per_page as usize).min(all_repositories.len());
  ```
  — meaning pagination doesn't reduce DB load; it just slices an
  in-memory `Vec<LlmRepository>`. For this module (a curated table,
  typically <20 rows) the impact is small. But the same pattern
  appears in the hub and llm_model modules where the row count is
  much larger; the precedent of "fetch all, slice in app" is a
  latent risk.

  Also: `start = ((page - 1) * per_page) as usize` with `page = 0`
  computes `(0_i32 - 1) * 20 = -20`, which casts to a huge usize.
  `start < all_repositories.len()` is then false, so the slice
  returns empty — no crash, but the math is unprincipled. Negative
  `page` values are not rejected.
- **Recommendation:**
  1. `#[validate(range(min = 1, max = 10_000))]` on `page` and
     `range(min = 1, max = 1000)` on `per_page`.
  2. Push pagination into SQL (`LIMIT $per_page OFFSET $offset`)
     instead of in-memory slicing.

---

### F-12 — `RepoBuilder::clone` is called with default authentication callbacks that send credentials to *any* host that asks for them
- **Severity:** **Medium**
- **ASVS:** V9.1.3 (Authenticated channels constrained to intended
  hosts)
- **CWE:** CWE-441 (Unintended Proxy or Intermediary)
- **Location:** `src-app/server/src/utils/git/service.rs:368-376,
  195-201`
- **Description:** The credential callback installed in libgit2 is:
  ```rust
  callbacks.credentials(|_url, username_from_url, _allowed_types| {
      if let Some(token) = auth_token.as_deref() {
          Cred::userpass_plaintext(username_from_url.unwrap_or(""), token)
      } else {
          Cred::default()
      }
  });
  ```
  It ignores `_url`. If the remote redirects from `https://github.com/x.git`
  to `https://attacker.example/x.git`, libgit2 follows the redirect (smart
  HTTP allows it) and re-invokes the credential callback with the
  attacker's URL. The callback **hands the same token** to the
  attacker's host — the auth_token is bound to the *repository row*
  in `llm_repositories`, not to the host the request actually
  reached. There is no `if !same_origin(url, original_url) {
  return Cred::default() }` guard.

  HTTP redirects in libgit2 are constrained by the curl version it
  was built against; modern libcurl by default permits redirects but
  drops the Authorization header on cross-origin redirects — except
  when authentication is via the URL userinfo (`https://user:pass@host`),
  in which case libcurl re-uses the userinfo on the new host. git2's
  `userpass_plaintext` translates to userinfo for HTTPS, so the cred
  may be re-sent on redirect.
- **Recommendation:** Inspect `_url` in the callback and refuse to
  return credentials unless the host matches the originally requested
  host (capture `original_host` outside the closure, compare against
  `url::Url::parse(url).host_str()` on each invocation, error out on
  mismatch). For HTTPS, set libgit2's
  `http.followRedirects` to `initial` (or disable redirects entirely)
  to remove the redirect attack vector at the libcurl layer.

---

### F-13 — `eprintln!` / `println!` used instead of `tracing`, bypassing the central log subscriber
- **Severity:** **Low**
- **ASVS:** V7.1.1 (Logging policy), V7.1.3 (Log levels)
- **CWE:** CWE-778 (Insufficient Logging)
- **Location:** `src-app/server/src/modules/llm_repository/handlers.rs:43,
  89, 127, 172, 185, 234, 244`,
  `src-app/server/src/modules/llm_repository/utils.rs:207`,
  `src-app/server/src/utils/git/service.rs:209, 384`
- **Description:** Every database error in the handlers is logged
  with `eprintln!` to stderr. The `tracing` subscriber set up at
  application start formats structured JSON and routes by level;
  `eprintln!` bypasses both, producing inconsistent log lines and
  losing trace correlation IDs. Operationally annoying and a
  detection gap (queries against "all logs mentioning a repo ID"
  miss the eprintln-only lines).
- **Recommendation:** Replace every `eprintln!`/`println!` in this
  module with `tracing::error!`/`tracing::warn!`/`tracing::debug!`
  at appropriate levels.

---

### F-14 — Inconsistent error reporting: `test_repository_connection` returns the upstream status code in the response body
- **Severity:** **Low**
- **ASVS:** V8.1.6 (Generic error responses)
- **CWE:** CWE-209 (Information Exposure Through an Error Message)
- **Location:** `src-app/server/src/modules/llm_repository/utils.rs:240-261`
- **Description:** On failed connection, the handler returns
  `format!("Connection to {} failed: {}", request.name, e)` where
  `e` is one of `"Connection timed out"`, `"DNS resolution failed:
  {full reqwest error}"`, `"Connection failed: {full reqwest
  error}"`. The full `reqwest::Error`'s `Display` impl includes the
  resolved IP address, target port, OS errno, and (sometimes) the
  TLS chain root name. This fingerprints internal network topology
  (combined with F-01, this is the *exfiltration* side of the SSRF
  oracle).
- **Recommendation:** Return a generic `"connection failed"` to
  clients; log the detailed error server-side at `debug!`.

---

### F-15 — `RepoBuilder::clone` runs `--share-net` equivalent (no DNS/host pinning)
- **Severity:** **Low**
- **ASVS:** V9.1.1, V12.6.1
- **CWE:** CWE-350 (Reliance on Reverse DNS Resolution)
- **Location:** `src-app/server/src/utils/git/service.rs:451-471`
- **Description:** Even with F-01's allowlist applied at validation
  time, the actual DNS resolution happens later inside libgit2 (or
  inside reqwest, for `test_repository_connection`). There is no
  shared resolver instance pinned to a non-rebinding cache. DNS
  rebinding (TTL-0, first answer public, second answer
  127.0.0.1) is open. Defence-in-depth requires a custom resolver.
- **Recommendation:** Build the reqwest client with a custom
  `Resolve` impl that resolves once and reuses the result; for
  git2, this is harder (libgit2 → libcurl resolves internally),
  but the impact is reduced if F-01's allowlist is enforced
  *server-side after resolution*, e.g. via a `socks5` filter
  process that checks each connect destination.

---

### F-16 — Built-in repositories trust the migration-time URL forever; no integrity check on URL changes
- **Severity:** **Low**
- **ASVS:** V8.1.4 (Sensitive data integrity)
- **CWE:** CWE-345 (Insufficient Verification of Data Authenticity)
- **Location:**
  `src-app/server/migrations/00000000000002_create_llm_repositories_table.sql:24-26`,
  `src-app/server/src/modules/llm_repository/repository.rs:162-220`
  (update_llm_repository)
- **Description:** The migration inserts Hugging Face and GitHub
  with `built_in = true` and *fixed* URLs. `delete_llm_repository`
  refuses to delete built-in rows, but `update_llm_repository`
  does **not** restrict edits to built-in rows — any user with
  `llm_repositories::edit` can change the Hugging Face row's URL
  to `https://attacker.example/`, and every subsequent `find_by_url`
  lookup or hub-driven download will reach attacker.example.

  The `built_in` flag is therefore semantically meaningless on
  update — only on delete.
- **Recommendation:**
  1. In `update_llm_repository`, refuse to change `url`,
     `auth_type`, or `built_in` of any row where `built_in = true`.
     Allow `enabled`, `auth_config` (operator credentials) to
     change.
  2. Add a per-row `url_hash` column computed at insert; reject
     updates that change the hash unless the request includes an
     admin elevation.

---

### F-17 — Info-class cleanup / audit / atomicity gaps (orphan caches, no audit log, non-atomic multi-UPDATE, mis-located probe URL)
- **Severity:** **Info** (4 sub-items)
- **ASVS / CWE:** V4.3.2 / CWE-459 (orphan cache dirs); V7.1.4 /
  CWE-778 (no audit log); V4.3.2 / CWE-362 (non-atomic update);
  architectural (probe URL co-located with secrets)
- **Location:** `repository.rs:162-249`, all handlers, `models.rs:27-28`
- **Description:**
  1. **Orphan caches on delete.** Schema declares `ON DELETE CASCADE`
     on `download_instances.repository_id` and
     `llm_models.repository_id` (good), but the cloned cache dir at
     `~/.cache/ziee-chat/models/git/<repo_id>-<hash>/` is not
     cleaned. Deleting a repo leaks disk indefinitely.
  2. **No audit trail.** `LlmRepositoryEvent::{created,updated,deleted}`
     carry no acting user ID; no `audit_log` insert in the same tx;
     eprintln logs lack identity. Cannot reconstruct "who changed
     Hugging Face URL to attacker.example".
  3. **Non-atomic update.** `update_llm_repository`
     (`repository.rs:162-220`) issues 5 separate `UPDATE` statements
     with no surrounding transaction. Partial failure leaves
     inconsistent rows (e.g., `auth_type = "api_key"` with
     `auth_config.token = "..."` and no `api_key`).
  4. **Probe URL co-located with secrets.**
     `auth_test_api_endpoint` lives inside `RepositoryAuthConfig`
     next to `api_key`/`password`/`token`. This complicates
     redaction (cannot show the endpoint without the secrets) and
     muddies the threat model.
- **Recommendation:**
  - On delete, enqueue cleanup of all cache directories beginning
    with the deleted UUID.
  - Add user_id to every `LlmRepositoryEvent`; insert an
    `audit_log` row in the same tx; redact secret bytes in audit
    payload (record `auth_type` changes and "url: X → Y" only).
  - Wrap `update_llm_repository` in `pool.begin()/.commit()`, or
    rewrite as a single dynamic UPDATE.
  - Move `auth_test_api_endpoint` to a top-level field on
    `LlmRepository`; the redaction story becomes trivial.

---

## ASVS Coverage Matrix

| ASVS Requirement | Section | Coverage | Finding(s) |
| --- | --- | --- | --- |
| V4.3.1 — ACL on resources | Access Control | **Gap** — repos global, no per-user owner | F-09 |
| V4.3.2 — Atomic state, referential integrity | Access Control | Partial — schema cascades; updates non-atomic | F-17, F-19 |
| V5.1.3 — Numeric range validation | Validation | **Gap** — page/per_page unbounded | F-11 |
| V5.1.4 — Input length limits | Validation | **Gap** — name unbounded (255 only at DB) | F-10 |
| V5.1.5 — URL scheme allowlist | Validation | **Critical gap** | F-01, F-03, F-08 |
| V5.2.6 — URL validation | Validation | **Critical gap** | F-01, F-03 |
| V5.3.3 — Output encoding | Encoding | Partial — relies on UI | F-10 |
| V6.1.1 — Secrets in storage | Cryptography | **Gap** — JSONB plaintext | F-02 |
| V7.1.1 — Logging policy | Logging | **Gap** — println/eprintln bypass tracing | F-13 |
| V7.1.3 — Log levels | Logging | **Gap** — everything at println/eprintln level | F-13 |
| V7.1.4 — Security-relevant events with identity | Logging | **Gap** — events lack user identity | F-18 |
| V8.1.4 — Sensitive data integrity | Data Protection | **Gap** — built-in URLs editable | F-16 |
| V8.1.6 — Generic error responses | Data Protection | **Gap** — upstream errors echoed | F-14 |
| V8.3.4 — No unnecessary sensitive return | Data Protection | **Gap** — credentials in every response | F-02 |
| V8.3.5 — Sensitive data cleansing | Data Protection | **Gap** — URLs with creds logged | F-04 |
| V9.1.1 — TLS for sensitive data | Communications | Partial — `https` not enforced | F-01, F-03 |
| V9.1.3 — Authenticated channel constrained to host | Communications | **Gap** — credential callback unscoped | F-12 |
| V10.3.2 — Subprocess hardening (library analogue) | Malicious Code | **Gap** — git2 unisolated | F-07 |
| V12.1.1 — Resource exhaustion limits | Files | **Gap** — no clone-depth, no LFS size cap | F-05 |
| V12.3.4 — Path traversal in file storage | Files | **Gap** — LFS oid not sanitised | F-06 |
| V12.4.1 — File size limits | Files | **Gap** | F-05 |
| V12.6.1 — SSRF protection on outbound URL fetches | Files | **Critical gap** | F-01, F-03, F-08, F-15 |

---

## Positive Findings

1. **Permission gating is consistent.** Every route uses
   `RequirePermissions<(LlmRepositories<X>,)>` — no anonymous
   routes, no `auth: Option<...>`. The permission grammar
   (`llm_repositories::read|create|edit|delete`) is well-scoped
   per-action.

2. **Built-in rows are protected from deletion.**
   `delete_llm_repository` explicitly refuses to delete `built_in =
   true` rows and returns a clear bad-request error. (Caveat:
   update is *not* similarly restricted — see F-16.)

3. **SQL is parameterised throughout.** All `sqlx::query!`
   macros use positional `$1..$N` parameters; no string-interpolated
   SQL. The compile-time SQLx verification in `build.rs` ensures
   schema drift is caught at build time.

4. **Strict request shape:** `CreateLlmRepositoryRequest` is
   `#[serde(deny_unknown_fields)]`, blocking attacker-injected
   fields from silently round-tripping through deserialisation.

5. **Auth-type allowlist is explicit:** `validate_auth_type`
   compares against the fixed array
   `["none", "api_key", "basic_auth", "bearer_token"]`. No
   reflective construction.

6. **Pagination is computed safely** for in-range inputs (no
   integer overflow in `(page - 1) * per_page` for the practical
   row count of this table).

7. **Cancellation token is plumbed through to the clone callback**
   (utils/git/service.rs:131-145) — long-running operations are
   interruptible (within the 100 ms poll granularity).

8. **LFS download verifies SHA-256** (`lfs/service.rs:355-361`)
   before promoting the temp file to the cache. Bad bytes don't
   land in the cache. (Caveat: only AFTER the temp file is fully
   written — see F-05 for the disk-fill before verification.)

9. **`urlencode_userinfo` / `set_password` use the safe `url`
   crate API** rather than format!-style string concatenation.

10. **No subprocess invocation in this module.** Everything is
    in-process via git2 and reqwest. No `Command::new("git")` argv
    parsing concerns apply directly — the SSRF/scheme concerns
    apply but the V10.3.2 argv-hygiene concerns are inapplicable
    here. (They apply outside this module: `file/utils/pandoc.rs`,
    audited separately.)

---

## Out of Scope / Deferred

- **llm_model upload + download flow** (`uploads.rs`) — F-04, F-05,
  F-06, F-12 reach into it but the module itself is in the next
  audit slot. Findings noted with cross-references.
- **llm_provider OAuth handling** — separate audit (the previous
  pass's HIGH-1 / HIGH-2 covered some overlap with provider
  credentials).
- **Hub module** (`hub/handlers.rs`) — only the `find_by_url`
  consumer touches `llm_repository`; the rest of the hub catalogue
  manifest verification is out of scope here.
- **MCP/code_sandbox interaction with downloaded models** — out of
  scope.
- **UI rendering** — F-10 (XSS) depends on whether the UI escapes
  the `name` field. Confirmation is a UI-audit task.
- **Cosign-style integrity on cloned model weights** — out of
  scope (handled by a separate model-signing workstream if any).
- **Frontend store/event flow for repository management** — N/A
  for backend audit.

---

## Cross-references to prior audit `.sec-audits/04-llm-modules-audit.md`

- 04-audit HIGH-1 (creds in API responses) → this audit **F-02**
  (still unfixed; severity confirmed High).
- 04-audit HIGH-2 (SSRF) → this audit **F-01** (escalated to
  Critical based on the `auth_test_api_endpoint` amplifier the
  prior audit did not analyse, and the cred-theft amplifier
  through git2's credential callback that the prior audit also did
  not analyse).
- 04-audit HIGH-3 (file upload validation) → out of scope here;
  belongs to the llm_model audit.

The remediation guidance below repeats the prior audit's
recommendations and adds the new ones discovered:

1. (From 04, restated) Separate response model with secrets
   redacted; encrypt secrets at rest.
2. (From 04, restated) Strict URL allowlist (`https` only, no
   IP literals, no IMDS/loopback/RFC1918).
3. (New) Validate `auth_config.auth_test_api_endpoint` through
   the same allowlist.
4. (New) Strip userinfo from URLs before storage / logging.
5. (New) Shallow clone (`depth=1`) + per-clone byte cap.
6. (New) OID hex-length validation in LFS metadata parser.
7. (New) Same-origin credential scoping in git2 callback.
8. (New) Move tracing off `eprintln!`/`println!`.
9. (New) Protect built-in row URL/auth_type from edit.
10. (New) Audit log with user identity for every CRUD op.

---

**Auditor's note.** Concentrated severity comes from a small set of
trust-boundary mistakes: the module accepts free-form URLs from
authenticated users, stores plaintext secrets next to those URLs,
echoes both back to every reader, and feeds them to libgit2 / libcurl
without scope-narrowing the credentials. Fixing F-01 (URL allowlist
+ resolver pinning) and F-02 (response redaction + at-rest
encryption) closes the bulk of the externally-exploitable surface;
F-03 through F-06 close the local-defence-in-depth gaps.
