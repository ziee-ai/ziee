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
//! - **Secrets never inline.** Any value may contain `${VAR}` placeholders that
//!   are resolved from process env at reconcile time. A *secret* field
//!   (`password`) MUST be exactly one `${VAR}` placeholder — an inline literal
//!   is rejected. Resolved values are never logged; logs name the env VAR only.
//! - **Idempotent.** Every entry is existence-checked against its natural key
//!   before writing (server → `(name, is_system)`; admin → "an admin exists";
//!   user → username/email; group → name). Re-running on the next deploy
//!   creates nothing and clobbers nothing.
//! - **Per-entry `mode`** (servers / admin / users): `ensure` (default) creates
//!   when absent and otherwise leaves the row completely alone — never
//!   clobbering an admin's later UI edit (the `seed_from_config_once` contract,
//!   `modules/auth/session_settings.rs`); `enforce` additionally re-syncs the
//!   declared fields to the file on every boot. Group permission entries carry
//!   no mode: a permission set has no create/update distinction, so it is
//!   always reconciled (see `GroupEntry`).
//! - **Never crashes boot.** A bad entry (unresolved env var, inline secret,
//!   DB error) logs an error and is skipped; an unparseable file skips the whole
//!   reconcile. The server always goes on to serve.
//!
//! Pattern mirrors: `modules/auth/session_settings.rs::seed_from_config_once`
//! (config → DB once, DB authoritative after) and
//! `desktop/tauri/src/modules/auth/bootstrap.rs::ensure_desktop_admin`
//! (boot-time admin creation via `Repos.app.create_admin_user`).

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

/// Substitute every `${VAR}` in `raw` with its process-env value.
///
/// A `$` not followed by `{` is left intact. An unset/empty var is an error
/// (the caller skips that entry) rather than a silent empty string — a server
/// silently registered at the URL "" would be worse than one that is absent.
pub fn resolve(raw: &str) -> Result<String, TemplateError> {
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
                        let value = std::env::var(name).unwrap_or_default();
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
pub fn resolve_secret(raw: &str) -> Result<String, TemplateError> {
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
    resolve(trimmed)
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

/// Apply the declared removals + additions to a permission array.
/// Returns the new array (caller compares against the old one to decide whether
/// a write is needed at all).
pub fn apply_permission_ops(current: &[String], remove: &[String], add: &[String]) -> Vec<String> {
    let mut next: Vec<String> = current
        .iter()
        .filter(|perm| !remove.iter().any(|pat| permission_matches(pat, perm)))
        .cloned()
        .collect();

    for perm in add {
        if !next.iter().any(|existing| existing == perm) {
            next.push(perm.clone());
        }
    }

    next
}

// ───────────────────────────── the reconciler ─────────────────────────────

/// Reconcile the desired-state file into the DB. Never returns an error: every
/// failure is logged and skipped so a bad manifest can't stop the server from
/// booting.
pub async fn reconcile(pool: &PgPool) {
    let Some(path) = std::env::var_os(DESIRED_STATE_ENV) else {
        tracing::debug!("desired_state: {DESIRED_STATE_ENV} unset; nothing to reconcile");
        return;
    };
    let path = std::path::PathBuf::from(path);

    if !path.exists() {
        tracing::info!(
            path = %path.display(),
            "desired_state: file not found; skipping reconcile"
        );
        return;
    }

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "desired_state: cannot read file; skipping reconcile");
            return;
        }
    };

    let desired: DesiredState = match serde_norway::from_str(&raw) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "desired_state: invalid YAML; skipping reconcile");
            return;
        }
    };

    tracing::info!(path = %path.display(), "desired_state: reconciling");

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
        reconcile_mcp_server(pool, server).await;
    }

    tracing::info!("desired_state: reconcile complete");
}

/// Create the root admin ONLY when no admin exists. A later boot never resets
/// the password — the account is the operator's after first boot.
async fn reconcile_admin(entry: &AdminEntry) {
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

    let password = match resolve_secret(&entry.password) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                username = %entry.username,
                reason = %e,
                "desired_state: admin not created (set the password env var to enable it)"
            );
            return;
        }
    };

    if let Err(reason) = password::validate_password_strength(&password) {
        tracing::error!(
            username = %entry.username,
            reason = %reason,
            "desired_state: admin password rejected; admin not created"
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

    let password = match resolve_secret(&entry.password) {
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

    let next = apply_permission_ops(&group.permissions, &entry.remove, &entry.add);
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
async fn reconcile_mcp_server(pool: &PgPool, entry: &McpServerEntry) {
    let url = match resolve(&entry.url) {
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
    let existing = match sqlx::query!(
        "SELECT id FROM mcp_servers WHERE name = $1 AND is_system = true",
        entry.name
    )
    .fetch_optional(pool)
    .await
    {
        Ok(row) => row.map(|r| r.id),
        Err(e) => {
            tracing::error!(error = %e, server = %entry.name, "desired_state: MCP server lookup failed; skipping");
            return;
        }
    };

    let server_id = match (existing, entry.mode) {
        // Already there, `ensure` → leave every field (and its group
        // assignments) exactly as the admin last left them.
        (Some(_id), Mode::Ensure) => {
            tracing::debug!(server = %entry.name, "desired_state: MCP server already present; untouched (ensure)");
            return;
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
                Err(e) => {
                    tracing::error!(error = ?e, server = %entry.name, "desired_state: MCP server update failed");
                    return;
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

        match Repos.mcp.assign_to_group(server_id, group.id).await {
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

    /// Env is process-global; keep the vars used here unique per test so a
    /// parallel run can't race.
    #[test]
    fn resolve_substitutes_env_vars() {
        unsafe { std::env::set_var("DS_TEST_URL", "http://rcpa:9000/mcp") };
        assert_eq!(
            resolve("${DS_TEST_URL}").unwrap(),
            "http://rcpa:9000/mcp".to_string()
        );
        // Surrounding literal text is preserved.
        assert_eq!(
            resolve("prefix ${DS_TEST_URL} suffix").unwrap(),
            "prefix http://rcpa:9000/mcp suffix".to_string()
        );
    }

    #[test]
    fn resolve_errors_on_unset_var() {
        let err = resolve("${DS_TEST_DEFINITELY_UNSET}").unwrap_err();
        assert_eq!(
            err,
            TemplateError::Unresolved("DS_TEST_DEFINITELY_UNSET".to_string())
        );
    }

    #[test]
    fn resolve_errors_on_empty_var() {
        unsafe { std::env::set_var("DS_TEST_EMPTY", "") };
        assert!(matches!(
            resolve("${DS_TEST_EMPTY}").unwrap_err(),
            TemplateError::Unresolved(_)
        ));
    }

    #[test]
    fn resolve_leaves_non_placeholder_dollars_intact() {
        assert_eq!(resolve("costs $5").unwrap(), "costs $5".to_string());
        assert_eq!(resolve("${}").unwrap(), "${}".to_string());
        // Unterminated `${` is literal, not an error.
        assert_eq!(resolve("a ${OPEN").unwrap(), "a ${OPEN".to_string());
    }

    #[test]
    fn resolve_secret_rejects_inline_literals() {
        unsafe { std::env::set_var("DS_TEST_PW", "s3cret-value") };

        // The only accepted shape: exactly one placeholder.
        assert_eq!(
            resolve_secret("${DS_TEST_PW}").unwrap(),
            "s3cret-value".to_string()
        );
        assert_eq!(
            resolve_secret("  ${DS_TEST_PW}  ").unwrap(),
            "s3cret-value".to_string()
        );

        // Everything else is a committed secret.
        assert_eq!(
            resolve_secret("hunter2").unwrap_err(),
            TemplateError::InlineSecret
        );
        assert_eq!(
            resolve_secret("prefix-${DS_TEST_PW}").unwrap_err(),
            TemplateError::InlineSecret
        );
        assert_eq!(
            resolve_secret("${DS_TEST_PW}${DS_TEST_PW}").unwrap_err(),
            TemplateError::InlineSecret
        );
        assert_eq!(resolve_secret("").unwrap_err(), TemplateError::InlineSecret);
    }

    #[test]
    fn resolve_secret_propagates_unset_var() {
        assert!(matches!(
            resolve_secret("${DS_TEST_PW_UNSET}").unwrap_err(),
            TemplateError::Unresolved(_)
        ));
    }

    #[test]
    fn permission_wildcard_matches_by_hierarchy() {
        assert!(permission_matches("hub::*", "hub::models::read"));
        assert!(permission_matches("hub::*", "hub::assistants::create"));
        assert!(permission_matches("hub::*", "hub"));
        // Must not match a different top-level segment that merely shares a prefix.
        assert!(!permission_matches("hub::*", "hubris::read"));
        assert!(!permission_matches("hub::*", "chat::read"));
        // Non-wildcard patterns are exact.
        assert!(permission_matches("assistants::read", "assistants::read"));
        assert!(!permission_matches("assistants::read", "assistants::edit"));
    }

    #[test]
    fn apply_permission_ops_removes_hides_and_keeps_the_rest() {
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

        let next = apply_permission_ops(
            &current,
            &[
                "assistants::*".to_string(),
                "hub::*".to_string(),
                "projects::*".to_string(),
            ],
            &[],
        );

        // The hidden features are gone …
        assert!(!next.iter().any(|p| p.starts_with("assistants::")));
        assert!(!next.iter().any(|p| p.starts_with("hub::")));
        assert!(!next.iter().any(|p| p.starts_with("projects::")));
        // … and the KEEP set survives untouched.
        assert_eq!(
            next,
            vec![
                "profile::read".to_string(),
                "chat::read".to_string(),
                "mcp_servers::read".to_string(),
                "user_llm_providers::read".to_string(),
            ]
        );
    }

    #[test]
    fn apply_permission_ops_add_is_idempotent() {
        let current = vec!["chat::read".to_string()];

        let next = apply_permission_ops(
            &current,
            &[],
            &["chat::read".to_string(), "chat::create".to_string()],
        );
        assert_eq!(
            next,
            vec!["chat::read".to_string(), "chat::create".to_string()]
        );

        // Re-applying the same ops changes nothing (the caller uses this
        // equality to skip the DB write entirely).
        let again = apply_permission_ops(
            &next,
            &[],
            &["chat::read".to_string(), "chat::create".to_string()],
        );
        assert_eq!(again, next);
    }

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
        // deny_unknown_fields: a typo in the deploy manifest must fail loudly,
        // not be silently ignored.
        let yaml = r#"
mcp_servers:
  - name: rcpa
    display_name: RCPA
    url: http://x/mcp
    timout_seconds: 300
"#;
        assert!(serde_norway::from_str::<DesiredState>(yaml).is_err());
    }

    /// The file that is actually baked into the container image must parse,
    /// and every secret in it must be an env placeholder (TEST-17).
    #[test]
    fn shipped_desired_state_file_is_valid() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/desired-state.yaml");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

        let ds: DesiredState = serde_norway::from_str(&raw)
            .unwrap_or_else(|e| panic!("shipped desired-state.yaml is invalid: {e}"));

        // Secrets are placeholders, never literals.
        for user in &ds.users {
            assert!(
                matches!(
                    resolve_secret(&user.password),
                    Err(TemplateError::Unresolved(_)) | Ok(_)
                ),
                "user {} has an INLINE password in the shipped file",
                user.username
            );
        }
        if let Some(admin) = &ds.admin {
            assert!(
                !matches!(
                    resolve_secret(&admin.password),
                    Err(TemplateError::InlineSecret)
                ),
                "admin has an INLINE password in the shipped file"
            );
        }

        // The three org servers are declared, each reachable by the Users group.
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
        }

        // The Users group trims exactly the three hidden features.
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
    }
}
