//! Config-as-code: a declarative "desired state" file the server reconciles
//! into its DB at boot, so a fresh deploy (TeamCity, `docker compose up`) comes
//! up fully configured with NO manual UI setup.
//!
//! Wiring: `main.rs` calls [`reconcile`] AFTER migrations + `init_repositories`
//! + `init_storage_key`, and BEFORE the server serves. The file is located by
//! the `ZIEE_DESIRED_STATE_FILE` env var (the container image sets it); unset,
//! or pointing at a path that does not exist, makes this a **silent no-op** —
//! so dev, the desktop app, and every existing test are unaffected.
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
//!   before writing (server → `(name, is_system)`; admin → "an admin exists";
//!   user → username/email; group → name). Re-running on the next deploy
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
//!   `users` but inert: both are pure create-if-absent (the admin password is
//!   never reset). `groups` carry no mode: a permission set has no create/update
//!   distinction, so it is always reconciled.
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
//!   Note the deliberate asymmetry: an UNSET `${ZIEE_ADMIN_PASSWORD}` skips the
//!   admin entry (loudly) rather than aborting. That leaves the deployment in
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
use crate::modules::mcp::{
    CreateMcpServerRequest, TransportType, UpdateMcpServerRequest, UsageMode,
};

/// Env var holding the absolute path of the desired-state file.
pub const DESIRED_STATE_ENV: &str = "ZIEE_DESIRED_STATE_FILE";

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
    /// The root admin account, created only when NO admin exists yet.
    #[serde(default)]
    pub admin: Option<AdminEntry>,
    /// Regular (non-admin) accounts, placed in the default group.
    #[serde(default)]
    pub users: Vec<UserEntry>,
    /// Declarative group-permission reconcile.
    #[serde(default)]
    pub groups: Vec<GroupEntry>,
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
pub struct AdminEntry {
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    /// MUST be a single `${VAR}` placeholder — an inline literal is rejected.
    pub password: String,
    /// Accepted for symmetry with `mcp_servers`, but INERT: an account is
    /// created only when absent, and its password is never reset. Present so a
    /// manifest that spells out `mode: ensure` here still parses (the structs
    /// are `deny_unknown_fields`, so an unknown key would fail the WHOLE file).
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
    let Some(path) = std::env::var_os(DESIRED_STATE_ENV) else {
        tracing::debug!("desired_state: {DESIRED_STATE_ENV} unset; nothing to reconcile");
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);

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
    // Admin first: `create_admin_user` also joins the Administrators + Users
    // groups, so it must run before the group-permission pass has any bearing
    // on it (it doesn't — root admins bypass permission checks — but the
    // ordering keeps the log narrative honest).
    if let Some(admin) = &desired.admin {
        reconcile_admin(admin).await;
    }
    for user in &desired.users {
        reconcile_user(user).await;
    }
    for group in &desired.groups {
        reconcile_group(group).await;
    }
    for server in &desired.mcp_servers {
        reconcile_mcp_server(server).await;
    }

    tracing::info!("desired_state: reconcile complete");
    Ok(())
}

/// Create the root admin ONLY when no admin exists. A later boot never resets
/// the password — the account is the operator's after first boot.
async fn reconcile_admin(entry: &AdminEntry) {
    if entry.mode == Mode::Enforce {
        tracing::warn!(
            username = %entry.username,
            "desired_state: `mode: enforce` is INERT on an account — an existing admin is never \
             re-written and its password is never reset. Remove the key or use `ensure`."
        );
    }

    match Repos.user.has_admin().await {
        Ok(true) => {
            tracing::info!(
                "desired_state: an admin already exists; leaving it untouched (password is never reset)"
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error = ?e, "desired_state: admin check failed; skipping admin");
            return;
        }
    }

    let password = match resolve_secret_with(&entry.password, &env_lookup) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                username = %entry.username,
                reason = %e,
                "desired_state: NO ADMIN CREATED — this deployment now has no administrator, so \
                 the unauthenticated first-run setup page is open to whoever reaches it first. \
                 Set the admin password env var and redeploy."
            );
            return;
        }
    };

    if let Err(reason) = password::validate_password_strength(&password) {
        tracing::error!(
            username = %entry.username,
            reason = %reason,
            "desired_state: admin password rejected; NO ADMIN CREATED (the first-run setup page is open)"
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
        .create_admin_user(
            &entry.username,
            &entry.email,
            &hash,
            entry.display_name.clone(),
        )
        .await
    {
        Ok(user) => tracing::info!(
            username = %user.username,
            "desired_state: created root admin (Administrators + Users)"
        ),
        Err(e) => tracing::error!(error = ?e, username = %entry.username, "desired_state: admin creation failed"),
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
admin:
  username: admin
  email: admin@tinnguyen-lab.com
  password: ${ZIEE_ADMIN_PASSWORD}
  mode: ensure
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

        // `mode` on admin/users parses (it is accepted-but-inert). Without the
        // field, `deny_unknown_fields` would fail the WHOLE document.
        assert_eq!(ds.admin.as_ref().unwrap().username, "admin");
        assert_eq!(ds.users.len(), 1);
        assert_eq!(ds.groups[0].remove.len(), 2);
        assert!(ds.groups[0].add.is_empty());
    }

    #[test]
    fn empty_document_is_a_legal_no_op() {
        let ds: DesiredState = serde_norway::from_str("{}").unwrap();
        assert!(ds.mcp_servers.is_empty());
        assert!(ds.admin.is_none());
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
        if let Some(admin) = &ds.admin {
            assert!(
                !matches!(
                    resolve_secret_with(&admin.password, &empty),
                    Err(TemplateError::InlineSecret)
                ),
                "the admin has an INLINE password in the shipped file"
            );
        }

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
    }
}
