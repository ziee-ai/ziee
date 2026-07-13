//! Config-as-code: a declarative "desired state" file the server reconciles
//! into its DB at boot, so a fresh deploy (TeamCity, `docker compose up`) comes
//! up fully configured with NO manual UI setup.
//!
//! Wiring: `main.rs` calls [`reconcile`] AFTER migrations + `init_repositories`
//! + `init_storage_key`, and BEFORE the server serves.
//!
//! **DEPLOY-ONLY, OPT-IN, DEFAULT OFF.** The reconciler does NOTHING unless the
//! deploy signal `ZIEE_APPLY_DESIRED_STATE=1` is set — no seeding, no enforce, no
//! MCP/admin/permission writes. The repo-checked `config/desired-state.yaml` is
//! version control, not a trigger: its presence (in the tree OR baked into the
//! image) never applies it. Only a deploy (TeamCity sets the flag on the deploy
//! configs) turns it on. This is what guarantees a local developer's
//! hand-configured models / MCP servers / admin / permissions are never touched,
//! enforced, or duplicated. When the flag is on, `ZIEE_DESIRED_STATE_FILE` says
//! WHICH file (defaulting to the image's `/etc/ziee/desired-state.yaml`).
//!
//! Contract (all of it deliberate):
//!
//! - **Secrets never inline.** `${VAR}` placeholders are resolved from process
//!   env at reconcile time, in the fields where they are meaningful: an MCP
//!   server's `url`, and the `password` of `admin` / `users`. A *secret* field
//!   (`password`) MUST be exactly one `${VAR}` placeholder — an inline literal
//!   is rejected. Resolved values are never logged; logs name the env VAR only.
//!   (Other fields — names, descriptions, emails — are taken verbatim.)
//! - **Idempotent.** Every entry is existence-checked against its natural key
//!   before writing (server → `(name, is_system)`; user → username/email; group →
//!   name). The ADMIN is separate: it comes from the environment, not the file,
//!   and is created ONLY on a database with NO account at all — so a redeploy can
//!   never overwrite it or revert a password the operator changed in the UI. Re-running on the next deploy
//!   creates nothing and clobbers nothing. The whole reconcile additionally runs
//!   under a Postgres ADVISORY LOCK, so two containers booting against one
//!   database (a rolling redeploy, `--scale`) serialize instead of racing the
//!   check-then-insert into duplicate rows.
//! - **Per-entry `mode`** (`mcp_servers`): `ensure` (default) creates when
//!   absent and otherwise leaves the row's fields alone — never clobbering an
//!   admin's later UI edit (the `seed_from_config_once` contract,
//!   `modules/auth/session_settings.rs`); `enforce` additionally re-syncs the
//!   fields the file DECLARES on every boot (a field the file omits keeps its DB
//!   value — the update is COALESCE-based). `mode` is accepted on `admin` /
//!   `users` but inert: both are pure create-if-absent. `groups` carry no mode: a
//!   permission set has no create/update distinction, so it is always reconciled.
//! - **Group availability always converges.** The `groups:` list on a server is
//!   re-applied on every boot even in `ensure` mode (assignment is additive and
//!   idempotent), because a server assigned to no group is unusable by non-admin
//!   users — a first-boot assignment failure must be self-healing. Removing a
//!   group from the file does NOT revoke it.
//! - **A bad ENTRY never crashes boot; a bad FILE does.** An unresolved env var,
//!   an inline secret, or a DB error on one entry logs an error and skips just
//!   that entry. But an unreadable / unparseable desired-state FILE is FATAL: a
//!   manifest the operator MEANT to apply, which the server cannot even read, is
//!   a deploy bug — failing loudly beats serving a silently-unconfigured box. A
//!   file that is simply ABSENT is not an error: that is how every
//!   non-config-as-code deployment (dev, desktop, tests) runs.
//!
//!   Note the deliberate asymmetry: an unset admin env triple skips the admin
//!   bootstrap (loudly) rather than aborting. That leaves the deployment in
//!   ziee's ORDINARY fresh-install state — no admin, so the unauthenticated
//!   first-run setup page is open, exactly as for any stock `docker compose up`.
//!   It is not a state this module introduces, and making it fatal would break
//!   the documented quick-start; the warning tells the operator to set the var.
//!
//! Pattern mirrors: `modules/auth/session_settings.rs::seed_from_config_once`
//! (config → DB once, DB authoritative after) and
//! `desktop/tauri/src/modules/auth/bootstrap.rs::ensure_desktop_admin`
//! (boot-time admin creation via `Repos.app.create_admin_user`).
//!
//! **Operational note (MCP health check).** `mcp::init` spawns a boot health
//! check that probes every enabled, non-built-in MCP server and AUTO-DISABLES
//! the unreachable ones (`mcp/connection_health.rs`). A declared org server whose
//! endpoint is not up when ziee boots will therefore be flipped to
//! `enabled = false`. That is why the shipped manifest declares the servers
//! `mode: enforce` — the next deploy re-asserts `enabled: true`.

use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::auth::password;
use crate::modules::auth::providers::repository as provider_repo;
use crate::modules::mcp::{
    CreateMcpServerRequest, TransportType, UpdateMcpServerRequest, UsageMode,
};

/// The DEPLOY SIGNAL. The reconciler does nothing at all unless this is set to
/// `1` / `true`. Default OFF, so a local developer — who has hand-configured
/// their own models, MCP servers, admin and permissions — can never have that
/// state seeded, enforced, or duplicated just because the repo happens to carry
/// a desired-state file. Only a deploy (TeamCity) turns it on.
pub const APPLY_ENV: &str = "ZIEE_APPLY_DESIRED_STATE";

/// The admin bootstrap is FULLY env-driven — deliberately NOT in the committed
/// file, which must never carry an admin identity or credential.
pub const ADMIN_USERNAME_ENV: &str = "ZIEE_ADMIN_USERNAME";
pub const ADMIN_EMAIL_ENV: &str = "ZIEE_ADMIN_EMAIL";
pub const ADMIN_PASSWORD_ENV: &str = "ZIEE_ADMIN_PASSWORD";

/// Env var holding the absolute path of the desired-state file. Only consulted
/// once [`APPLY_ENV`] has opted in — the file's mere presence never applies it.
pub const DESIRED_STATE_ENV: &str = "ZIEE_DESIRED_STATE_FILE";

/// Default path inside the container image. Used when the deploy signal is on
/// but no explicit path was given.
const DEFAULT_FILE: &str = "/etc/ziee/desired-state.yaml";

/// Is the deploy signal on?
fn apply_enabled() -> bool {
    matches!(
        std::env::var(APPLY_ENV).unwrap_or_default().trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

// ───────────────────────────── the file schema ─────────────────────────────

/// Reconcile mode for entities that have a create-vs-update distinction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Create when absent; if it already exists, leave it completely alone.
    #[default]
    Ensure,
    /// Create when absent; otherwise re-sync the declared fields every boot.
    Enforce,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesiredState {
    /// Admin system MCP servers (`is_system = true`, no owner).
    #[serde(default)]
    pub mcp_servers: Vec<McpServerEntry>,
    /// Regular (non-admin) accounts, placed in the default group.
    #[serde(default)]
    pub users: Vec<UserEntry>,
    /// Declarative group-permission reconcile.
    #[serde(default)]
    pub groups: Vec<GroupEntry>,
    /// Pre-seeded auth providers to configure + enable from env (e.g. `google`).
    #[serde(default)]
    pub auth_providers: Vec<AuthProviderEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServerEntry {
    /// Natural key. Dedup is `(name, is_system = true)` — there is no unique
    /// index on `mcp_servers.name`, so this check is what stops a re-deploy
    /// from silently creating duplicate rows.
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// http(s) endpoint; typically an env placeholder, e.g. `${RCPA_MCP_URL}`.
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub supports_sampling: bool,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
    /// Group names this system server is made available to. Without at least
    /// one, only admins can use it (that is how the seeded `fetch` server is
    /// wired). Applied on create, and on every boot in `enforce` mode.
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub mode: Mode,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserEntry {
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    /// MUST be a single `${VAR}` placeholder — an inline literal is rejected.
    pub password: String,
    /// Accepted but inert — see `AdminEntry::mode`.
    #[serde(default)]
    pub mode: Mode,
}

/// A group's permission reconcile. Deliberately has **no `mode`**: a permission
/// set has no create/update distinction, so the declared removals/additions are
/// re-applied on every boot (which is what makes a later `grant_*_to_users`
/// migration re-adding a hidden feature's permission self-correcting).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupEntry {
    pub name: String,
    /// Permissions to strip. An entry ending in `::*` matches by `::`
    /// hierarchy prefix (`hub::*` strips `hub::models::read`, …); any other
    /// entry is an exact match.
    #[serde(default)]
    pub remove: Vec<String>,
    /// Permissions to add if absent (exact strings).
    #[serde(default)]
    pub add: Vec<String>,
}

/// A pre-seeded auth provider (migration 47 seeds `google`) to be configured +
/// enabled from the environment on deploy. The reconciler only UPDATES an
/// existing row's fields (client_id / client_secret / enabled) — it never
/// creates or deletes a provider (the row + its `provider_type` / issuer /
/// scopes / attribute_mapping come from a seed migration). Generic by name so a
/// future `microsoft` / `apple` provider (each seeded by its own migration) can
/// be enabled the same way.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthProviderEntry {
    /// Natural key = `auth_providers.name` (e.g. `google`). Idempotency key.
    pub name: String,
    /// OAuth/OIDC client id; typically an env placeholder, e.g.
    /// `${GOOGLE_CLIENT_ID}`. Not a secret, but an unset/empty placeholder
    /// SKIPS the whole entry (like an unset MCP url).
    #[serde(default)]
    pub client_id: Option<String>,
    /// OAuth/OIDC client secret. MUST be a single `${VAR}` placeholder — an
    /// inline literal is rejected. Unset/empty SKIPS the whole entry. Stored
    /// encrypted at rest via the auth-provider repository (never inline).
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: Mode,
}

fn default_true() -> bool {
    true
}

// ───────────────────────────── env templating ─────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum TemplateError {
    /// `${VAR}` had no value in the environment (or was empty).
    Unresolved(String),
    /// A secret field carried something other than a single `${VAR}`.
    InlineSecret,
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateError::Unresolved(var) => {
                write!(f, "env var ${{{var}}} is unset or empty")
            }
            TemplateError::InlineSecret => write!(
                f,
                "secret fields must be exactly one ${{ENV_VAR}} placeholder, never an inline value"
            ),
        }
    }
}

/// How a `${VAR}` is looked up. Production passes [`env_lookup`]; the unit tests
/// pass a map, so they never mutate the process environment — `std::env::set_var`
/// is `unsafe` precisely because it races concurrent `getenv` in other test
/// threads of the same binary.
pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// The production lookup: process environment.
pub fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Substitute every `${VAR}` in `raw` with its value from `lookup`.
///
/// A `$` not followed by `{` is left intact. An unset/EMPTY var is an error (the
/// caller skips that entry) rather than a silent empty string — a server
/// registered at the URL "" would be worse than one that is absent. This also
/// makes `docker-compose`'s `"${RCPA_MCP_URL:-}"` (unset → empty string) behave
/// as "not configured", which is what the compose file documents.
pub fn resolve_with(raw: &str, lookup: Lookup<'_>) -> Result<String, TemplateError> {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            match raw[i + 2..].find('}') {
                Some(rel_end) => {
                    let name = &raw[i + 2..i + 2 + rel_end];
                    // `${}` (empty name) is not a placeholder — pass it through.
                    if name.is_empty() {
                        out.push_str("${}");
                    } else {
                        let value = lookup(name).unwrap_or_default();
                        if value.is_empty() {
                            return Err(TemplateError::Unresolved(name.to_string()));
                        }
                        out.push_str(&value);
                    }
                    i += 2 + rel_end + 1;
                }
                // Unterminated `${` — literal.
                None => {
                    out.push_str(&raw[i..]);
                    break;
                }
            }
        } else {
            let ch_len = raw[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            out.push_str(&raw[i..i + ch_len]);
            i += ch_len;
        }
    }

    Ok(out)
}

/// Resolve a SECRET field. The value must be exactly one `${VAR}` placeholder;
/// anything else means a secret was committed inline, which we refuse.
pub fn resolve_secret_with(raw: &str, lookup: Lookup<'_>) -> Result<String, TemplateError> {
    let trimmed = raw.trim();
    let is_single_placeholder = trimmed.starts_with("${")
        && trimmed.ends_with('}')
        && trimmed.len() > 3
        // no second placeholder, and no literal text around it
        && !trimmed[2..trimmed.len() - 1].contains('{')
        && !trimmed[2..trimmed.len() - 1].contains('}');

    if !is_single_placeholder {
        return Err(TemplateError::InlineSecret);
    }
    resolve_with(trimmed, lookup)
}

// ─────────────────────────── permission set-ops ───────────────────────────

/// Does `pattern` match permission `perm`? `foo::*` matches `foo` and anything
/// under `foo::`; every other pattern is an exact match. Mirrors the `::`
/// hierarchy the permission checker itself uses
/// (`modules/permissions/checker.rs::check_permissions_array`).
pub fn permission_matches(pattern: &str, perm: &str) -> bool {
    match pattern.strip_suffix("::*") {
        Some(prefix) => perm == prefix || perm.starts_with(&format!("{prefix}::")),
        None => pattern == perm,
    }
}

/// Is this a legal `add:` entry? Wildcards are REFUSED on the add side: `*` is
/// the grant-everything permission the Administrators group holds
/// (`permissions/checker.rs` short-circuits on it), and `foo::*` is not a real
/// permission — so a `remove:`-shaped pattern accidentally pasted into `add:`
/// would hand the default group far more than the author meant.
///
/// This is a FOOTGUN guard, not a trust boundary: the manifest is trusted deploy
/// config (same trust level as `config.yaml`, which holds `jwt.secret`), and it
/// can still grant any CONCRETE permission it names. Removal patterns may use
/// `::*` freely — widening is what needs the guard, not narrowing.
pub fn is_legal_add(perm: &str) -> bool {
    !perm.is_empty() && !perm.contains('*')
}

/// Apply the declared removals + additions to a permission array.
/// Returns `(new_array, rejected_adds)` — the caller compares the array against
/// the old one to decide whether a write is needed at all, and logs the
/// rejections.
pub fn apply_permission_ops(
    current: &[String],
    remove: &[String],
    add: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut next: Vec<String> = current
        .iter()
        .filter(|perm| !remove.iter().any(|pat| permission_matches(pat, perm)))
        .cloned()
        .collect();

    let mut rejected = Vec::new();
    for perm in add {
        if !is_legal_add(perm) {
            rejected.push(perm.clone());
            continue;
        }
        if !next.iter().any(|existing| existing == perm) {
            next.push(perm.clone());
        }
    }

    (next, rejected)
}

// ───────────────────────────── the reconciler ─────────────────────────────

/// Largest desired-state file we will read. A manifest is a few KiB; this cap
/// exists so a bind-mount pointed at something pathological can't be slurped
/// into memory.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// A fixed key for the Postgres advisory lock that serializes the reconcile
/// across concurrently-booting containers. Arbitrary but stable.
const RECONCILE_LOCK_KEY: i64 = 0x7A1E_E_DE51_u64 as i64;

/// Bounded wait for that lock: 60 × 500ms = 30s. A reconcile is a handful of
/// statements, so a peer that holds it longer than this is stuck, not slow.
const LOCK_WAIT_ATTEMPTS: usize = 60;
const LOCK_WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Reconcile the desired-state file into the DB.
///
/// `Ok(())` also covers "there is nothing to do" (env var unset / file absent).
/// `Err(msg)` means the FILE itself is unusable — the caller must FAIL THE BOOT
/// rather than serve a silently-unconfigured deployment. A failure of one
/// ENTRY inside a valid file is logged and skipped, never fatal.
pub async fn reconcile(pool: &PgPool) -> Result<(), String> {
    // The deploy gate. OFF unless a deploy explicitly turns it on — a repo-checked
    // desired-state file must never apply itself on a developer's machine.
    if !apply_enabled() {
        tracing::info!(
            "desired-state reconcile: disabled ({APPLY_ENV} is not set) — no seeding, no enforce, \
             no MCP/admin/permission writes"
        );
        return Ok(());
    }

    let path = std::path::PathBuf::from(
        std::env::var_os(DESIRED_STATE_ENV).unwrap_or_else(|| DEFAULT_FILE.into()),
    );
    tracing::info!(
        path = %path.display(),
        "desired-state reconcile: ENABLED ({APPLY_ENV} is set)"
    );

    // An ABSENT file is the normal "not a config-as-code deployment" case (dev,
    // desktop, the test suite, and the image with the var pointed elsewhere).
    if !path.exists() {
        tracing::info!(
            path = %path.display(),
            "desired_state: file not found; skipping reconcile"
        );
        return Ok(());
    }

    // A path that exists but is NOT a regular file is a misconfiguration, not a
    // manifest. Docker famously creates a DIRECTORY when you bind-mount a host
    // path that doesn't exist; a FIFO/char device would block the read forever.
    let meta = std::fs::metadata(&path)
        .map_err(|e| format!("cannot stat {} : {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!(
            "{} is not a regular file (a bind-mount of a missing host path creates a directory)",
            path.display()
        ));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{} is {} bytes, over the {MAX_FILE_BYTES}-byte cap",
            path.display(),
            meta.len()
        ));
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let desired: DesiredState = serde_norway::from_str(&raw)
        .map_err(|e| format!("{} is not a valid desired-state file: {e}", path.display()))?;

    tracing::info!(path = %path.display(), "desired_state: reconciling");

    // Serialize across concurrently-booting containers (rolling redeploy,
    // `docker compose up --scale`). Without this, two processes both pass the
    // check-then-insert and create duplicate system MCP servers — `mcp_servers`
    // has no unique index on `name`. The lock is released when the connection
    // returns to the pool at the end of this function.
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| format!("cannot acquire a connection for the reconcile lock: {e}"))?;

    // Bounded wait, NOT a bare `pg_advisory_lock`: that blocks forever, so a peer
    // container hung mid-reconcile would hang THIS boot with no diagnosis. Poll
    // `pg_try_advisory_lock` and give up with a clear error (the container then
    // restarts and tries again). A process that DIES holding the lock releases it
    // automatically — Postgres drops session advisory locks when the session ends.
    let mut locked = false;
    for _ in 0..LOCK_WAIT_ATTEMPTS {
        match sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
            .bind(RECONCILE_LOCK_KEY)
            .fetch_one(&mut *conn)
            .await
        {
            Ok(true) => {
                locked = true;
                break;
            }
            Ok(false) => {
                tracing::info!(
                    "desired_state: another instance is reconciling; waiting for the lock"
                );
                tokio::time::sleep(LOCK_WAIT_INTERVAL).await;
            }
            Err(e) => return Err(format!("cannot take the reconcile advisory lock: {e}")),
        }
    }
    if !locked {
        return Err(
            "another instance has held the reconcile lock for too long; giving up (the container \
             will retry on restart)"
                .to_string(),
        );
    }

    let result = reconcile_entries(&desired).await;

    // Release the lock. `drop()` alone is NOT enough: a PoolConnection returns the
    // LIVE session to the pool, so a session-level advisory lock survives it — if
    // the unlock statement failed, the lock would be held for this process's whole
    // lifetime and every peer container would block on boot. On failure, CLOSE the
    // connection instead: ending the session makes Postgres drop the lock.
    match sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(RECONCILE_LOCK_KEY)
        .execute(&mut *conn)
        .await
    {
        Ok(_) => drop(conn),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "desired_state: could not release the reconcile lock; closing the connection so \
                 Postgres drops it with the session"
            );
            let _ = conn.close().await;
        }
    }

    result
}

/// The per-entry work, under the advisory lock. Each entry soft-fails.
async fn reconcile_entries(desired: &DesiredState) -> Result<(), String> {
    // Admin FIRST — and from the environment, never from the file. It may only
    // run on a virgin database, so it must go before the `users:` entries below
    // (which would otherwise populate the table and block it).
    bootstrap_admin_from_env().await;
    for user in &desired.users {
        reconcile_user(user).await;
    }
    for group in &desired.groups {
        reconcile_group(group).await;
    }
    for server in &desired.mcp_servers {
        reconcile_mcp_server(server).await;
    }
    for provider in &desired.auth_providers {
        reconcile_auth_provider(provider).await;
    }

    tracing::info!("desired_state: reconcile complete");
    Ok(())
}

/// Bootstrap the root admin from the ENVIRONMENT (`ZIEE_ADMIN_USERNAME` /
/// `ZIEE_ADMIN_EMAIL` / `ZIEE_ADMIN_PASSWORD`) — never from the committed file,
/// which must not carry an admin identity or credential.
///
/// It runs ONLY on a database with NO account at all. That is deliberately
/// stricter than "no admin": on any deployment that already has users, this is a
/// no-op, so it can never overwrite an account or reset a password — an operator
/// who rotates the admin password in the UI keeps their rotation forever.
async fn bootstrap_admin_from_env() {
    let username = std::env::var(ADMIN_USERNAME_ENV).unwrap_or_default();
    let email = std::env::var(ADMIN_EMAIL_ENV).unwrap_or_default();
    let password = std::env::var(ADMIN_PASSWORD_ENV).unwrap_or_default();

    if username.trim().is_empty() || email.trim().is_empty() || password.is_empty() {
        tracing::info!(
            "desired_state: admin bootstrap skipped — set {ADMIN_USERNAME_ENV}, {ADMIN_EMAIL_ENV} \
             and {ADMIN_PASSWORD_ENV} to create the first administrator. (With no admin, the \
             unauthenticated first-run setup page stays open.)"
        );
        return;
    }

    match Repos.user.has_any_user().await {
        // The virgin-DB guard: ANY existing account (admin or not) means this
        // deployment is already bootstrapped. Never touch it.
        Ok(true) => {
            tracing::info!(
                "desired_state: admin bootstrap skipped — this database already has accounts \
                 (nothing is overwritten, no password is ever reset)"
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error = ?e, "desired_state: account check failed; admin bootstrap skipped");
            return;
        }
    }

    if let Err(reason) = password::validate_password_strength(&password) {
        tracing::error!(
            reason = %reason,
            "desired_state: {ADMIN_PASSWORD_ENV} rejected; NO ADMIN CREATED (the first-run setup page is open)"
        );
        return;
    }

    let hash = match password::hash_password(&password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "desired_state: password hashing failed; admin not created");
            return;
        }
    };

    match Repos
        .app
        .create_admin_user(username.trim(), email.trim(), &hash, None)
        .await
    {
        Ok(user) => tracing::info!(
            username = %user.username,
            "desired_state: bootstrapped the root admin from the environment (Administrators + Users)"
        ),
        Err(e) => {
            tracing::error!(error = ?e, "desired_state: admin creation failed")
        }
    }
}

/// Create a regular (non-admin) user in the default group, if absent.
async fn reconcile_user(entry: &UserEntry) {
    if entry.mode == Mode::Enforce {
        tracing::warn!(
            username = %entry.username,
            "desired_state: `mode: enforce` is INERT on an account — an existing user is never \
             re-written and its password is never reset. Remove the key or use `ensure`."
        );
    }

    // `users.username` and `users.email` are each UNIQUE, so BOTH must be free
    // before we insert (the lookup matches one identifier against either column,
    // so a single call with the username would miss an email-only collision).
    for identifier in [&entry.username, &entry.email] {
        match Repos.user.get_by_username_or_email(identifier).await {
            Ok(Some(_)) => {
                tracing::debug!(username = %entry.username, "desired_state: user already exists; untouched");
                return;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = ?e, username = %entry.username, "desired_state: user lookup failed; skipping");
                return;
            }
        }
    }

    let password = match resolve_secret_with(&entry.password, &env_lookup) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                username = %entry.username,
                reason = %e,
                "desired_state: user not created (set the password env var to enable it)"
            );
            return;
        }
    };

    if let Err(reason) = password::validate_password_strength(&password) {
        tracing::error!(
            username = %entry.username,
            reason = %reason,
            "desired_state: user password rejected; user not created"
        );
        return;
    }

    let hash = match password::hash_password(&password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "desired_state: password hashing failed; user not created");
            return;
        }
    };

    match Repos
        .auth
        .create_local_user_with_default_group(
            &entry.username,
            &entry.email,
            Some(hash),
            entry.display_name.clone(),
        )
        .await
    {
        Ok(user) => {
            tracing::info!(username = %user.username, "desired_state: created user (default group)")
        }
        Err(e) => {
            tracing::error!(error = ?e, username = %entry.username, "desired_state: user creation failed")
        }
    }
}

/// Reconcile one group's permission array (declarative; re-applied every boot).
async fn reconcile_group(entry: &GroupEntry) {
    let group = match Repos.group.get_by_name(&entry.name).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            tracing::error!(group = %entry.name, "desired_state: group not found; skipping");
            return;
        }
        Err(e) => {
            tracing::error!(error = ?e, group = %entry.name, "desired_state: group lookup failed; skipping");
            return;
        }
    };

    let (next, rejected) = apply_permission_ops(&group.permissions, &entry.remove, &entry.add);
    for perm in &rejected {
        let reason = if perm.trim().is_empty() {
            "the entry is empty"
        } else {
            "a manifest must grant CONCRETE permissions, never a `*` wildcard"
        };
        tracing::error!(
            group = %entry.name,
            permission = %perm,
            reason,
            "desired_state: REFUSING to add this permission"
        );
    }
    if next == group.permissions {
        tracing::debug!(group = %entry.name, "desired_state: group permissions already reconciled");
        return;
    }

    let removed = group.permissions.len().saturating_sub(
        group
            .permissions
            .iter()
            .filter(|p| next.contains(p))
            .count(),
    );

    match Repos
        .group
        .update(group.id, None, None, Some(next), None)
        .await
    {
        Ok(updated) => tracing::info!(
            group = %entry.name,
            removed,
            total = updated.permissions.len(),
            "desired_state: group permissions reconciled"
        ),
        Err(e) => {
            tracing::error!(error = ?e, group = %entry.name, "desired_state: group permission update failed")
        }
    }
}

/// Create (or, in `enforce` mode, re-sync) one admin system MCP server, then
/// make it available to the declared groups.
async fn reconcile_mcp_server(entry: &McpServerEntry) {
    let url = match resolve_with(&entry.url, &env_lookup) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(
                server = %entry.name,
                reason = %e,
                "desired_state: MCP server skipped (its URL env var is not set)"
            );
            return;
        }
    };

    // Natural-key dedup. There is NO unique index on `mcp_servers.name`, so
    // without this check a second deploy would silently create a duplicate row.
    // (Concurrent deploys are serialized by the advisory lock in `reconcile`.)
    let existing = match Repos.mcp.get_system_server_by_name(&entry.name).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = ?e, server = %entry.name, "desired_state: MCP server lookup failed; skipping");
            return;
        }
    };

    let server_id = match (existing, entry.mode) {
        // Already there, `ensure` → leave its FIELDS exactly as the admin last
        // left them. The group assignment below still runs: a server assigned to
        // no group is unusable by non-admin users, so a first-boot assignment
        // failure has to be self-healing (assignment is additive + idempotent).
        (Some(id), Mode::Ensure) => {
            tracing::debug!(server = %entry.name, "desired_state: MCP server already present; fields untouched (ensure)");
            id
        }

        // Already there, `enforce` → re-sync the declared fields.
        (Some(id), Mode::Enforce) => {
            let request = UpdateMcpServerRequest {
                name: None,
                display_name: Some(entry.display_name.clone()),
                description: entry.description.clone(),
                enabled: Some(entry.enabled),
                command: None,
                args: None,
                environment_variables_entries: None,
                url: Some(url),
                headers_entries: None,
                timeout_seconds: entry.timeout_seconds,
                supports_sampling: Some(entry.supports_sampling),
                // Usage mode is always `auto` — the model decides when to reach
                // for these tools (never force-attached to every request).
                usage_mode: Some(UsageMode::Auto),
                max_concurrent_sessions: None,
                run_in_sandbox: None,
                sandbox_flavor: None,
            };
            match Repos.mcp.update_system_server(id, request).await {
                Ok(_) => tracing::info!(server = %entry.name, "desired_state: MCP server re-synced (enforce)"),
                // Log and carry on to the group assignment below — do NOT return.
                // A failed field re-sync must not also strand the server in no
                // group (that is the one failure that makes it unusable).
                Err(e) => {
                    tracing::error!(error = ?e, server = %entry.name, "desired_state: MCP server field re-sync failed; still converging its group assignment")
                }
            }
            id
        }

        // Absent → create it. Goes through the repository (transport validation
        // + at-rest secret handling), not raw SQL.
        (None, _) => {
            let request = CreateMcpServerRequest {
                name: entry.name.clone(),
                display_name: entry.display_name.clone(),
                description: entry.description.clone(),
                enabled: Some(entry.enabled),
                transport_type: TransportType::Http,
                command: None,
                args: None,
                environment_variables_entries: None,
                url: Some(url),
                headers_entries: None,
                timeout_seconds: entry.timeout_seconds,
                supports_sampling: Some(entry.supports_sampling),
                // Usage mode is always `auto` — the model decides when to reach
                // for these tools (never force-attached to every request).
                usage_mode: Some(UsageMode::Auto),
                max_concurrent_sessions: None,
                run_in_sandbox: None,
                sandbox_flavor: None,
                hub_id: None,
            };
            match Repos.mcp.create_system_server(request).await {
                Ok(server) => {
                    tracing::info!(server = %entry.name, id = %server.id, "desired_state: created system MCP server");
                    server.id
                }
                Err(e) => {
                    tracing::error!(error = ?e, server = %entry.name, "desired_state: MCP server creation failed");
                    return;
                }
            }
        }
    };

    assign_groups(server_id, entry).await;
}

/// Make a system server available to the declared groups (idempotent:
/// `assign_to_group` is `ON CONFLICT DO NOTHING`).
async fn assign_groups(server_id: Uuid, entry: &McpServerEntry) {
    for group_name in &entry.groups {
        let group = match Repos.group.get_by_name(group_name).await {
            Ok(Some(g)) => g,
            Ok(None) => {
                tracing::error!(server = %entry.name, group = %group_name, "desired_state: group not found; server not assigned");
                continue;
            }
            Err(e) => {
                tracing::error!(error = ?e, group = %group_name, "desired_state: group lookup failed; server not assigned");
                continue;
            }
        };

        match Repos.mcp.assign_to_group(group.id, server_id).await {
            Ok(()) => {
                tracing::info!(server = %entry.name, group = %group_name, "desired_state: MCP server assigned to group")
            }
            Err(e) => {
                tracing::error!(error = ?e, server = %entry.name, group = %group_name, "desired_state: group assignment failed")
            }
        }
    }
}

// ─────────────────────────── auth providers ───────────────────────────

/// The outcome of resolving an [`AuthProviderEntry`] against the environment and
/// the provider's existing (seeded) config — pure, so it is unit-testable
/// without a database.
#[derive(Debug)]
enum AuthPlan {
    /// The entry is not configured (an env var is unset/empty, or the secret is
    /// an inline literal). Log the reason and leave the row untouched.
    Skip(String),
    /// The entry is fully resolved: stamp this merged `config` (client_id +
    /// encrypted-at-rest client_secret) and set `enabled` on the existing row.
    Stamp {
        config: serde_json::Value,
        enabled: bool,
    },
}

/// Resolve an auth-provider entry's `client_id` / `client_secret` from the
/// environment and merge them onto the provider's EXISTING config (preserving
/// the seeded issuer / scopes / attribute_mapping / display_name). Pure: the DB
/// read + encrypted write happen in [`reconcile_auth_provider`].
///
/// - `client_id` is not a secret but an unset/empty `${VAR}` still SKIPS the
///   whole entry (a half-configured provider is worse than a disabled one).
/// - `client_secret` MUST be a single `${VAR}` placeholder (an inline literal is
///   rejected) — the same rule `reconcile_user` enforces on a password.
/// - The returned `config` carries the client_secret in PLAINTEXT; the caller's
///   `update_provider` encrypts it into `client_secret_encrypted` and blanks the
///   plaintext copy (the admin-CRUD at-rest path).
fn plan_auth_provider(
    entry: &AuthProviderEntry,
    existing_config: &serde_json::Value,
    lookup: Lookup<'_>,
) -> AuthPlan {
    let (Some(client_id_raw), Some(client_secret_raw)) =
        (entry.client_id.as_deref(), entry.client_secret.as_deref())
    else {
        return AuthPlan::Skip(format!(
            "{}: client_id and client_secret are both required",
            entry.name
        ));
    };

    // client_id: non-secret placeholder; unset/empty → skip.
    let client_id = match resolve_with(client_id_raw, lookup) {
        Ok(v) => v,
        Err(e) => return AuthPlan::Skip(format!("{}: {e}", entry.name)),
    };

    // client_secret: single ${VAR} only (inline literal rejected); unset → skip.
    let client_secret = match resolve_secret_with(client_secret_raw, lookup) {
        Ok(v) => v,
        Err(e) => return AuthPlan::Skip(format!("{}: {e}", entry.name)),
    };

    // Merge onto the existing (seeded) config: only client_id + client_secret
    // change; issuer_url / scopes / attribute_mapping / display_name are kept.
    let mut config = existing_config.clone();
    if let serde_json::Value::Object(map) = &mut config {
        map.insert("client_id".to_string(), serde_json::Value::String(client_id));
        map.insert(
            "client_secret".to_string(),
            serde_json::Value::String(client_secret),
        );
    } else {
        return AuthPlan::Skip(format!(
            "{}: existing provider config is not a JSON object",
            entry.name
        ));
    }

    AuthPlan::Stamp {
        config,
        enabled: entry.enabled,
    }
}

/// Configure + enable a PRE-SEEDED auth provider from the environment. Only
/// UPDATES an existing row (never creates or deletes): the row, its
/// `provider_type`, and its issuer / scopes / attribute_mapping come from a seed
/// migration (`google` = migration 47). With the entry's env vars unset the row
/// is left exactly as seeded (disabled) — so local dev, and any deploy that
/// omits the creds, is a clean no-op.
async fn reconcile_auth_provider(entry: &AuthProviderEntry) {
    let pool = Repos.pool();

    let existing = match provider_repo::get_provider_by_name(pool, &entry.name).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            // Never create — an auth provider needs a seed migration for its
            // type + config shape. A missing row means the manifest names a
            // provider this build does not seed.
            tracing::warn!(
                provider = %entry.name,
                "desired_state: auth provider not seeded (no such row); skipping"
            );
            return;
        }
        Err(e) => {
            tracing::error!(error = ?e, provider = %entry.name, "desired_state: auth provider lookup failed; skipping");
            return;
        }
    };

    // `ensure` on an existing row leaves its fields untouched (mirrors the MCP
    // branch). A pre-seeded provider therefore needs `enforce` to actually be
    // configured + enabled — google ships `enforce`.
    if entry.mode == Mode::Ensure {
        tracing::debug!(provider = %entry.name, "desired_state: auth provider already present; fields untouched (ensure)");
        return;
    }

    match plan_auth_provider(entry, &existing.config, &env_lookup) {
        AuthPlan::Skip(reason) => {
            tracing::warn!(
                provider = %entry.name,
                reason = %reason,
                "desired_state: auth provider skipped (set its client_id + client_secret env vars to enable it)"
            );
        }
        AuthPlan::Stamp { config, enabled } => {
            // Goes through the repository's at-rest secret path
            // (`prepare_config_for_write` → encrypt into
            // `client_secret_encrypted`, blank the plaintext copy in `config`);
            // NOT a raw SQL insert of the plaintext secret.
            match provider_repo::update_provider(pool, existing.id, None, Some(enabled), Some(&config)).await
            {
                Ok(p) => tracing::info!(
                    provider = %entry.name,
                    enabled = p.enabled,
                    "desired_state: auth provider configured + enabled from env"
                ),
                Err(e) => {
                    tracing::error!(error = ?e, provider = %entry.name, "desired_state: auth provider update failed")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A lookup backed by a map — the unit tests NEVER mutate the process
    /// environment. `std::env::set_var` is `unsafe` because it can realloc the
    /// shared `environ` block under a concurrent `getenv` in another test thread
    /// of the same binary; injecting the lookup sidesteps that entirely.
    fn map_lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    // ── TEST-1: env templating ──

    #[test]
    fn resolve_substitutes_vars() {
        let lookup = map_lookup(&[("RCPA_MCP_URL", "http://rcpa:9000/mcp")]);

        assert_eq!(
            resolve_with("${RCPA_MCP_URL}", &lookup).unwrap(),
            "http://rcpa:9000/mcp".to_string()
        );
        // Surrounding literal text is preserved.
        assert_eq!(
            resolve_with("prefix ${RCPA_MCP_URL} suffix", &lookup).unwrap(),
            "prefix http://rcpa:9000/mcp suffix".to_string()
        );
    }

    #[test]
    fn resolve_errors_on_unset_var() {
        let lookup = map_lookup(&[]);
        assert_eq!(
            resolve_with("${NOT_SET}", &lookup).unwrap_err(),
            TemplateError::Unresolved("NOT_SET".to_string())
        );
    }

    #[test]
    fn resolve_treats_an_empty_var_as_unset() {
        // docker-compose's `"${RCPA_MCP_URL:-}"` yields an EMPTY string when the
        // operator didn't set it — that must mean "not configured" (skip the
        // entry), never a server registered at the url "".
        let lookup = map_lookup(&[("EMPTY", "")]);
        assert!(matches!(
            resolve_with("${EMPTY}", &lookup).unwrap_err(),
            TemplateError::Unresolved(_)
        ));
    }

    #[test]
    fn resolve_leaves_non_placeholder_dollars_intact() {
        let lookup = map_lookup(&[]);
        assert_eq!(resolve_with("costs $5", &lookup).unwrap(), "costs $5");
        assert_eq!(resolve_with("${}", &lookup).unwrap(), "${}");
        // Unterminated `${` is literal, not an error.
        assert_eq!(resolve_with("a ${OPEN", &lookup).unwrap(), "a ${OPEN");
    }

    #[test]
    fn resolve_is_utf8_safe() {
        let lookup = map_lookup(&[("V", "x")]);
        assert_eq!(resolve_with("héllo ${V} 日本", &lookup).unwrap(), "héllo x 日本");
    }

    // ── TEST-2: the inline-secret guard ──

    #[test]
    fn resolve_secret_accepts_only_a_single_placeholder() {
        let lookup = map_lookup(&[("PW", "s3cret-value")]);

        assert_eq!(resolve_secret_with("${PW}", &lookup).unwrap(), "s3cret-value");
        assert_eq!(resolve_secret_with("  ${PW}  ", &lookup).unwrap(), "s3cret-value");

        // Everything else means a secret was committed to the file.
        for inline in ["hunter2", "prefix-${PW}", "${PW}${PW}", "${PW}x", ""] {
            assert_eq!(
                resolve_secret_with(inline, &lookup).unwrap_err(),
                TemplateError::InlineSecret,
                "{inline:?} must be rejected as an inline secret"
            );
        }
    }

    #[test]
    fn resolve_secret_propagates_an_unset_var() {
        let lookup = map_lookup(&[]);
        assert!(matches!(
            resolve_secret_with("${PW_UNSET}", &lookup).unwrap_err(),
            TemplateError::Unresolved(_)
        ));
    }

    // ── TEST-3: permission set-ops ──

    #[test]
    fn permission_wildcard_matches_by_hierarchy() {
        assert!(permission_matches("hub::*", "hub::models::read"));
        assert!(permission_matches("hub::*", "hub::assistants::create"));
        assert!(permission_matches("hub::*", "hub"));
        // Must not match a different segment that merely shares a text prefix.
        assert!(!permission_matches("hub::*", "hubris::read"));
        assert!(!permission_matches("hub::*", "chat::read"));
        // Non-wildcard patterns are exact.
        assert!(permission_matches("assistants::read", "assistants::read"));
        assert!(!permission_matches("assistants::read", "assistants::edit"));
    }

    #[test]
    fn apply_permission_ops_removes_the_hidden_features_and_keeps_the_rest() {
        let current: Vec<String> = [
            "profile::read",
            "chat::read",
            "assistants::create",
            "assistants::read",
            "hub::models::read",
            "hub::mcp_servers::create",
            "projects::read",
            "mcp_servers::read",
            "user_llm_providers::read",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let (next, rejected) = apply_permission_ops(
            &current,
            &[
                "assistants::*".to_string(),
                "hub::*".to_string(),
                "projects::*".to_string(),
            ],
            &[],
        );

        assert!(rejected.is_empty());
        assert_eq!(
            next,
            vec![
                "profile::read".to_string(),
                "chat::read".to_string(),
                "mcp_servers::read".to_string(),
                "user_llm_providers::read".to_string(),
            ],
            "exactly the hidden features are stripped; the KEEP set survives"
        );
    }

    #[test]
    fn apply_permission_ops_add_is_idempotent() {
        let current = vec!["chat::read".to_string()];

        let (next, _) = apply_permission_ops(
            &current,
            &[],
            &["chat::read".to_string(), "chat::create".to_string()],
        );
        assert_eq!(next, vec!["chat::read".to_string(), "chat::create".to_string()]);

        // Re-applying the same ops changes nothing — the caller uses this
        // equality to skip the DB write entirely.
        let (again, _) = apply_permission_ops(
            &next,
            &[],
            &["chat::read".to_string(), "chat::create".to_string()],
        );
        assert_eq!(again, next);
    }

    #[test]
    fn apply_permission_ops_refuses_to_add_a_wildcard() {
        // `*` is the grant-everything permission the Administrators group holds.
        // A typo'd or tampered manifest must never be able to hand it to the
        // default group.
        let current = vec!["chat::read".to_string()];

        let (next, rejected) = apply_permission_ops(
            &current,
            &[],
            &["*".to_string(), "hub::*".to_string(), "files::read".to_string()],
        );

        assert_eq!(
            next,
            vec!["chat::read".to_string(), "files::read".to_string()],
            "the concrete permission is added; both wildcards are refused"
        );
        assert_eq!(rejected, vec!["*".to_string(), "hub::*".to_string()]);
        assert!(!next.iter().any(|p| p.contains('*')));
    }

    // ── TEST-4: the file schema ──

    #[test]
    fn parses_a_full_document() {
        let yaml = r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
    url: ${RCPA_MCP_URL}
    timeout_seconds: 300
    groups: [Users]
  - name: biognosia
    display_name: Biognosia
    url: ${BIOGNOSIA_MCP_URL}
    supports_sampling: true
    mode: enforce
users:
  - username: user
    email: user@tinnguyen-lab.com
    password: ${ZIEE_DEFAULT_USER_PASSWORD}
groups:
  - name: Users
    remove: ["assistants::*", "hub::*"]
"#;
        let ds: DesiredState = serde_norway::from_str(yaml).unwrap();

        assert_eq!(ds.mcp_servers.len(), 2);
        let rcpa = &ds.mcp_servers[0];
        assert_eq!(rcpa.name, "rcpa");
        assert_eq!(rcpa.timeout_seconds, Some(300));
        assert_eq!(rcpa.groups, vec!["Users".to_string()]);
        assert!(rcpa.enabled, "enabled defaults to true");
        assert!(!rcpa.supports_sampling, "supports_sampling defaults to false");
        assert_eq!(rcpa.mode, Mode::Ensure, "mode defaults to ensure");

        let bio = &ds.mcp_servers[1];
        assert!(bio.supports_sampling);
        assert_eq!(bio.mode, Mode::Enforce);

        // `mode` on a user entry parses (accepted-but-inert). Without the field,
        // `deny_unknown_fields` would fail the WHOLE document.
        assert_eq!(ds.users.len(), 1);
        assert_eq!(ds.groups[0].remove.len(), 2);
        assert!(ds.groups[0].add.is_empty());
    }

    #[test]
    fn empty_document_is_a_legal_no_op() {
        let ds: DesiredState = serde_norway::from_str("{}").unwrap();
        assert!(ds.mcp_servers.is_empty());
        assert!(ds.users.is_empty());
        assert!(ds.groups.is_empty());
    }

    #[test]
    fn rejects_an_unknown_mode() {
        let yaml = r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
    url: http://x/mcp
    mode: clobber
"#;
        assert!(serde_norway::from_str::<DesiredState>(yaml).is_err());
    }

    #[test]
    fn rejects_a_server_without_a_url() {
        let yaml = r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
"#;
        assert!(serde_norway::from_str::<DesiredState>(yaml).is_err());
    }

    #[test]
    fn rejects_an_unknown_field() {
        // deny_unknown_fields: a typo in the deploy manifest must fail loudly.
        // (Because a file-level parse error is FATAL at boot, that typo stops the
        // deploy instead of silently shipping an unconfigured server.)
        let yaml = r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
    url: http://x/mcp
    timout_seconds: 300
"#;
        assert!(serde_norway::from_str::<DesiredState>(yaml).is_err());
    }

    #[test]
    fn deploy_gate_is_off_by_default_and_accepts_the_usual_truthy_values() {
        // The gate reads process env, so assert on the parser rather than
        // mutating the environment (see `map_lookup`'s note).
        let on = |v: &str| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        };
        // Local-dev defaults: anything that is not an explicit opt-in is OFF.
        for off in ["", " ", "0", "false", "no", "off", "maybe"] {
            assert!(!on(off), "{off:?} must NOT enable the reconciler");
        }
        for enabled in ["1", "true", "TRUE", " yes ", "on"] {
            assert!(on(enabled), "{enabled:?} must enable the reconciler");
        }
    }

    // ── TEST-1/2/3/4: auth providers (env-driven enablement) ──

    /// The `google` row as migration 47 seeds it (disabled, creds blank, issuer /
    /// scopes / attribute_mapping / display_name filled).
    fn seeded_google_config() -> serde_json::Value {
        serde_json::json!({
            "client_id": "",
            "client_secret": "",
            "issuer_url": "https://accounts.google.com",
            "scopes": ["openid", "email", "profile"],
            "attribute_mapping": {
                "user_id": "sub",
                "username": "email",
                "email": "email",
                "display_name": "name",
                "first_name": "given_name",
                "last_name": "family_name"
            },
            "display_name": "Sign in with Google"
        })
    }

    fn google_entry(
        client_id: Option<&str>,
        client_secret: Option<&str>,
        mode: Mode,
    ) -> AuthProviderEntry {
        AuthProviderEntry {
            name: "google".to_string(),
            client_id: client_id.map(str::to_string),
            client_secret: client_secret.map(str::to_string),
            enabled: true,
            mode,
        }
    }

    // TEST-1: an auth_providers block parses; an unknown field is rejected.
    #[test]
    fn parses_an_auth_providers_block() {
        let yaml = r#"
auth_providers:
  - name: google
    enabled: true
    client_id: ${GOOGLE_CLIENT_ID}
    client_secret: ${GOOGLE_CLIENT_SECRET}
    mode: enforce
"#;
        let ds: DesiredState = serde_norway::from_str(yaml).unwrap();
        assert_eq!(ds.auth_providers.len(), 1);
        let g = &ds.auth_providers[0];
        assert_eq!(g.name, "google");
        assert!(g.enabled);
        assert_eq!(g.mode, Mode::Enforce);
        assert_eq!(g.client_id.as_deref(), Some("${GOOGLE_CLIENT_ID}"));
        assert_eq!(g.client_secret.as_deref(), Some("${GOOGLE_CLIENT_SECRET}"));

        // A name-only entry is legal (defaults apply); a typo'd field is not.
        let ok: DesiredState =
            serde_norway::from_str("auth_providers:\n  - name: google\n").unwrap();
        assert!(ok.auth_providers[0].enabled, "enabled defaults to true");
        assert_eq!(ok.auth_providers[0].mode, Mode::Ensure, "mode defaults to ensure");
        assert!(serde_norway::from_str::<DesiredState>(
            "auth_providers:\n  - name: google\n    clientid: x\n"
        )
        .is_err());
    }

    // TEST-2: an unset (or half-set) env skips the entry — never a partial write.
    #[test]
    fn plan_auth_provider_skips_when_env_unset() {
        let cfg = seeded_google_config();

        // Both unset.
        let empty = map_lookup(&[]);
        let entry = google_entry(Some("${GOOGLE_CLIENT_ID}"), Some("${GOOGLE_CLIENT_SECRET}"), Mode::Enforce);
        assert!(matches!(
            plan_auth_provider(&entry, &cfg, &empty),
            AuthPlan::Skip(_)
        ));

        // Only the id is set — still a skip (a half-configured provider is worse
        // than a disabled one).
        let only_id = map_lookup(&[("GOOGLE_CLIENT_ID", "id-123")]);
        assert!(matches!(
            plan_auth_provider(&entry, &cfg, &only_id),
            AuthPlan::Skip(_)
        ));

        // The field itself absent (None) — skip.
        let both = map_lookup(&[("GOOGLE_CLIENT_ID", "id-123"), ("GOOGLE_CLIENT_SECRET", "sec")]);
        let missing = google_entry(None, Some("${GOOGLE_CLIENT_SECRET}"), Mode::Enforce);
        assert!(matches!(
            plan_auth_provider(&missing, &cfg, &both),
            AuthPlan::Skip(_)
        ));
    }

    // TEST-3: an inline-literal client_secret is rejected (never stamped).
    #[test]
    fn plan_auth_provider_rejects_an_inline_secret() {
        let cfg = seeded_google_config();
        let lookup = map_lookup(&[("GOOGLE_CLIENT_ID", "id-123")]);
        // client_secret is a literal, not a ${VAR} placeholder.
        let entry = google_entry(Some("${GOOGLE_CLIENT_ID}"), Some("hunter2"), Mode::Enforce);
        assert!(matches!(
            plan_auth_provider(&entry, &cfg, &lookup),
            AuthPlan::Skip(_)
        ));
    }

    // TEST-4: both set → Stamp merges client_id + client_secret and PRESERVES the
    // seeded issuer / scopes / attribute_mapping / display_name; enabled honored.
    #[test]
    fn plan_auth_provider_stamps_and_preserves_seeded_fields() {
        let cfg = seeded_google_config();
        let lookup = map_lookup(&[
            ("GOOGLE_CLIENT_ID", "id-123.apps.googleusercontent.com"),
            ("GOOGLE_CLIENT_SECRET", "top-secret-value"),
        ]);
        let entry = google_entry(Some("${GOOGLE_CLIENT_ID}"), Some("${GOOGLE_CLIENT_SECRET}"), Mode::Enforce);

        let AuthPlan::Stamp { config, enabled } = plan_auth_provider(&entry, &cfg, &lookup) else {
            panic!("expected Stamp");
        };
        assert!(enabled);
        assert_eq!(config["client_id"], "id-123.apps.googleusercontent.com");
        // The plaintext secret rides in config; the repository encrypts it on write.
        assert_eq!(config["client_secret"], "top-secret-value");
        // Seeded fields untouched.
        assert_eq!(config["issuer_url"], "https://accounts.google.com");
        assert_eq!(config["scopes"], serde_json::json!(["openid", "email", "profile"]));
        assert_eq!(config["attribute_mapping"]["user_id"], "sub");
        assert_eq!(config["display_name"], "Sign in with Google");
    }

    // TEST-9: the shipped deploy compose passes BOTH Google env vars through so a
    // future edit can't silently drop the deploy wiring.
    #[test]
    fn shipped_deploy_compose_passes_google_env_through() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docker-compose.deploy.yml");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for var in ["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"] {
            // Empty-default passthrough: `GOOGLE_CLIENT_ID: "${GOOGLE_CLIENT_ID:-}"`.
            assert!(
                raw.contains(&format!("{var}: \"${{{var}:-}}\"")),
                "docker-compose.deploy.yml must pass {var} through with an empty default"
            );
        }
    }

    // ── TEST-17: the file we actually ship ──

    /// The manifest baked into the container image must parse, keep every secret
    /// in an env placeholder, and declare what the deploy expects — so a typo
    /// fails the build, not the deploy.
    #[test]
    fn shipped_desired_state_file_is_valid() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/desired-state.yaml");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

        let ds: DesiredState = serde_norway::from_str(&raw)
            .unwrap_or_else(|e| panic!("shipped desired-state.yaml is invalid: {e}"));

        // Secrets are placeholders, never literals. (An empty lookup makes a
        // legitimate placeholder fail as `Unresolved` — what we must never see is
        // `InlineSecret`, which means a password was committed.)
        let empty = map_lookup(&[]);
        for user in &ds.users {
            assert!(
                !matches!(
                    resolve_secret_with(&user.password, &empty),
                    Err(TemplateError::InlineSecret)
                ),
                "user {} has an INLINE password in the shipped file",
                user.username
            );
        }
        // The admin is env-only: the committed manifest must carry no admin
        // identity or credential whatsoever. `deny_unknown_fields` already makes
        // an `admin:` key fail to parse — this pins the intent in the file text.
        assert!(
            !raw.lines().any(|l| l.trim_start().starts_with("admin:")),
            "the shipped file must not carry an `admin:` block — the first admin is env-only"
        );

        // The three org servers are declared, each reachable by the Users group,
        // and each `enforce` — the boot health check auto-disables an unreachable
        // MCP server, so `ensure` would never re-enable it on a later deploy.
        let names: Vec<&str> = ds.mcp_servers.iter().map(|s| s.name.as_str()).collect();
        for expected in ["rcpa", "dscc", "biognosia"] {
            assert!(names.contains(&expected), "{expected} missing from the file");
        }
        for server in &ds.mcp_servers {
            assert!(
                server.groups.contains(&"Users".to_string()),
                "{} is assigned to no group — non-admin users could not use it",
                server.name
            );
            assert_eq!(
                server.mode,
                Mode::Enforce,
                "{} must be `enforce` so a health-check auto-disable is repaired on the next deploy",
                server.name
            );
            assert!(server.enabled, "{} must ship enabled", server.name);
        }

        // The Users group trims exactly the three hidden features, and adds
        // nothing (an `add` list is where a manifest could escalate).
        let users_group = ds
            .groups
            .iter()
            .find(|g| g.name == "Users")
            .expect("the shipped file must reconcile the Users group");
        for pattern in ["assistants::*", "hub::*", "projects::*"] {
            assert!(
                users_group.remove.contains(&pattern.to_string()),
                "{pattern} is not removed from the Users group"
            );
        }
        assert!(
            users_group.add.is_empty(),
            "the shipped file must not GRANT permissions"
        );

        // TEST-8: the `google` auth provider is declared, `enforce`, ships
        // enabled, and its client_secret is an env placeholder (never inline).
        let google = ds
            .auth_providers
            .iter()
            .find(|p| p.name == "google")
            .expect("the shipped file must declare the `google` auth provider");
        assert_eq!(
            google.mode,
            Mode::Enforce,
            "google must be `enforce` so a disabled provider is re-enabled on the next deploy"
        );
        assert!(google.enabled, "google must ship enabled");
        let secret = google
            .client_secret
            .as_deref()
            .expect("google must declare a client_secret placeholder");
        assert!(
            !matches!(
                resolve_secret_with(secret, &empty),
                Err(TemplateError::InlineSecret)
            ),
            "google has an INLINE client_secret in the shipped file"
        );
    }
}
