# Security Audit — LLM Local Runtime Module
**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/llm_local_runtime/` (~2,951 LOC) — local LLM binary deployment, runtime lifecycle, version & binary management, health & log endpoints.
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Focus chapters:** V10 (Malicious Code in Dep / Subprocess), V12 (Files), V9 (Communication), V5 (Validation), V4 (Access Control)

---

## Executive Summary

The `llm_local_runtime` module is the server's mechanism for fetching pre-built `llama-server` / `mistralrs-server` binaries from a hardcoded pair of GitHub repositories (`ziee-ai/llama.cpp`, `ziee-ai/mistral.rs`), caching them under `~/.llm-runtime/binaries/`, and spawning them as long-running local subprocesses to serve inference requests. It exposes 14 HTTP endpoints (model start/stop/restart/status/health/logs, runtime-version CRUD, system-default selection, GitHub-update checks, cache sync). All endpoints are guarded by `RequirePermissions` extractors backed by RBAC — none are anonymous or self-service. The module integrates with the `llm-runtime` crate (`src-app/llm-runtime/`) for the actual fetch + extract + spawn primitives.

The most consequential security issues center on **supply chain trust of downloaded binaries** and **how user-controlled data is fed into subprocess argv**:

1. **Downloaded engine binaries are NOT integrity-verified.** No SHA-256, no Sigstore/cosign, no GPG, no GitHub-Releases-signature check. Transport security (HTTPS to `github.com`) is the *only* defense against arbitrary code execution on the server host. (Contrast `code_sandbox`, which performs `sigstore` keyless verification before mounting.)
2. **The user-controlled `model.name` field is passed verbatim as the `--model <path>` CLI argument** to `llama-server` / `mistralrs-server` with no validation against leading `-`, embedded NUL/CRLF, path traversal, or shell-metacharacters. An admin with `LlmModel.write` can store a model whose `name = "--rpc-server-addrs"` (or other engine flag) and cause flag injection at spawn time. The current llama.cpp flag surface includes flags that load remote weights, alter ports, write logs to arbitrary paths, etc.
3. **No `Command::env_clear()` and no `PR_SET_PDEATHSIG` / `kill_on_drop`.** The spawned llama-server inherits the entire server environment (which on a typical deployment contains `DATABASE_URL`, `JWT_SECRET`, OAuth client secrets, `*_API_KEY` for upstream LLM providers, etc.). A compromised/buggy engine binary — or an admin who downloads a maliciously-mirrored release in the future — has direct access to these secrets via `/proc/self/environ`. Additionally, if the server is hard-killed (OOM, SIGKILL, panic) the engine processes are orphaned.
4. **The TLS / reqwest client is built with defaults that do not pin the GitHub TLS chain** and the HTTP client used for binary downloads does not set a maximum response size, redirect cap, or timeout (download phase has no upper bound). A network-position adversary who can MITM the TLS chain (corporate proxy, system CA poisoning) reaches code execution.

There is **no host isolation** of the spawned engine: no user/group namespace, no seccomp, no cgroup memory/CPU limits, no rlimits. Resources are bounded only by what the engine binary self-imposes. This is consistent with the design intent (local single-tenant server, admin-trusted models) but should be made explicit in the threat model.

The module's RBAC design is otherwise solid: all 14 routes carry `RequirePermissions<...>` extractors, the permission set is fine-grained (`Read`/`Manage`/`Logs` + `Create`/`Update`/`Delete` for versions), and the underlying RBAC engine has an `is_admin` fast-path bypass plus group-union semantics audited elsewhere (see `.sec-audits/2026-05/02-permissions.md`).

### Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 4 |
| Medium | 5 |
| Low | 4 |
| Info | 3 |
| **Total** | **16** |

### Top-3 risks (must-fix)

1. **F-01 (High)** — No integrity verification of downloaded engine binaries. A malicious mirror, GitHub account compromise, or TLS MITM yields arbitrary code execution on the server host with the server's full environment + filesystem.
2. **F-02 (High)** — `model.name` (user-controllable by any admin/`LlmModel.write` holder) flows directly into the `--model` argv slot without validation. Flag injection (`--rpc`, `--log-file`, etc.) is possible because the engine binaries treat unknown leading-`-` strings as flags.
3. **F-03 (High)** — Spawned engine processes inherit the server's full environment (no `env_clear()`), exposing `DATABASE_URL`, `JWT_SECRET`, OAuth secrets, and upstream-provider API keys to whatever the engine binary chooses to do with `/proc/self/environ` or `getenv()`. Combined with F-01 this is a credential-exfiltration primitive.

---

## Findings

Severity definitions (per ASVS 4.0.3 risk-rating guidance, scaled to this codebase):
- **Critical** — directly exploitable from an unauthenticated or low-privilege user, yields RCE / full-DB exfil / auth bypass.
- **High** — exploitable from an authenticated admin or via a realistic supply-chain compromise; yields RCE, secret exfiltration, or persistent host compromise.
- **Medium** — local-blast-radius bug (DoS, log injection, resource exhaustion, weak hardening) requiring admin or specific conditions.
- **Low** — hardening gap / defense-in-depth deficiency; not directly exploitable.
- **Info** — observation worth noting; no exploit path identified.

---

### F-01 — No integrity verification of downloaded engine binaries (High)

**ASVS:** V10.3.2 (Application Integrity), V14.2.4 (third-party libraries verified)
**Files:** `src-app/llm-runtime/src/binary_download.rs` (the entire `download` flow), `src-app/server/src/modules/llm_local_runtime/binary_manager.rs:43-98` (`download_and_register`)

**Observation.**
`BinaryDownloader::download` (`binary_download.rs:85-207`) fetches an archive from
`https://github.com/{repo}/releases/download/{version}/{archive_name}`, writes it to `~/.llm-runtime/binaries/.tmp/`, extracts the binary and its `.so`/`.dylib`/`.dll` siblings, and marks the binary executable. **No checksum, no Sigstore/cosign verification, no GPG signature check, no GitHub Releases artifact-attestation check** is performed at any point. The downloaded binary becomes the engine that the server will spawn on subsequent `POST /local-runtime/models/{model_id}/start`.

For comparison, `code_sandbox` (a peer feature in this repo, see project `CLAUDE.md`) performs in-process keyless Sigstore verification via the `sigstore` Rust crate against a pinned `known_revisions.toml` of SHA-256 + cosign bundles before mounting its rootfs. `llm_local_runtime` ships none of that machinery.

**Exploit chain (any one of these is sufficient):**
1. A future operator-error / phishing / 2FA-bypass against the `ziee-ai` GitHub org publishes a malicious release. Every server with `LlmRuntimeVersion.create` permission that downloads that version executes arbitrary code on next `instance.start`.
2. A network-position adversary that can present a forged TLS certificate (corporate MITM proxy with installed root CA, supply-chain CA compromise, or stolen GitHub cert) replaces the archive bytes during transit. reqwest's defaults rely entirely on the system root store; nothing is pinned.
3. An attacker who can write to `~/.llm-runtime/binaries/.tmp/` between download and extract can race the file (the temp dir is shared per-user; no per-download randomised subdir).

**Impact.** Arbitrary code execution as the server uid, with full filesystem access to the model store, the SQLite/Postgres connection, and the entire environment (see F-03). Persistent: the binary is cached and re-used on every subsequent spawn.

**Recommendation.**
- Adopt the same pattern `code_sandbox` already uses in this repo: ship a `known_revisions.toml` in the server crate that pins `{engine, version, platform, arch, backend} → sha256` and, for production builds, a cosign bundle. Verify both before registering the row in `llm_runtime_versions`.
- Reject any future "latest" resolution that doesn't have a known-good pin (or require the operator to explicitly opt in to unsigned-latest).
- At minimum, in the near term: verify a SHA-256 from a server-baked-in pinned table before extracting the archive.

---

### F-02 — Argv flag-injection via user-controlled `model.name` (High)

**ASVS:** V5.2.7 (output encoded for the OS command interpreter), V5.3.8 (avoid OS command injection)
**Files:**
- `src-app/server/src/modules/llm_local_runtime/handlers.rs:61` — `let model_path = model.name.clone();`
- `src-app/server/src/modules/llm_local_runtime/handlers.rs:228` — same in `restart_model_instance`
- `src-app/server/src/modules/llm_local_runtime/deployment/local.rs:46-94` — `build_llamacpp_command` / `build_mistralrs_command`
- `src-app/server/src/modules/llm_model/utils.rs:9-63` — `validate_create_request` (no charset/prefix validation)

**Observation.**
At spawn time, the local runtime builds the engine argv as:
```rust
cmd.arg("--model").arg(model_path);     // llama.cpp
cmd.arg("--model-path").arg(model_path); // mistral.rs
```
`model_path` is `model.name`, which is taken from `CreateLlmModelRequest.name` — accepted from any authenticated user with `LlmModel.create` permission (in practice, admin) and validated only for **non-empty + length ≤ 255 characters**. There is no validation against:
- Leading `-` or `--` (argv-flag injection — *llama.cpp and mistral.rs both treat `-flag`-style args after `--model` as continuing positional?*; actually they do **not**, because `--model` consumes exactly one value. But: if a future flag is added or the engine misparses, a name like `--model-path /etc/shadow --log-file /tmp/x` becomes one OsString and is rejected at engine-parse time. **The realistic flag-injection is in the OTHER fields**: `config.json` keys via `serde_json::Value`, see below.)
- Embedded `\0` (rejected by Rust `OsStr`, so this is moot on Linux).
- Path-traversal (`../../etc/passwd`) — would just fail to open as a GGUF.
- Network paths or `file://` URIs — llama.cpp's `--model` accepts plain paths only; safe in current engine version.

The more material issue is in the same `build_llamacpp_command`:
```rust
if let Some(ctx_size) = config.get("context_size").and_then(|v| v.as_i64()) { ... }
if let Some(n_gpu_layers) = config.get("n_gpu_layers").and_then(|v| v.as_i64()) { ... }
```
The `config` parameter is `&serde_json::Value` and is currently hard-coded to `json!({})` at the call sites (`handlers.rs:69`, `handlers.rs:236`). **The infrastructure to pass arbitrary user JSON straight into argv exists** — when the `StartInstanceRequest` (currently an empty struct, `models.rs:13`) is later extended to forward fields, every `config.get("key")` that returns a string will become a sink.

Additionally, the `build_mistralrs_command` already does:
```rust
if let Some(model_type) = config.get("model_type").and_then(|v| v.as_str()) {
    cmd.arg("--model-type").arg(model_type);
}
```
This `model_type` flag value passes through with no allow-list — if a future endpoint plumbs user JSON in, a value like `gguf --some-other-flag --log-file /etc/cron.d/exfil` is *split into two argv slots only if the user injects a `--` separator that the runtime doesn't honor*; here, since `.arg(model_type)` passes the whole string as a single OsString, it cannot split argv. The flag-injection risk is in the parser: if mistralrs-server treats unknown `--model-type` *values* as silently splitting on whitespace internally (some `clap` configurations do not), the whole string becomes a single value. Verify behavior in mistralrs's `--model-type` parser before exposing.

**Compounding factor.** The `--` argv separator is never inserted between the flag-bearing args and the `model_path` (or before `config`-derived values). Best practice for any subprocess that mixes flags + positional user data is to insert `cmd.arg("--")` before the first positional. llama.cpp's `--model` is value-bearing and not positional, so this doesn't help in the current spec, but it's a defense-in-depth gap.

**Recommendation.**
- Validate `model.name` at the `llm_model` validation layer with a strict allow-list (e.g. `^[A-Za-z0-9._-]{1,128}$`, and explicitly reject leading `-`).
- For `model_path` that is supposed to be a real filesystem path, use `ModelStorage::get_model_path(provider_id, model_id)` (already present at `src-app/server/src/modules/llm_model/storage.rs:75`) instead of plumbing the user-controlled `name` field. The current code passes a *name string* as a *path* — that's a category bug separate from injection.
- Before adding any future `config.get(...)` plumbing from user input, build a closed map of `key → enum-bounded value` and reject anything else.

---

### F-03 — Spawned engine inherits server environment; no `env_clear()` (High)

**ASVS:** V10.2.3 (sensitive information should not be exposed to processes that do not need it)
**Files:** `src-app/server/src/modules/llm_local_runtime/deployment/local.rs:46-94`, `src-app/llm-runtime/src/engine/llamacpp.rs:215-224`

**Observation.**
Neither `build_llamacpp_command` nor `build_mistralrs_command` calls `Command::env_clear()` before adding the engine-specific args. The spawned child inherits the entire environment of the Axum server, which on every deployment of this codebase includes (per `src-app/server/config/dev.yaml` and the README):
- `DATABASE_URL` (Postgres credentials with full schema access),
- `JWT_SECRET` (server's HMAC key — full token forgery on leak),
- `*_API_KEY` for upstream LLM providers (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.) if configured globally,
- OAuth client secrets,
- HuggingFace token (`HUGGINGFACE_API_KEY`).

A compromised engine binary (see F-01), or any future engine bug that logs environment, leaks all of these. The `code_sandbox` peer module documents the same threat and uses `--clearenv` via bwrap; `llm_local_runtime` ships nothing equivalent.

In addition:
- **No `kill_on_drop` on the `tokio::process::Command`.** If the Axum server process dies abruptly (panic, SIGKILL, OOM), the spawned engine becomes an orphan reparented to PID 1, still listening on 127.0.0.1:<port>, still holding the model in RAM. There is no Drop impl on `LocalDeployment` or `DeploymentManager`.
- **No `PR_SET_PDEATHSIG` on Linux** — would deliver SIGTERM to the engine when the server exits.
- **No process-group / setsid** — kill signals are per-PID; if the engine fork()s a worker (mistralrs sometimes does), `child.kill()` only kills the head process.

**Recommendation.**
- Call `cmd.env_clear()` then explicitly re-add only the minimal set the engine needs (`HOME`, `PATH=/usr/local/bin:/usr/bin:/bin`, and any GPU env like `CUDA_VISIBLE_DEVICES` if configured per-instance — never the whole env).
- Set `cmd.kill_on_drop(true)` so a server crash collects the engine.
- On Linux, use `pre_exec` to call `prctl(PR_SET_PDEATHSIG, SIGTERM)` and `setsid()` (or set a new process group with `pgid(0)`), so the engine dies if the server is OOM-killed.
- Add a `Drop` impl on `LocalDeployment` that signals all tracked PIDs.

---

### F-04 — Health-check endpoint exposed on `127.0.0.1` is fine, but no auth on the engine itself (High)

**ASVS:** V4.1.1 (access control on all endpoints), V13.1.1 (each component authenticates upstream)
**Files:** `src-app/server/src/modules/llm_local_runtime/deployment/local.rs:55, 83` (engine `--host 127.0.0.1`)

**Observation.**
Good news: both `build_llamacpp_command` and `build_mistralrs_command` hard-code `--host 127.0.0.1`, so the engine only listens on loopback. This blocks LAN attackers reaching the engine HTTP API.

Bad news: the engine **has no authentication**. Anything that lands on `127.0.0.1:<random_port>` (port chosen by `portpicker::pick_unused_port()` and stored in the DB) gets unauthenticated access to the OpenAI-compatible endpoints — `/v1/chat/completions`, `/health`, and **`/v1/embeddings`**, which is enough to read arbitrary text the model has been told to memorize, run untrusted prompts (denial-of-wallet for hosted backends not applicable here, but local CPU/GPU burn), and on llama.cpp's server **the `/v1/completions` `prompt` field is the entry point for full unrestricted inference**. This matters because:

- **Any local user on the host** (other system accounts, docker sidecars, browser-rendered fetch from a same-origin compromise, port-scanning tooling) can hit the loopback endpoint without credentials.
- **Server-side request forgery (SSRF) elsewhere in the codebase** that allows targeting `http://127.0.0.1:<port>/v1/chat/completions` reaches the engine without authentication. The `llm_provider` module's outgoing proxy/repo-download code is one such candidate (see prior `.sec-audits/04-llm-modules-audit.md` HIGH-2).
- The chosen port is stored unencrypted in `llm_runtime_instances.local_port` and returned to any caller with `LocalRuntimeRead` permission via `GET /local-runtime/models/{model_id}/instance` — so the port is also database-discoverable.

**Recommendation.**
- The engine binaries (llama.cpp's `server`, mistral.rs's `server`) both support `--api-key` flags. Generate a per-instance random secret at spawn time, pass it as `--api-key`, store it server-side (in-memory only — never serialised to the DB), and require all forwarded requests from the chat module to include `Authorization: Bearer <secret>`.
- Alternatively, bind the engine to a Unix domain socket in a directory chmod'd to the server uid only, and proxy through the Axum server. The engines' `--port` flag does not support UDS today, so this requires a wrapper or an upstream patch — `--api-key` is the pragmatic choice.

---

### F-05 — Tar/zip extraction follows symlinks; archive can write outside cache dir (Medium)

**ASVS:** V12.3.1 (path traversal on file uploads/downloads)
**Files:** `src-app/llm-runtime/src/binary_download.rs:286-393`

**Observation.**
`extract_tar_gz` configures `set_preserve_permissions(true)`, `set_preserve_mtime(true)`, `set_unpack_xattrs(true)` but does *not* configure `set_overwrite(false)` or `set_unpack_xattrs(false)`. The tar crate's default symlink handling will create symlinks defined in the archive at the destination path. The code does filter file entries by basename (`file_name`) and joins them to `dest_dir`, but **symlink entries are not filtered out** — `entry.header().entry_type().is_dir()` is the only early-skip. A symlink entry whose `name` is `libfoo.so` (matching the `.contains(".so.")` filter) and whose target is `../../../home/server/.ssh/authorized_keys` would be created at `<cache>/libfoo.so → /home/server/.ssh/authorized_keys`. Any subsequent file entry sharing that name would then write *through* the symlink.

The same issue applies to `extract_zip`: zip filenames are sanitised by `Path::new(name).file_name()` (which strips traversal), but zip's `by_index()` does not deduplicate names within the archive. If `set_preserve_permissions` were enabled for zip (it isn't), the same path could be opened twice.

Risk gated by F-01: only realisable if an attacker can already control the archive content. Hence Medium not High.

**Recommendation.**
- In `extract_tar_gz`: before `entry.unpack`, check `entry.header().entry_type()`. Accept only `Regular`/`Continuous`; reject `Symlink`, `Hardlink`, `Block`/`Char`/`Fifo`/`GnuSparse`. Drop `set_unpack_xattrs(true)` (capability xattrs can grant CAP_NET_RAW etc. on the extracted binary).
- After unpacking each file, verify the canonicalised destination starts with `dest_dir.canonicalize()`.
- Do not preserve permissions blindly; instead set `0o755` on the binary and `0o644` on libraries.

---

### F-06 — No download size cap / timeout on engine fetch (Medium)

**ASVS:** V12.1.1 (file-upload size limits), V13.1.4 (avoid resource exhaustion via untrusted external services)
**Files:** `src-app/llm-runtime/src/binary_download.rs:235-283`

**Observation.**
`reqwest::Client::builder().user_agent(...).build()` builds a client with **no `.timeout(...)`** and **no `.connect_timeout(...)`**. The streaming download (`response.chunk().await?`) has no upper byte cap; the only loop exit is server EOF. A malicious mirror — or a slowloris-style GitHub-spoofing host — can hold the connection open indefinitely, or stream multi-GB junk and fill the server disk.

The `head_response` is used only to display a progress bar; its `Content-Length` is not enforced against the streamed bytes.

**Recommendation.**
- Add `.timeout(Duration::from_secs(600))` and `.connect_timeout(Duration::from_secs(15))` on the client builder.
- Track bytes read into the file; abort when the total exceeds a hard cap (e.g. 2 GB) or exceeds `Content-Length * 1.1`.
- Add `.redirect(reqwest::redirect::Policy::limited(3))` and refuse redirects whose host is not `github.com` or `objects.githubusercontent.com`.

---

### F-07 — No quota on concurrent instances; unlimited fork bomb via `start` (Medium)

**ASVS:** V12.4.1 (resource limits on per-user/per-request basis)
**Files:** `src-app/server/src/modules/llm_local_runtime/handlers.rs:26-124` (`start_model_instance`), `deployment/local.rs:140-249`

**Observation.**
`LocalDeployment::start` checks only that the *same `model_id`* is not already running (`processes.contains_key(&model_id)`). It does not enforce:
- A maximum number of concurrent instances (per-user, per-provider, or global).
- A check against available RAM / GPU memory (each llama-server allocates the model into RAM; spawning N instances of a 70B model trivially OOMs the host).
- Any cgroup/rlimit caps on the child (no `setrlimit(RLIMIT_AS)`, no cgroup v2 memory.max).

An admin with `LocalRuntimeManage` can iterate over all models and call `/start` on each, scaling RAM use linearly. While `LocalRuntimeManage` is admin-only, this is also reachable via any token leak from an admin session.

**Recommendation.**
- Cap concurrent instances per-user and globally; surface as `code_sandbox.max_instances`-style config.
- On Linux, place each engine in a per-instance cgroup v2 slice (the pattern already exists in `code_sandbox`) with `memory.max` and `pids.max`.
- Apply rlimits (`RLIMIT_AS`, `RLIMIT_CPU`, `RLIMIT_NPROC`) via `pre_exec` before exec.

---

### F-08 — Log capture grows unbounded across reconnects; line-removal is O(n²) (Medium)

**ASVS:** V7.3.3 (log integrity), V12.4.2 (resource exhaustion via log channels)
**Files:** `src-app/server/src/modules/llm_local_runtime/deployment/local.rs:96-137`, `:330-345`

**Observation.**
The `capture_logs` task reads `stdout`/`stderr` line-by-line and appends to `proc_info.logs: Vec<String>`. Truncation logic:
```rust
if proc_info.logs.len() > 1000 {
    proc_info.logs.remove(0);
}
```
This is per-line `Vec::remove(0)`, which is O(n) (shifts all subsequent elements). Over the life of a chatty engine, that's quadratic in the number of lines. More importantly:

- There's no **per-line size limit**. A malicious engine binary (or a chunked binary error message) can emit a single 1 GB line and fill memory because `lines.next_line()` reads until LF.
- There's no log redaction: API keys, prompt PII, model weights printed at debug verbosity, all flow into `proc_info.logs` and out via `GET /local-runtime/models/{model_id}/logs`. The `LocalRuntimeLogs` permission gates the *read*, but the data is held in memory regardless.
- Two task handles per process (one stdout, one stderr) are spawned with no abort handle stored. When the process is removed from the map, the readers continue reading until EOF on the inherited FD — they're harmless but waste a tokio task each.

**Recommendation.**
- Use a `VecDeque<String>` and `pop_front()` (O(1)).
- Cap each line to 4 KiB (`reader.take(4096)` or check length and truncate).
- Cap total log byte count, not just line count.
- Either also write logs to a file on disk and serve from there (with file size cap), or use a ring-buffer crate.
- Store and abort the spawned reader tasks when `stop()` is called.

---

### F-09 — TLS / reqwest redirect & TLS behavior unhardened (Medium)

**ASVS:** V9.1.1 (TLS for all connections), V9.2.1 (validate TLS chain)
**Files:** `src-app/llm-runtime/src/binary_download.rs:55-66`, `src-app/server/src/modules/llm_local_runtime/binary_manager.rs:244-253`

**Observation.**
Both `reqwest::Client::builder().user_agent(...).build()` (binary_download.rs) and `reqwest::Client::new()` (binary_manager.rs `check_for_updates`) use defaults. The defaults are mostly safe (`danger_accept_invalid_certs(false)` is the default, TLS is via the system root store or rustls per feature), but:

- No `.https_only(true)` — if a future redirect from `github.com` redirects to `http://`, reqwest will downgrade silently.
- No `.redirect(Policy::...)` cap.
- No CT-log / pin / TOFU on `github.com`'s cert chain. A corporate-MITM proxy that installed its own root CA in the system trust store is sufficient to intercept.
- `User-Agent` is `llm-runtime/0.1.0` (binary_download.rs) and `ziee-chat/1.0` (binary_manager.rs) — informational only, not a security issue but worth being consistent for log-correlation.

**Recommendation.**
- Pass `.https_only(true)` and `.redirect(Policy::limited(3))`.
- If the threat model includes corporate-MITM proxies on the host, add `.add_root_certificate(...)` with the GitHub Let's Encrypt/Sectigo chain hard-pinned, OR adopt the cosign verification flow from F-01 (which makes the TLS chain irrelevant).

---

### F-10 — `model_path` is a model **name** masquerading as a path (Low)

**ASVS:** V5.1.4 (verify all input is of the expected type)
**Files:** `src-app/server/src/modules/llm_local_runtime/handlers.rs:60-61`, `:227-228`

**Observation.**
```rust
// Use model name as the path/identifier
let model_path = model.name.clone();
```
The comment acknowledges the conflation. The downstream `deployment.start(model_id, engine_type, model_path, config)` passes this as the `--model` value to llama.cpp. **llama.cpp interprets `--model` as a filesystem path to a GGUF**; passing a logical model name will fail at engine startup (file-not-found). This means:
- In the current codebase, no instance can actually start successfully via this code path unless the operator places a GGUF at exactly the model's name (relative to the engine's CWD).
- This bug, once "fixed" by plumbing the real path, becomes the entry point for argv flag injection (F-02). The fix needs to be done **with** the validation from F-02, not before.

This isn't a security bug *yet*, but the comment indicates planned work, and the planned work without validation enables F-02. Documented here as a Low so it doesn't slip through.

**Recommendation.**
- Resolve `model_path` via `ModelStorage::get_model_path(provider_id, model_id).join(file_format-aware-filename)`. This is server-controlled (UUID/UUID hierarchy) and cannot collide with argv flags.

---

### F-11 — `binary_path` field on `DeploymentConfig::Local` is API-exposed but unused (Low)

**ASVS:** V4.2.1 (attack-surface reduction), V14.1.5 (deprecated/unused interfaces removed)
**Files:** `src-app/server/src/modules/llm_local_runtime/models.rs:67-74`, `deployment/manager.rs:25-30`

**Observation.**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum DeploymentConfig {
    #[serde(rename = "local")]
    Local { binary_path: Option<String> },
}
```
The variant accepts a `binary_path: Option<String>` in its public OpenAPI schema, but `DeploymentManager::get_deployment` *ignores* it — `local.clone()` is returned unconditionally. The hosting code always passes `binary_path: None`. So today, this field is dead but advertised in the API schema (`/api/openapi.json`).

The concern is forward-compatibility: a future operator who plumbs the field through *without* validation creates a "set my own binary" primitive equivalent to "run arbitrary binary as the server uid" — because `Command::new(user_binary_path)` accepts any path.

**Recommendation.**
- Either remove the field from `DeploymentConfig::Local` (preferred — it's dead), or, if a future use is planned, gate it behind a config-file allow-list of paths that maps a logical key (e.g. `"llama-7b-q4"`) to a server-resolved absolute path.

---

### F-12 — `delete_version` `remove_binary: true` removes the *parent directory* (Low)

**ASVS:** V12.3.4 (file deletion limited to the intended target)
**Files:** `src-app/server/src/modules/llm_local_runtime/binary_manager.rs:192-227`

**Observation.**
```rust
if remove_binary {
    let binary_path = PathBuf::from(&version.binary_path);
    if binary_path.exists() {
        if let Some(parent) = binary_path.parent() {
            std::fs::remove_dir_all(parent)?;
            ...
        }
    }
}
```
The code removes the parent directory of the binary using `remove_dir_all`. Since `version.binary_path` is `<cache>/{engine}/{version}/{platform-arch-backend}/llama-server`, the parent is `<cache>/{engine}/{version}/{platform-arch-backend}/` — usually fine.

But the path comes from the database, which was inserted at download time from `binary_info.path.to_string_lossy()`. If for any reason the path were ever set to something like `/`, `/etc`, or `/home/server`, the code would happily try to `remove_dir_all` that. The DB path is currently only set by the downloader (server-controlled) and by `sync_cache` (also server-controlled, reading from cache_dir). There is no API to set it directly. So the risk is bounded — but the deletion logic should still defend against it.

**Recommendation.**
- Before `remove_dir_all(parent)`, assert that `parent.canonicalize()?.starts_with(self.downloader.binaries_dir().canonicalize()?)`.

---

### F-13 — Status endpoint leaks PID without permission gating beyond `Read` (Low)

**ASVS:** V8.3.4 (sensitive data not exposed to unauthorised users)
**Files:** `src-app/server/src/modules/llm_local_runtime/handlers.rs:334-369`

**Observation.**
`GET /local-runtime/models/{model_id}/status` and `GET /local-runtime/models/{model_id}/instance` return `pid`, `local_port`, and `base_url`. `LocalRuntimeRead` is a "view instance" permission — likely granted broadly to operators. The PID and port are arguably operational info but, combined with F-04 (engine has no auth on its 127.0.0.1 port), they form a "shoot here" pointer for any local-only adversary. The Instance API also returns `error_message`, which on engine startup failures contains the engine's first stderr line — potentially including filesystem paths revealing the cache layout.

Documented as Low because it requires another vulnerability (F-04 or local-host access) to weaponise.

**Recommendation.**
- Once F-04 is fixed (per-instance secret), omitting `local_port`/`pid` from the API stops being a security concern. Pre-F-04, consider returning the port only to `LocalRuntimeManage` holders.

---

### F-14 — `nvidia-smi`, `rocm-smi`, `system_profiler` invoked from inherited `PATH` (Low)

**ASVS:** V10.3.1 (subprocess discovery from secure path)
**Files:** `src-app/server/src/modules/llm_local_runtime/utils/gpu_detect.rs:64-148`

**Observation.**
`Command::new("nvidia-smi").output()`, `Command::new("rocm-smi").output()`, and `Command::new("system_profiler").arg("SPDisplaysDataType").output()` rely on whatever `PATH` the server inherits. If a server is launched from a shell whose `PATH` includes `/home/server/.local/bin:/usr/bin:...`, an attacker who can write to `~/.local/bin/nvidia-smi` gets code execution at GPU-detect time (server startup or whenever this runs).

In practice the `detect_gpu_backend()` function is not called from inside `llm_local_runtime` — verified by `grep` (no callers in the module). It's a utility intended for future use. Documented as Low so future use does not introduce the issue.

**Recommendation.**
- When/if this is wired up, resolve absolute paths via `which::which()` once at startup and store them, OR set a known `PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin` via `cmd.env("PATH", ...)`.
- Better: link `nvml-wrapper` / `rocm-smi-lib` as a Rust dependency instead of shelling out.

---

### F-15 — Engine `--port` and `--host 127.0.0.1` cannot be overridden — but neither can `--api-key` (Info)

**Files:** `src-app/server/src/modules/llm_local_runtime/deployment/local.rs:46-94`

The host-binding to `127.0.0.1` and the port from `portpicker` are both hard-coded — good. The lack of plumbing for any engine-side auth flags (no `--api-key`, no `--ssl-key-file`, no per-instance secret) is the gap noted in F-04, not a separate finding.

---

### F-16 — Log file path on `llm-runtime` standalone engine is CWD-relative (Info)

**Files:** `src-app/llm-runtime/src/engine/llamacpp.rs:198-212`

The standalone `llm-runtime` (CLI binary, not the Axum integration we're auditing) writes engine logs to `std::env::current_dir().unwrap_or(...) .join("logs").join(format!("{}_engine.log", config.id))`. The `config.id` is a string field — in the standalone CLI it's operator-set, so this is fine. **The Axum integration does not use this code path** (the integration uses `stdout/stderr` pipes captured into `proc_info.logs`). Noted because it would become relevant if the integration ever switched to `llm_runtime::Runtime::start` (which it doesn't today).

---

## ASVS Coverage Matrix

| ASVS § | Control | Status | Notes |
|---|---|---|---|
| **V1 — Architecture** | | | |
| V1.1.4 | Trust boundaries documented | Partial | Module-level threat model implicit; written here for the first time. |
| V1.2.1 | Defined trust between components | Yes | DB ↔ Axum server ↔ engine binary is clear. |
| **V4 — Access Control** | | | |
| V4.1.1 | Access controls enforced on every endpoint | **Yes** | All 14 routes carry `RequirePermissions<…>` extractors. |
| V4.1.3 | Least privilege for module operations | Yes | Read/Manage/Logs/Create/Update/Delete split. |
| V4.2.1 | Attack surface minimised | Partial | F-11 (dead `binary_path` field). |
| V4.2.2 | All routes require auth (no anonymous) | Yes | Verified by `RequirePermissions`. |
| **V5 — Validation, Sanitisation, Encoding** | | | |
| V5.1.1 | Input validation defined for all fields | Partial | `validate_create_request` checks length/empty only. |
| V5.1.3 | Validation against business rules | No | `model.name` admits values that break argv (F-02). |
| V5.2.7 | OS command-interpreter encoding | Partial | No shell is used (`Command::arg`), but flag-injection mitigation absent (F-02). |
| V5.3.8 | OS command injection avoided | Partial | Argv-based, but no `--` separator and no leading-`-` check. |
| **V7 — Errors & Logging** | | | |
| V7.1.1 | Logs do not contain credentials | Partial | Captured engine logs may contain secrets via F-03. |
| V7.3.3 | Log integrity | Partial | In-memory `Vec<String>` is mutated freely; F-08. |
| V7.4.1 | Errors do not leak sensitive data | Partial | `error_message` field surfaces engine stderr. |
| **V8 — Data Protection** | | | |
| V8.3.4 | Sensitive data minimisation in responses | Partial | F-13 (PID/port). |
| **V9 — Communication** | | | |
| V9.1.1 | TLS for all external connections | Yes | All GitHub fetches are HTTPS. |
| V9.2.1 | Server TLS chain validation | Yes (default) | But not pinned; F-09. |
| V9.2.3 | TLS configuration audited | No | No explicit redirect/timeout config; F-06, F-09. |
| **V10 — Malicious Code / Subprocess** | | | |
| **V10.3.2** | **Binary integrity verified** | **No** | **F-01 — no SHA-256 / cosign.** |
| V10.3.3 | Subprocess argv constructed safely | Partial | F-02. |
| V10.3.4 | Subprocess environment minimised | No | F-03. |
| V10.3.5 | Subprocess resource-bounded | No | F-07 (no cgroup / rlimit). |
| **V12 — Files & Resources** | | | |
| V12.1.1 | File-size limits on uploads/downloads | No | F-06. |
| V12.3.1 | Path traversal prevented | Partial | basename filter present; symlink filter absent (F-05). |
| V12.3.4 | File deletion scoped | Partial | F-12. |
| V12.4.1 | Resource limits per request | No | F-07, F-08. |
| **V13 — API & Web Service** | | | |
| V13.1.1 | Each API component authenticates upstream | No | Engine has no auth on its loopback port (F-04). |
| V13.1.4 | Avoid resource exhaustion from external services | No | F-06. |
| **V14 — Configuration** | | | |
| V14.1.5 | Deprecated/unused interfaces removed | No | F-11. |
| V14.2.4 | Third-party libraries are verified/integrity-checked | No | F-01. |

**Compliance summary:** 9 controls Yes, 12 Partial, 11 No (of 32 surveyed).

---

## Positive Findings

These are the things the module already does well. Preserve them through refactors.

1. **Routes consistently use `RequirePermissions`.** All 14 routes carry an explicit permissions extractor. The default Axum behaviour ("everything is anonymous unless guarded") is not the default here.
2. **Argv assembly uses `Command::arg(...)` per slot, not `format!()` into a shell string.** No `sh -c`, no string concatenation of arguments into a single command line. Argv-injection within a single arg slot (e.g. spaces inside a value) is not a vulnerability under this construction.
3. **Engine binds to `127.0.0.1` only.** `build_llamacpp_command` and `build_mistralrs_command` both hard-code `--host 127.0.0.1`. A LAN-side attacker cannot reach the engine directly. Counter: see F-04 for the in-host gap.
4. **All SQL is parameterised via `sqlx::query!` / `sqlx::query_as!` macros.** Compile-time-checked. No string concatenation, no dynamic SQL.
5. **GitHub repos are closed allow-listed.** `check_for_updates` and `download_and_register` route through a hard-coded match on `"llamacpp" → ziee-ai/llama.cpp` / `"mistralrs" → ziee-ai/mistral.rs`. No user-supplied repo path. (See F-01 for the integrity gap on the *content* fetched.)
6. **Port allocation uses `portpicker`** (random-available ports). No fixed-port collision attack.
7. **Process tracking uses `Uuid` model_id as the key**, preventing one model from impersonating another's instance.
8. **Permission system is fine-grained**: separate `LocalRuntimeRead`, `LocalRuntimeManage`, `LocalRuntimeLogs` plus version CRUD permissions. This is more granular than typical "admin/non-admin" splits.
9. **Module init enforces single-initialisation** via `OnceCell<Arc<DeploymentManager>>` with a clear panic message if `init()` was missed.
10. **`stop()` uses `tokio::time::timeout` around `child.wait()`** to bound shutdown to 10 seconds. Not perfect (no SIGKILL fallback in the local module — the underlying `child.kill()` does SIGKILL on Linux directly, so this is OK), but the timeout is correctly bounded.

---

## Out of Scope / Deferred

Items intentionally not assessed in this audit:

1. **llm_model download flow** (HF / repository fetches, GGUF validation, multipart upload handling). Per the audit charter — separate audit.
2. **The `llm_provider` / repository credential surface.** Already covered in `.sec-audits/04-llm-modules-audit.md` CRIT-1 / HIGH-1 / HIGH-2.
3. **Database migration files (`migrations/*_llm_runtime*.sql`).** Not reviewed for schema-level constraints; recommend a follow-up to verify FK CASCADE behaviour on `llm_runtime_instances` and `llm_runtime_versions`.
4. **The `llm-runtime` crate's standalone CLI / `bin/llm-runtime.rs`.** Not reachable from the Axum server in this audit; only the parts used by the server (`BinaryDownloader`, `EngineType`, `binary::ensure_executable`) were reviewed.
5. **The `supervisor.rs` health-check loop** (`src-app/llm-runtime/src/supervisor.rs`). Not wired into the server's `LocalDeployment` (server uses its own `processes: HashMap<Uuid, ProcessInfo>` and per-call status checks); only the underlying engine `start_with_binary` path is exercised. The supervisor's auto-restart logic was scanned but no findings raised here.
6. **The standalone llm-runtime engine HTTP server attack surface** (llama.cpp, mistral.rs internals). These are upstream OSS projects; their bugs are filed upstream. F-04 is a configuration issue on *our* side.
7. **GPU resource enumeration / scheduling.** Not present in the audited module.
8. **The `RuntimeVersion` repo-side data model (`runtime_version/repository.rs`).** Reviewed for SQL injection (parametrised, safe); no findings. Not deep-audited for race conditions on `set_system_default`.
9. **Frontend code** under `src-app/ui/`. Out of scope per the standard `*/server/*` module audit boundary.
10. **`code_sandbox` module** — referenced only for comparison of integrity-verification patterns. Subject of a separate WSL2/sandbox audit (`.sec-audits/wsl2-*`).

---

## Remediation Priority

**Sprint 1 (must-fix before next release):**
- F-01: SHA-256 pin (table baked into the binary) before any extract. Promote to cosign-pinned over the next release cycle.
- F-02 + F-10: Replace `model.name` as `--model` value with `ModelStorage::get_model_path(...)`; add `^[A-Za-z0-9._-]{1,128}$` validator on `model.name`.
- F-03: `cmd.env_clear()` + minimal re-add of HOME/PATH; `cmd.kill_on_drop(true)`; `prctl(PR_SET_PDEATHSIG, SIGTERM)` via `pre_exec` on Linux.
- F-04: per-instance `--api-key` generated at spawn, propagated through `chat` module to outbound requests.

**Sprint 2 (defense-in-depth):**
- F-05: Reject non-Regular tar entries; drop `set_unpack_xattrs(true)`; verify canonical destination.
- F-06: Add `.timeout(...)`, `.connect_timeout(...)`, size cap to download client.
- F-07: Per-user instance quota; cgroup v2 / rlimits on engine spawn.
- F-08: Bounded ring-buffer for engine logs; per-line cap; abort handles tracked.
- F-09: `.https_only(true)`, `.redirect(Policy::limited(3))`.

**Sprint 3 (cleanup):**
- F-11: Remove dead `binary_path: Option<String>` field from `DeploymentConfig::Local`.
- F-12: Canonical-prefix check before `remove_dir_all` in `delete_version`.
- F-13: Audit which permission tier sees `pid`/`port` (post-F-04 this becomes Info).
- F-14: When wiring up `detect_gpu_backend`, pin tool paths via `which` at startup.

---

## Files Reviewed

Server-side (in scope):
- `src-app/server/src/modules/llm_local_runtime/mod.rs`
- `src-app/server/src/modules/llm_local_runtime/binary_manager.rs`
- `src-app/server/src/modules/llm_local_runtime/handlers.rs`
- `src-app/server/src/modules/llm_local_runtime/models.rs`
- `src-app/server/src/modules/llm_local_runtime/permissions.rs`
- `src-app/server/src/modules/llm_local_runtime/repository.rs`
- `src-app/server/src/modules/llm_local_runtime/routes.rs`
- `src-app/server/src/modules/llm_local_runtime/events.rs`
- `src-app/server/src/modules/llm_local_runtime/deployment/mod.rs`
- `src-app/server/src/modules/llm_local_runtime/deployment/local.rs`
- `src-app/server/src/modules/llm_local_runtime/deployment/manager.rs`
- `src-app/server/src/modules/llm_local_runtime/runtime_version/mod.rs`
- `src-app/server/src/modules/llm_local_runtime/runtime_version/handlers.rs`
- `src-app/server/src/modules/llm_local_runtime/runtime_version/models.rs`
- `src-app/server/src/modules/llm_local_runtime/runtime_version/repository.rs`
- `src-app/server/src/modules/llm_local_runtime/utils/mod.rs`
- `src-app/server/src/modules/llm_local_runtime/utils/gpu_detect.rs`

Dependency surface (sampled for inbound calls from the module):
- `src-app/llm-runtime/src/binary_download.rs`
- `src-app/llm-runtime/src/binary.rs`
- `src-app/llm-runtime/src/engine/llamacpp.rs` (only `start_with_binary` referenced)
- `src-app/llm-runtime/src/runtime.rs`
- `src-app/llm-runtime/src/supervisor.rs`
- `src-app/llm-runtime/Cargo.toml`

Cross-references:
- `src-app/server/src/modules/llm_model/types.rs` (CreateLlmModelRequest)
- `src-app/server/src/modules/llm_model/models.rs` (LlmModel.name field)
- `src-app/server/src/modules/llm_model/utils.rs` (validate_create_request)
- `src-app/server/src/modules/llm_model/storage.rs` (the path API that *should* be used)
- `src-app/server/src/modules/permissions/extractors.rs` (RequirePermissions semantics)
- `.sec-audits/04-llm-modules-audit.md` (prior LLM-modules audit, sampled for context — no overlap with this audit's findings)
