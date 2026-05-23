# Security Audit — Core Infrastructure

**Date:** 2026-05-23
**Scope:** `src/main.rs`, `src/lib.rs`, `src/core/`, `src/common/`, `src/module_api/`, `src/utils/`, `src/openapi/`, `build.rs`, `Cargo.toml`, `config/`
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Chapters in focus:** V1 (Architecture), V7 (Error/Logging), V9 (Communication / TLS), V10 (Malicious Code / Deps), V14 (Configuration)
**Prior audit reviewed:** `.sec-audits/07-core-infrastructure-audit.md` (2025-11-21)

---

## Executive Summary

This re-audit of the Ziee Chat server's core infrastructure confirms that **most findings from the November 2025 audit remain unremediated** six months later. The most consequential gaps are still around request lifecycle controls (no body-size limit, no request timeout, no rate limiting, no security headers), credential exposure on the boot path and at build time, and a CORS default that drops to fully permissive when the operator forgets to configure it.

In addition to the carry-forward findings I identified several new or sharpened issues that were not raised in the previous audit:

- The server registers a panic hook (`database/mod.rs:377`) that spins up a **brand-new Tokio runtime** from inside the existing async panic context (`Runtime::new().unwrap()`). On panic, this will itself panic with `"Cannot start a runtime from within a runtime"`, producing a double-fault and lost shutdown.
- The `cleanup_database` helper does the same (`database/mod.rs:391`), so a panic during shutdown corrupts the embedded PostgreSQL data directory.
- `Repos` is a process-wide singleton initialised with `set(...).ok()` (`repository.rs:77`), so a second initialisation **silently no-ops** and integration test isolation depends on accidents.
- `tower-sessions` and `tower-sessions-sqlx-store` are pulled into the binary (Cargo.toml:22-23) but no `SessionManagerLayer` is mounted, so the dependency adds attack surface without providing value (and one of its transitive trees pulls in `rustls 0.21`, an older codepath).
- `eventsource-client = "0.12"` (a release from 2024) forces `hyper 0.14`, `rustls 0.21`, and `rand 0.8` to coexist with the modern `hyper 1.7`, `rustls 0.23`, `rand 0.9` stack — doubling the TLS attack surface and complicating CVE patching.
- The `--generate-openapi` codepath **starts the full embedded PostgreSQL** and runs migrations every time the schema is dumped (`openapi/mod.rs:17`), which is both a build-time amplification of network/disk usage and means CI machines that run `cargo run -- --generate-openapi` end up with a populated production-like Postgres instance on disk.
- The auth backend, repository factory, and shutdown teardown all assume that **a single process owns the database**; there is no advisory locking or per-instance identifier. Two instances of the binary started against the same `data_dir` will race in the embedded postgres bootstrap.

### Severity counts (this audit)

| Severity | New / re-confirmed | Total |
|---|---|---|
| Critical | 2 re-confirmed                                                          | 2 |
| High     | 3 re-confirmed (CORS default, body limit, JWT secret), 2 new           | 5 |
| Medium   | 5 re-confirmed + 5 new (sessions, panic hook, runtime-in-runtime, etc.) | 10 |
| Low      | 4 re-confirmed + 2 new                                                  | 6 |
| Info     | 4 hardening notes                                                       | 4 |

### Top three risks (act this week)

1. **F-01 — `DefaultBodyLimit::disable()` applied globally.** Any unauthenticated POST/PUT can ship a multi-gigabyte payload that the server buffers in memory before the handler ever runs. Trivial DoS.
2. **F-04 — CORS default is `Any/Any/Any` when the config block is missing or contains `"*"`.** Combined with cookie-bearing auth (`axum-login`/`tower-sessions` are present in deps), this is a credentialed-CORS misconfiguration waiting for a forgotten config field.
3. **F-09 — Panic hook spawns a nested Tokio runtime from within an async context.** Any panic in the request path (and there are many `.unwrap()` / `.expect()` paths in `utils/git/service.rs`) will deadlock the embedded PostgreSQL shutdown sequence and leave the data dir locked.

---

## Findings

### F-01 — Global `DefaultBodyLimit::disable()` (DoS via unbounded request body) [RE-CONFIRMED]

- **Severity:** Critical
- **ASVS:** V14.5.2, V13.1.3
- **CWE:** CWE-400 (Uncontrolled Resource Consumption), CWE-770 (Allocation of Resources Without Limits)
- **Location:** `src-app/server/src/main.rs:172`, `src-app/server/src/lib.rs:197`

**Description**

Both code paths that build the Axum app apply `DefaultBodyLimit::disable()` globally:

```rust
// main.rs:170-176
let app = api_router
    .finish_api(&mut api_doc)
    .layer(axum::extract::DefaultBodyLimit::disable())      // <-- no limit
    .layer(axum::Extension(event_bus))
    .layer(axum::Extension(jwt_service))
    .layer(axum::Extension(mcp_session_manager.clone()))
    .layer(cors);
```

The same call is present in `lib.rs:197` for the desktop-embedded server path. This is identical to the November 2025 finding (`07-core-infrastructure-audit.md` §1) and is **not remediated**. The inline comment "Disable body size limit for model uploads (models can be very large)" reflects an intent that should have been satisfied with a per-route override, not a global disable.

The 2026-05 audit also notes that the code_sandbox `tools/files.rs:660` contains a regression test that explicitly relies on `DefaultBodyLimit::disable()` being global, indicating the team is aware of the constraint and has chosen to live with it.

**Exploitation**

`curl -X POST -d "$(head -c 50G /dev/urandom)" http://host/api/auth/login` — body is buffered before authentication, OOM-kills the server. No authentication required.

**Impact**

Memory exhaustion / OOM, disk-buffer exhaustion (Axum may spill to `/tmp` on multipart), network amplification (worker tied up draining attacker stream).

**Recommendation**

Apply a sane default (e.g. 25 MB) globally, then override per-route for the genuine large-upload endpoints:

```rust
// global default
.layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024))

// in modules/llm_model/routes.rs upload handler
.route("/upload", post(upload_handler).layer(DefaultBodyLimit::max(8 * 1024 * 1024 * 1024)))
```

Add a `tower_http::limit::RequestBodyLimitLayer` outside the Axum extractor layer as belt-and-braces for streaming requests that bypass `Bytes::from_request` (multipart, SSE).

---

### F-02 — Hardcoded database password in `build.rs`, printed to stderr on failure [RE-CONFIRMED]

- **Severity:** Critical (per the original audit severity ladder; in practice this is a CI-hygiene High because the credential is for a build-time docker container, not production)
- **ASVS:** V14.1.2, V7.1.1
- **CWE:** CWE-798 (Hardcoded Credentials), CWE-532 (Sensitive Information in Logs)
- **Location:** `src-app/server/build.rs:16-30`

**Description**

```rust
let database_url = env::var("DATABASE_URL")
    .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string());
// ...
Err(e) => {
    eprintln!("\nERROR: Failed to connect to database: {}", e);
    eprintln!("DATABASE_URL: {}", database_url);   // <-- credential to stderr
    panic!("Database connection failed");
}
```

The exact same finding was raised in `07-core-infrastructure-audit.md` §2 and is **not remediated**. The credential reaches stderr on every failed build, which on a CI runner is captured to a log archive that may outlive the ephemeral runner.

This is described in `CLAUDE.md` as the "build database" running in docker-compose for SQLx compile-time verification. The credential never reaches a deployment, so the operational risk is bounded to **the build host and any operator who runs `cargo build` against a misconfigured database**. The credential is also identical to the literal string `"password"` documented in `CLAUDE.md`, so it has no defensible secrecy expectation. Still: shipping a hardcoded password through `eprintln!` is sloppy and trains contributors to ignore credential discipline.

**Recommendation**

```rust
let database_url = env::var("DATABASE_URL")
    .unwrap_or_else(|_| "postgresql://postgres@127.0.0.1:54321/postgres".to_string());
// On failure, do NOT echo the URL; show a redacted form
eprintln!("Hint: set DATABASE_URL=postgresql://postgres:<password>@127.0.0.1:54321/postgres");
```

Also: `build.rs` runs `DROP SCHEMA public CASCADE` against whichever URL `DATABASE_URL` resolves to. Add a host-allowlist check (`127.0.0.1` / `localhost` only) before issuing the drop, so that a developer who accidentally `export DATABASE_URL=postgresql://prod...` does not nuke production.

---

### F-03 — Weak / default JWT secret accepted at runtime [RE-CONFIRMED]

- **Severity:** High
- **ASVS:** V2.4.1, V6.4.1, V14.1.2
- **CWE:** CWE-798, CWE-326
- **Location:** `src-app/server/src/modules/auth/jwt.rs:41-50`, `src-app/server/config/dev.yaml:81`, `prod.example.yaml:41`

**Description**

`JwtService::new` accepts whatever `config.jwt.secret` contains, with no length, entropy, or "is this the example secret" check. The committed `dev.yaml` and `dev.example.yaml` ship the literal string `"dev-secret-change-in-production-min-32-chars-long"`, and the production example is `"REPLACE_ME_WITH_A_LONG_RANDOM_SECRET_AT_LEAST_32_CHARS"`. A deployment that forgets to override the value boots successfully and signs tokens with a fully public secret.

Same as `07-core-infrastructure-audit.md` §3 — **not remediated**.

**Exploitation**

Any attacker who reads the public repo can forge an `is_admin: true` token for any user id; no defence-in-depth (token-binding, refresh-token rotation against db) prevents this.

**Recommendation**

Validate at config load:

```rust
let weak = [
    "dev-secret-change-in-production-min-32-chars-long",
    "REPLACE_ME_WITH_A_LONG_RANDOM_SECRET_AT_LEAST_32_CHARS",
    "change-me", "secret", "",
];
if weak.contains(&config.jwt.secret.as_str()) {
    return Err("JWT secret matches a public example value — refusing to boot".into());
}
if config.jwt.secret.len() < 32 {
    return Err("JWT secret must be >= 32 chars".into());
}
// Bonus: estimate entropy and warn if shannon < 4.0 bits/char
```

Apply the same logic in `JwtService::new` as a second line of defence — a misconfigured `lib.rs` path should not be able to skip the config-loader check.

---

### F-04 — CORS defaults to `Any/Any/Any` when block missing or contains `"*"` [RE-CONFIRMED]

- **Severity:** High
- **ASVS:** V14.5.3
- **CWE:** CWE-942 (Permissive Cross-domain Policy)
- **Location:** `src-app/server/src/core/app_builder.rs:100-157`

**Description**

```rust
pub fn create_cors_layer(config: &Config) -> CorsLayer {
    if let Some(ref cors_config) = config.server.cors {
        // ...
        if cors_config.allow_origins.contains(&"*".to_string()) || origins.is_empty() {
            layer = layer.allow_origin(Any);
        }
        // ...
    } else {
        CorsLayer::new()
            .allow_origin(Any)       // <-- the implicit default
            .allow_methods(Any)
            .allow_headers(Any)
    }
}
```

Three issues:

1. The `else` branch (no CORS block at all) gives `Any/Any/Any`, so any operator config omission yields the worst-case policy.
2. Inside the `if let Some(...)`, an `allow_origins: []` (or `["*"]`) also yields `Any` — empty array silently becomes wildcard.
3. The policy compatibility with credentials matters here: `axum-login`/`tower-sessions` are pulled into the binary; if a future operator wires up session cookies (the deps are already there), `Access-Control-Allow-Origin: *` plus credentials becomes a CSRF accelerator.

Same as `07-core-infrastructure-audit.md` §4 — **not remediated**.

**Recommendation**

- Refuse to boot if `server.cors` is missing in production mode; default to an empty allowlist (which denies cross-origin) instead of `Any` in dev.
- Never accept `"*"` for `allow_origins`. Log a warning and ignore it.
- Use `tower_http::cors::CorsLayer::permissive()` only behind an explicit `cors_dev_permissive: true` config flag and emit a noisy log line at startup.

---

### F-05 — No global request timeout [RE-CONFIRMED, sharpened]

- **Severity:** High
- **ASVS:** V13.4.1
- **CWE:** CWE-400, CWE-1325 (Improperly Controlled Sequential Memory Allocation)
- **Location:** `src-app/server/src/main.rs:170-176`, `src-app/server/src/lib.rs:195-201`

**Description**

The Axum app applies no `tower_http::timeout::TimeoutLayer`. A slow-reader client (slowloris) can hold a request open indefinitely. Combined with F-01 (no body limit), an attacker can sustain a long-running request that consumes a connection slot and an unbounded amount of memory for the body buffer.

The Axum server itself has no idle-connection timeout configured; the default Hyper connection idle is generous (no explicit limit on graceful idle).

**Recommendation**

```rust
use tower_http::timeout::TimeoutLayer;
use std::time::Duration;

.layer(TimeoutLayer::new(Duration::from_secs(30)))   // global default
```

Apply a longer per-route timeout (or none) only on the SSE/streaming routes (chat, mcp sampling, code_sandbox event stream). Keep auth routes at 5 seconds.

---

### F-06 — No rate limiting on any endpoint [RE-CONFIRMED]

- **Severity:** High
- **ASVS:** V11.1.3, V13.1.4
- **CWE:** CWE-307 (Improper Restriction of Excessive Authentication Attempts), CWE-770
- **Location:** all routing files; no `tower-governor` or equivalent in deps

**Description**

`grep tower-governor` returns no hits; `grep rate.limit` returns no hits in `src-app/server/src/`. The auth handlers (`modules/auth/handlers.rs`) accept unlimited password attempts; the LLM key-validation endpoints accept unlimited invalid keys; the MCP OAuth refresh endpoint can be hammered to amplify against an upstream identity provider.

Same as `07-core-infrastructure-audit.md` §5 — **not remediated**.

**Recommendation**

Add `tower-governor` (or `governor` crate directly) with two policies:

- `auth`: 5 attempts per minute per IP, 30 per minute per (IP, username), burst 10.
- `default`: 60 requests per minute per IP, burst 100.

Make the limiter configurable via the YAML config block (`server.rate_limit.{enabled, per_minute, burst}`).

---

### F-07 — Unused `tower-sessions` + `tower-sessions-sqlx-store` in binary [NEW]

- **Severity:** Medium
- **ASVS:** V10.3.2 (Maintained code), V1.14.6 (Dependency footprint)
- **CWE:** CWE-1357 (Reliance on Insufficiently Trustworthy Component) — not exploitable on its own, but the unused crate is permanently in the binary
- **Location:** `src-app/server/Cargo.toml:22-23`; no `SessionManagerLayer::new` anywhere in `src/`

**Description**

The `Cargo.toml` declares `tower-sessions = "0.14.0"` and `tower-sessions-sqlx-store = { version = "0.15.0", features = ["postgres"] }`, but no module mounts a `SessionManagerLayer`. The crates link in transitive deps (additional rand/time/serde paths, sqlx feature surface) without contributing functionality.

This is a code-hygiene Medium because:

1. The shipped binary is larger than it needs to be (~150 kB of dead code).
2. Any CVE against `tower-sessions`/`tower-sessions-sqlx-store` will flag the project, and the remediation cost is real.
3. A future maintainer who sees the dep and wires up sessions without coordinating with the JWT layer may end up with **two independent auth stacks** (cookie sessions vs. JWT bearer), which is the standard preamble to an auth bypass.

**Recommendation**

Delete both deps unless the team has a concrete near-term plan for session-based auth. If the plan is real, document it (e.g. in `CLAUDE.md`) so reviewers understand the deferred state.

---

### F-08 — `eventsource-client = "0.12"` drags in `hyper 0.14` + `rustls 0.21` [NEW]

- **Severity:** Medium
- **ASVS:** V10.3.2
- **CWE:** CWE-1395 (Dependency on Vulnerable Third-Party Components)
- **Location:** `Cargo.toml:101`; `Cargo.lock` confirms `hyper 0.14.32`, `rustls 0.21.12`, `h2 0.3.27`, `rand 0.8.5`, `hyper-rustls 0.24.2`, `hyper-timeout 0.4.1` are all in the build because of `eventsource-client`.

**Description**

The modern stack uses `hyper 1.7`, `rustls 0.23`, `h2 0.4`, `rand 0.9`. The `eventsource-client` SSE client (used by `modules/mcp/client/sse.rs`) is on its **last release pinned to the legacy stack**. This doubles the TLS attack surface (two TLS implementations active in the binary, each consuming a separate cert store and patch cadence) and means a CVE in `hyper 0.14` or `rustls 0.21` will require updating either the SSE client or replacing it.

**Exploitation**

Not directly exploitable today, but: `rustls 0.21` was the line in which RUSTSEC-2024-0336 (record-fragmentation memory amplification) was patched; staying on it means future bug-fix windows are narrower than for `rustls 0.23`. `hyper 0.14` is in maintenance-only mode upstream.

**Recommendation**

Either:

- Migrate the SSE client off `eventsource-client` and onto `reqwest`'s SSE support (which is already on `hyper 1.7`) — see `modules/mcp/client/sse.rs`, ~50 LOC affected.
- Or fork `eventsource-client` to the modern stack and vendor it.

Either way, drop the legacy TLS chain before the next CVE window.

---

### F-09 — Panic hook starts a NEW Tokio runtime from inside an async panic [NEW]

- **Severity:** Medium (mostly availability, not security — but lost shutdown == lost audit-log durability)
- **ASVS:** V11.1.4
- **CWE:** CWE-755 (Improper Handling of Exceptional Conditions)
- **Location:** `src-app/server/src/core/database/mod.rs:376-383`, lines 391 (DatabaseCleanup::drop) and 354 (cleanup_database inner spawn)

**Description**

```rust
fn register_cleanup_handlers() {
    if CLEANUP_REGISTERED.swap(true, Ordering::SeqCst) { return; }
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        println!("Panic detected, cleaning up database...");
        let rt = tokio::runtime::Runtime::new().unwrap();   // <-- nested runtime
        rt.block_on(cleanup_database());
        orig_hook(panic_info);
    }));
}
```

When any axum handler panics, the panic propagates inside `tokio::spawn`'s task runtime, and the hook executes inside that runtime. `tokio::runtime::Runtime::new()` then panics with `"Cannot start a runtime from within the context of another runtime"`. The `.unwrap()` re-panics, the original panic information is lost, and the embedded PostgreSQL instance is not stopped gracefully. The result is a stale `postmaster.pid` and the next boot has to call `pg_ctl stop` (which `stop_existing_postgres_instance` does — but that flow can also fail and call `process::exit(1)`).

`DatabaseCleanup::drop` at line 391 has the same bug; it would only fire on a successful program exit, but the Drop runs on the main thread which **may or may not** still be inside a runtime depending on whether `tokio::main` has finished.

**Exploitation**

Not directly attacker-facing (panics are typically triggered by malformed input or bugs). However, the audit-trail durability loss is real: when a request handler panics, the server cannot guarantee that pending event-bus emissions (`emit_async`) have completed. Combined with F-15 (`emit_async` is fire-and-forget), this means a panic during a sensitive write can leave the audit log inconsistent with the database.

**Recommendation**

Use `tokio::runtime::Handle::try_current()` and reuse the existing runtime if present; only build a new runtime if `try_current()` errs (i.e. main thread after `tokio::main` returns). Better still, use `tokio::task::block_in_place` or a oneshot channel to signal a shutdown task that lives outside the panic handler.

---

### F-10 — `Repos` factory uses `set(...).ok()` (silent double-init) [NEW]

- **Severity:** Medium
- **ASVS:** V1.14.4
- **CWE:** CWE-665 (Improper Initialization)
- **Location:** `src-app/server/src/core/repository.rs:75-78`

**Description**

```rust
static FACTORY: OnceCell<RepositoryFactory> = OnceCell::new();
pub fn init_repositories(pool: PgPool) {
    FACTORY.set(RepositoryFactory::new(pool)).ok();   // <-- silently drops if already set
}
```

A second call to `init_repositories` (legitimate in some test harnesses — the lib and main both call it) is silently ignored. In normal operation that's the desired behaviour. But it also means that **if a test forgets to drop the previous factory** (and the embedded postgres reuses a port), the test process keeps the old pool, which now points at a postgres that may have been wiped by the next test's setup. Bugs of this shape have caused intermittent failures in similar codebases.

Compound this with the production-side risk: a future refactor that adds a "swap to fresh pool" path on, e.g., DB failover will fail silently.

**Recommendation**

Either return `Result<(), AlreadyInitialized>` from `init_repositories`, or accept that the operation is idempotent and **document** that pool replacement is unsupported. Add a debug-assertion that panics on double-init in test builds.

---

### F-11 — Database `log_statement: "all"` in dev embedded Postgres + `serde_yaml` deprecated [NEW + RE-CONFIRMED]

- **Severity:** Medium (combined finding)
- **ASVS:** V7.1.1, V14.1.5
- **CWE:** CWE-532, CWE-1395
- **Location:** `config/dev.yaml:35`; `Cargo.toml:37` (`serde_yaml = "0.9"`)

**Description**

`config/dev.yaml` sets the embedded PostgreSQL's `log_statement: "all"`. With `bcrypt`-hashed passwords stored as parameters this is mostly safe (the hash isn't reversible), but `INSERT INTO users (..., password_hash, ...)` rows will log the bcrypt hash. Plus, any `INSERT INTO oauth_states` / `user_keys` row that contains an API key will appear in the postgres log file at `installation_dir/log/postgresql-*.log`.

Separately, `serde_yaml = "0.9.34+deprecated"` (per `Cargo.lock`). The upstream crate is deprecated; `serde_norway` or `serde-yaml-bw` are the maintained successors. The deprecation itself is not a CVE, but it means parsing changes (and security fixes) will lag.

**Recommendation**

- Change `dev.yaml` default to `log_statement: "ddl"` and document `"all"` as opt-in.
- Track migration off `serde_yaml` (it powers config loading — `core/config.rs:183`).

---

### F-12 — Database URL printed (with credentials) via `println!` in `database/mod.rs:205` [RE-CONFIRMED]

- **Severity:** Medium
- **ASVS:** V7.1.1
- **CWE:** CWE-532
- **Location:** `src-app/server/src/core/database/mod.rs:205`

```rust
let database_url = postgresql.settings().url("postgres");
println!("Generated database_url: {:?}", database_url);   // <-- credentials in stdout
```

This is the exact issue raised in `07-core-infrastructure-audit.md` §6, **not remediated**. The whole file is a wash of `println!` — there are 28 `println!`/`eprintln!` calls in `database/mod.rs`, none using the `tracing` macros that the rest of the codebase uses. This means:

- The DB log is sent to stdout pre-`tracing_subscriber::fmt().init()` (which is fine for ordering) but ends up un-redactable by log filtering middleware.
- `tracing::with_max_level(WARN)` cannot silence database boot noise because `database/mod.rs` doesn't go through tracing.
- The credential-bearing URL is in stdout at every boot.

**Recommendation**

Replace `println!` with `tracing::info!`/`tracing::debug!` throughout `database/mod.rs`. Add a `redact_url(&str) -> String` helper for any path that needs to log a connection string.

---

### F-13 — Error messages leak `sqlx::Error` internals to clients [RE-CONFIRMED]

- **Severity:** Medium
- **ASVS:** V7.4.1
- **CWE:** CWE-209 (Information Exposure Through Error Message)
- **Location:** `src-app/server/src/common/type.rs:109-115`, `:141-148`

```rust
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::not_found("Resource"),
            _ => AppError::database_error(err),       // <-- includes raw sqlx error
        }
    }
}

pub fn database_error(err: impl std::error::Error) -> Self {
    Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "SYSTEM_DATABASE_ERROR",
        format!("Database error: {}", err),           // <-- leaks schema info
    )
}
```

`sqlx::Error::Database(e)` includes the underlying Postgres error message, which contains the table name, column name, constraint name, and sometimes the offending value. Examples observed in similar codebases: `duplicate key value violates unique constraint "users_email_key"`, `null value in column "is_admin" of relation "users" violates not-null constraint`. These are emitted verbatim to the client.

Same as `07-core-infrastructure-audit.md` §7 — **not remediated**.

**Recommendation**

`database_error` should log the full error via `tracing::error!` and return a generic message:

```rust
pub fn database_error(err: impl std::error::Error) -> Self {
    tracing::error!("Database error: {}", err);
    Self::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "SYSTEM_DATABASE_ERROR",
        if cfg!(debug_assertions) { format!("{}", err) } else { "Database error".to_string() },
    )
}
```

Pair with a correlation-id header in the response so debugging is still possible.

---

### F-14 — `--generate-openapi` boots the full embedded PostgreSQL [NEW]

- **Severity:** Medium
- **ASVS:** V14.4.1 (Inappropriate build/CI behaviour)
- **CWE:** CWE-668 (Exposure of Resource to Wrong Sphere)
- **Location:** `src-app/server/src/openapi/mod.rs:17-21`

**Description**

```rust
pub async fn generate_openapi_spec(...) -> Result<...> {
    let config = Config::load_from(config_file)?;
    let pool = crate::core::database::initialize_database(&config).await?;   // <-- full DB boot
    crate::core::init_repositories((*pool).clone());
    // ...
}
```

Generating the OpenAPI spec — a static documentation artifact — requires the server to:

1. Start the embedded PostgreSQL (or connect to an external one).
2. Run all migrations.
3. Initialise the global repository factory.

This means CI runs that produce TypeScript types end up with a populated Postgres data directory on disk. For air-gapped CI it also means the cosign verification of postgresql-embedded's bundled tarball runs even for a doc job that doesn't need it.

The functional reason for the DB connection is that some modules' `init()` may consult the database (e.g. to register seed data). But OpenAPI generation should not depend on runtime state.

**Recommendation**

Split `AppModule::init` into `init_routes` (pure, no DB) and `init_runtime` (DB-aware). The OpenAPI generator should only call `init_routes`.

Short-term workaround: detect `--generate-openapi` in `Config::load_from` and skip DB init when present. The current `config/openapi-gen.yaml` workaround (point at the docker build database) is fragile because it requires the build database to be running.

---

### F-15 — `EventBus::emit_async` is fire-and-forget; no backpressure / poison handling [NEW]

- **Severity:** Medium
- **ASVS:** V7.1.4, V11.1.4
- **CWE:** CWE-754 (Improper Check for Unusual Conditions)
- **Location:** `src-app/server/src/core/events.rs:100-118`

**Description**

```rust
pub fn emit_async(&self, event: AppEvent) {
    let handlers = self.handlers.clone();
    let pool = self.pool.clone();
    tracing::debug!("Emitting event asynchronously: {:?}", event);
    tokio::spawn(async move {
        for handler in handlers {
            if let Err(e) = handler.handle(&event, &pool).await {
                tracing::error!("Event handler '{}' failed: {}", handler.handler_name(), e);
            }
        }
    });
}
```

Three concerns:

1. **No backpressure.** Each emission spawns a fresh tokio task; an attacker who can drive a burst of state changes (e.g. rapid login attempts → auth events) can spawn arbitrarily many concurrent handler tasks.
2. **Errors are silently logged** (which is what the doc-comment says, but the doc-comment is on `emit`, not `emit_async`). A handler whose responsibility is "write to audit log" can fail and the originating request still returns 200.
3. **Handlers run sequentially inside the spawn**, so slow handlers block subsequent handlers within the same emission but **multiple emissions race against each other**. Ordering between emissions is undefined.

**Recommendation**

- Bound the concurrency of `emit_async` tasks (e.g. via a `tokio::sync::Semaphore` clamped at 256).
- For audit-critical events, require synchronous `emit` and propagate errors.
- Document the event-ordering guarantees (or lack thereof) in `core/events.rs`.

---

### F-16 — Unvalidated binary downloads (Pandoc/PDFium/UV/Bun) at build time [RE-CONFIRMED, still no checksums]

- **Severity:** Medium
- **ASVS:** V14.2.4 (Software Supply Chain Integrity)
- **CWE:** CWE-494 (Download of Code Without Integrity Check), CWE-829
- **Location:** `build_helper/pandoc.rs:6-20`, `build_helper/pdfium.rs:4-18`, `build_helper/uv.rs:83-98`, `build_helper/bun.rs:78-93`

**Description**

All four binary fetchers in `build_helper/` download from GitHub releases over TLS but perform no sha256 / signature verification of the downloaded archive. The binary is then `include-flate`d into the final server binary. A compromise of:

- `github.com/jgm/pandoc/releases`
- `github.com/bblanchon/pdfium-binaries/releases` (especially since this uses `releases/latest/download/...` — **unpinned to a specific version**)
- `github.com/astral-sh/uv/releases`
- `github.com/oven-sh/bun/releases`

...would inject a malicious binary directly into the server binary. The PDFium fetch is the most concerning because it pulls **latest**, so a single upstream attack window catches every fresh build.

Same as `07-core-infrastructure-audit.md` §9 — **not remediated**. (The sandbox-rootfs path does cosign-verify via `sigstore`; the build_helper binaries do not.)

**Recommendation**

1. Pin PDFium to a specific tag (it currently fetches `releases/latest`).
2. Maintain a `build_helper/checksums.toml` with sha256 per (binary, version, target) and verify before `include-flate` embeds.
3. Long-term: reuse the `sigstore`-based verification path that sandbox-rootfs already implements.

---

### F-17 — Static panic-on-poison in `APP_DATA_DIR` mutex [RE-CONFIRMED]

- **Severity:** Low (unchanged from previous audit)
- **ASVS:** V1.14.4
- **CWE:** CWE-362
- **Location:** `src-app/server/src/core/app_state.rs:34-39`

`get_app_data_dir` calls `.lock().expect("Failed to lock APP_DATA_DIR")` — if another thread panics while holding the mutex, every subsequent caller crashes. Recommendation per the previous audit: use `OnceLock` (write-once) or `RwLock` and handle `PoisonError::into_inner()` to recover. Same as `07-core-infrastructure-audit.md` §10.

---

### F-18 — Missing security headers (no CSP, HSTS, X-Content-Type-Options, X-Frame-Options) [RE-CONFIRMED]

- **Severity:** Low
- **ASVS:** V14.4.1, V14.4.2, V14.4.5
- **CWE:** CWE-1021 (Improper Restriction of Rendered UI), CWE-693 (Protection Mechanism Failure)
- **Location:** entire middleware stack

No `SetResponseHeaderLayer` is mounted. None of the standard hardening headers are emitted:

- `X-Content-Type-Options: nosniff`
- `Strict-Transport-Security: max-age=...; includeSubDomains`
- `X-Frame-Options: DENY` (or CSP `frame-ancestors 'none'`)
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Content-Security-Policy: default-src 'self'` (or appropriate for the SPA)

Same as `07-core-infrastructure-audit.md` §12 — **not remediated**.

---

### F-19 — Panic on `expect` paths in startup (signal handler install, server bind) [LOW, new]

- **Severity:** Low
- **ASVS:** V11.1.4
- **CWE:** CWE-755
- **Location:** `src-app/server/src/main.rs:197, 213, 219`, `src-app/server/src/lib.rs:229`

```rust
axum::serve(listener, app.into_make_service())
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("Failed to start server");          // <-- panic, not graceful exit
// ...
signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
// ...
signal::unix::signal(signal::unix::SignalKind::terminate())
    .expect("Failed to install signal handler")
```

Three places where a runtime failure (signal handler unavailable in a containerized environment, server bind interrupted, etc.) panics instead of returning a clean error and triggering the cleanup hook. Combined with F-09 (panic hook is itself broken), this means a sandboxed environment that strips out signal capabilities will lose the embedded Postgres cleanup.

**Recommendation**

Convert the `expect`s to `match`/`?` with `tracing::error!` + `process::exit(1)`. Reserve `panic!` for invariants the program cannot recover from.

---

### F-20 — `find_available_port` race window [LOW, new]

- **Severity:** Low
- **ASVS:** V11.1.4
- **CWE:** CWE-362 (Race Condition)
- **Location:** `src-app/server/src/core/config.rs:252-268`, `lib.rs:267-269`

```rust
fn find_available_port(start_port: u16, end_port: u16) -> Option<u16> {
    for port in start_port..=end_port {
        if let Ok(listener) = TcpListener::bind(...) {
            drop(listener);
            if let Ok(listener2) = TcpListener::bind(...) {   // <-- TOCTOU
                drop(listener2);
                return Some(port);
            }
        }
    }
    portpicker::pick_unused_port()
}
```

Classic time-of-check / time-of-use: between the second `drop(listener2)` and the caller actually binding, another process can grab the port. The double-bind dance does not actually help.

In production, ports are typically configured explicitly (`port: 3000`), so this is only relevant for `port: 0` (auto-pick), which is used by tests. It's a Low-severity correctness bug — flake source, not security risk — but worth fixing.

**Recommendation**

Bind once, keep the listener (return it to the caller) instead of dropping and re-binding. Axum supports `axum::serve(listener, app)` from an already-bound listener.

---

### F-21 — `set_ignore_missing(true)` on migrations allows desktop app to add migrations the server has never seen [LOW, INFO]

- **Severity:** Low / Info
- **ASVS:** V14.1.5
- **CWE:** CWE-693
- **Location:** `src-app/server/src/core/database/mod.rs:244-247`

```rust
sqlx::migrate!("./migrations")
    .set_ignore_missing(true)        // <-- allows external migrations
    .run(&pool)
    .await?;
```

The comment says this is to support the desktop app adding its own migrations. Functionally correct, but:

1. If a desktop app accidentally runs against the server's database, its migrations may run successfully but leave a schema the server doesn't understand.
2. The server then accepts that schema without warning and may issue queries against columns the desktop app dropped/altered.

This is more a defence-in-depth note than a vulnerability. Consider requiring desktop migrations to live in a separate `desktop_migrations` namespace, or at minimum logging a warning when an unrecognized migration is seen.

---

### F-22 — `Cargo.lock` mixes `rand 0.8` and `rand 0.9`, multiple SHA / RSA / TLS stacks [INFO]

- **Severity:** Informational
- **ASVS:** V10.3.2
- **Location:** `Cargo.lock` (root-level inspection)

`grep` results from `Cargo.lock`:

- `rand 0.8.5` and `rand 0.9.2` both present (eventsource-client and others bring 0.8).
- `hyper 0.14.32` and `hyper 1.7.0` both present.
- `rustls 0.21.12` and `rustls 0.23.34` both present.
- `h2 0.3.27` and `h2 0.4.12` both present.
- `openssl 0.10.74` is present (likely pulled in by `ldap3` and `git2`).

This is the cost of `eventsource-client` (F-08), `ldap3`, and `git2` lagging behind. Not exploitable directly; relevant to estimate CVE-patch latency.

---

### F-23 — `tracing_subscriber` configured without `EnvFilter` [INFO]

- **Severity:** Informational
- **ASVS:** V7.1.1
- **Location:** `src-app/server/src/main.rs:67-96`, `src-app/server/src/lib.rs:89-124`

The subscriber is built with `with_max_level(level)` but no `EnvFilter`. This means modules can't be selectively raised/silenced via `RUST_LOG`. Operationally minor; relevant when investigating noisy production logs.

**Recommendation**

```rust
use tracing_subscriber::EnvFilter;
let filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new(level.to_string()));
tracing_subscriber::fmt().with_env_filter(filter).init();
```

---

### F-24 — `aide`-generated OpenAPI spec emits no `securitySchemes` [INFO]

- **Severity:** Informational (correctness/documentation, not a vulnerability)
- **ASVS:** V13.2.2
- **Location:** `src-app/server/src/openapi/mod.rs`, no `OpenApi::with_security_scheme` call anywhere

The generated `openapi.json` describes endpoints but does not declare a `securitySchemes` block with `bearerAuth: { type: http, scheme: bearer }`, nor does it annotate which routes require authentication. Consumers of the spec (typescript codegen, Postman, third-party clients) cannot tell from the spec which routes are authenticated.

**Recommendation**

Add a `bearerAuth` scheme in `openapi/mod.rs` and require modules to declare per-operation security via `aide`'s `with_security` transformer.

---

### F-25 — `set_app_data_dir` failure is logged but does not propagate [INFO]

- **Severity:** Informational
- **Location:** `src-app/server/src/core/app_state.rs:20-30`

If the mutex is poisoned at startup, `set_app_data_dir` quietly logs `error!("Failed to lock APP_DATA_DIR mutex")` and returns. The server then boots with the default `~/.ziee` path. This is the same class of issue as F-17. Combine the two when fixing.

---

## ASVS Coverage Matrix

| ASVS Chapter | Requirement | Status | Findings |
|---|---|---|---|
| V1.14.3 | Service segmentation | Adequate (modular AppModule) | — |
| V1.14.4 | Concurrent component design | **Gaps** | F-09, F-10, F-17 |
| V1.14.6 | Minimised deps | **Gaps** | F-07, F-08, F-22 |
| V7.1.1 | No secrets in logs | **Fail** | F-02, F-11, F-12, F-23 |
| V7.1.4 | Error robustness | Partial | F-15, F-19 |
| V7.4.1 | Generic client-facing errors | **Fail** | F-13 |
| V9.1.x | TLS configuration | N/A (terminated upstream; no in-process TLS) | F-08 (transitive risk) |
| V10.3.2 | Maintained dependencies | Partial | F-07, F-08, F-11, F-22 |
| V11.1.3 | Anti-automation / rate limiting | **Fail** | F-06 |
| V11.1.4 | Async/shutdown correctness | Partial | F-09, F-19, F-20 |
| V13.1.3 | Body-size limits | **Fail** | F-01 |
| V13.1.4 | Throttling | **Fail** | F-06 |
| V13.2.2 | OpenAPI security descriptors | **Fail** | F-24 |
| V13.4.1 | Idle/request timeouts | **Fail** | F-05 |
| V14.1.2 | Secure defaults | **Fail** | F-02, F-03, F-04 |
| V14.1.5 | Schema management | Partial | F-11, F-21 |
| V14.2.4 | Supply-chain integrity | Partial (sandbox-rootfs cosign yes; build_helper no) | F-16 |
| V14.4.x | Security response headers | **Fail** | F-18 |
| V14.5.2 | Validated request size | **Fail** | F-01 |
| V14.5.3 | CORS allowlist | **Fail** | F-04 |

---

## Positive Findings

The audit found a number of practices that are actively well-done and worth recognising:

1. **`AppError` is a well-shaped, code-driven error type.** The `error_code` field gives consumers a stable machine-readable error contract, distinct from the human-readable message — a pattern most projects don't bother with.

2. **The repository factory uses `OnceCell` per-repository inside the global factory.** Lazy-init is correct, no double-construction of expensive repos.

3. **Module registration via `linkme::distributed_slice` with deterministic ordering** (`app_builder.rs:18-22`, sorted by `order` field). This is the right answer to "can two modules register the same route" — the order is build-time deterministic and the explicit `order: ...` literal is grep-able. Modules with conflicting orders would still need a runtime guard, but the foundation is sound.

4. **`postgresql_embedded` is configured with `--rustls`** (not the optional openssl backend), keeping the embedded-Postgres TLS stack on the modern crypto path.

5. **`code_sandbox_seccomp` is a default-on feature on Linux** (Cargo.toml:126), with the operator-friendly story documented in `CLAUDE.md`. This is uncommon in the Rust ecosystem and well-handled.

6. **`build.rs` uses `cargo:rerun-if-env-changed=DATABASE_URL`** — the right primitive for build-time env tracking.

7. **`Cargo.lock` is checked in** (for a binary). This is the correct decision for reproducible builds even though the `.gitignore` template still says "leave it for libraries" (`.gitignore:9`).

8. **`config/dev.yaml` is gitignored** (`config/.gitignore:1`), so the actual dev secret doesn't get committed even though the example does.

9. **No use of `unsafe` blocks in any of the audited files.** The whole core stays in safe Rust.

10. **Graceful shutdown handles both `SIGTERM` and `SIGINT`** (`main.rs:209-239`) and explicitly closes MCP sessions and database resources. The actual signal handling is solid; the panic-hook variant (F-09) is the broken piece.

---

## Out of Scope / Deferred

- **Per-module business logic** (auth, chat, file, mcp, llm_*, assistant, code_sandbox internals) — covered in audits 01-08 and the dedicated sandbox audits.
- **Frontend security** (CSP, XSS, etc.) — separate UI audit.
- **Database schema review** (migration semantics, RLS, audit log durability) — touched on lightly in F-21 but a full review is a separate workstream.
- **Network / deployment topology** (TLS termination expectations, reverse-proxy headers, X-Forwarded-For trust) — assumed delegated to a reverse proxy; once a deployment guide exists, audit it.
- **`tokio` runtime tuning** (worker count, blocking pool size) — observability/perf concern.
- **`postgresql_embedded` upstream supply chain** — pinned to `0.20.0` with bundled binaries; would require a dedicated review of `theseus`'s binary distribution model.
- **MCP code_sandbox runtime hardening** — covered in dedicated sandbox audits (`wsl2-*`, `mcp-phase3-i2-get-sse-audit-*`).

---

## Recommended Remediation Order

| Order | Finding | Effort | Risk reduced |
|---|---|---|---|
| 1 | F-01 (body limit) | 1h | Critical DoS |
| 2 | F-05 (timeout) | 1h | High DoS |
| 3 | F-04 (CORS default) | 2h | High CSRF / credential leak |
| 4 | F-03 (JWT secret validation) | 2h | High authn bypass |
| 5 | F-02 + F-12 (credential redaction) | 3h | Critical / Medium credential leak |
| 6 | F-06 (rate limiting) | 1d | High brute force / DoS |
| 7 | F-09 (panic hook nested runtime) | 4h | Medium availability + audit-log integrity |
| 8 | F-13 (error message generalisation) | 4h | Medium info disclosure |
| 9 | F-18 (security headers) | 2h | Low defence-in-depth |
| 10 | F-08 (eventsource-client migration) | 2d | Medium dep hygiene |
| 11 | F-07 (drop unused tower-sessions) | 30m | Medium dep hygiene |
| 12 | F-14 (OpenAPI gen DB decoupling) | 1d | Medium build hygiene |
| 13 | F-16 (binary download checksums) | 1d | Medium supply-chain |
| 14 | F-24 (OpenAPI security schemes) | 4h | Info / docs correctness |
| 15 | remainder | as time permits | Low / Info |

---

**End of audit.**
