//! Declarative seed provider for the `mcp_servers` section (admin/system servers).
//!
//! The built-in `fetch` server is baked by the mcp migration and ADOPTED here. This
//! provider CREATEs operator-declared system servers, converges their group assignments,
//! routes secret header/env values through the repo's three-column at-rest encryption, and
//! — under reconcile / `remove:` — deletes only ledger-owned servers. Ported onto the
//! `ziee-seed` SDK engine; system-server lookup-by-name is an inline query (this tree's
//! `McpRepository` exposes no `get_system_server_by_name`; a system server is `user_id IS
//! NULL`).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use linkme::distributed_slice;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::mcp::types::EnvVarEntry;
use crate::modules::mcp::{
    CreateMcpServerRequest, HeaderEntry, TransportType, UpdateMcpServerRequest, UsageMode,
};
use ziee_seed::{
    SeedCtx, SeedEntry, SeedError, SeedMode, SeedOutcome, SeedProvider, SeedSection, SEED_PROVIDERS,
};

const SECTION: &str = "mcp_servers";
const ASSIGN_SECTION: &str = "user_group_mcp_servers";

#[distributed_slice(SEED_PROVIDERS)]
static MCP_SERVERS_SEED: SeedEntry = SeedEntry {
    section: SECTION,
    order: 35,
    factory: || Arc::new(McpServersSeedProvider),
};

pub struct McpServersSeedProvider;

#[derive(Debug, Deserialize)]
struct ServerItem {
    name: String,
    display_name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    transport_type: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    supports_sampling: Option<bool>,
    #[serde(default)]
    timeout_seconds: Option<i32>,
    /// `{ header_name: ${ENV_VAR} }` — each value is a secret, encrypted at rest.
    #[serde(default)]
    headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    environment_variables: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    groups: Vec<String>,
}

impl ServerItem {
    fn transport(&self) -> TransportType {
        match self.transport_type.as_deref() {
            Some("stdio") => TransportType::Stdio,
            Some("http") | Some("sse") => TransportType::Http,
            _ if self.command.is_some() => TransportType::Stdio,
            _ => TransportType::Http,
        }
    }

    /// Resolve a `{k: ${VAR}}` secret map into encrypted-at-rest entries; an unset var
    /// drops that single entry (logged), never the whole server.
    fn resolve_headers(&self, ctx: &SeedCtx) -> Vec<HeaderEntry> {
        self.headers
            .iter()
            .filter_map(|(k, raw)| match ctx.resolve_secret(raw) {
                Ok(v) => Some(HeaderEntry { key: k.clone(), value: Some(v), is_secret: true }),
                Err(e) => {
                    tracing::warn!(server = %self.name, header = %k, reason = %e, "seed: mcp header env var not set; header omitted");
                    None
                }
            })
            .collect()
    }
    fn resolve_env(&self, ctx: &SeedCtx) -> Vec<EnvVarEntry> {
        self.environment_variables
            .iter()
            .filter_map(|(k, raw)| match ctx.resolve_secret(raw) {
                Ok(v) => Some(EnvVarEntry { key: k.clone(), value: Some(v), is_secret: true }),
                Err(e) => {
                    tracing::warn!(server = %self.name, var = %k, reason = %e, "seed: mcp env var not set; omitted");
                    None
                }
            })
            .collect()
    }
}

/// A system server has `user_id IS NULL`. This tree has no repo by-name accessor.
async fn find_system_server_by_name(pool: &sqlx::PgPool, name: &str) -> Result<Option<Uuid>, SeedError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM mcp_servers WHERE name = $1 AND user_id IS NULL ORDER BY created_at LIMIT 1",
        name
    )
    .fetch_optional(pool)
    .await?)
}

impl McpServersSeedProvider {
    async fn assign(&self, server_id: Uuid, item: &ServerItem, mode: SeedMode, ctx: &SeedCtx) -> Result<(), SeedError> {
        for gname in &item.groups {
            let Some(group) = Repos.group.get_by_name(gname).await? else {
                tracing::warn!(server = %item.name, group = %gname, "seed: group not found; server not assigned");
                continue;
            };
            Repos.mcp.assign_to_group(group.id, server_id).await?;
            ctx.ledger
                .record(ASSIGN_SECTION, &format!("{}:{}", item.name, gname), Some(group.id))
                .await?;
        }

        // Reconcile-revoke: a seed-owned assignment for THIS server no longer in the declared
        // group list is revoked (seed-if-empty stays additive). Enumerate the server's ACTUAL
        // current assignments by server_id, and revoke only seed-owned + undeclared ones — an
        // admin-made assignment is untouched.
        if mode == SeedMode::Reconcile {
            let declared: HashSet<&str> = item.groups.iter().map(String::as_str).collect();
            let current = sqlx::query!(
                "SELECT g.id, g.name FROM user_group_mcp_servers ugm \
                 JOIN groups g ON g.id = ugm.group_id WHERE ugm.mcp_server_id = $1",
                server_id
            )
            .fetch_all(&ctx.pool)
            .await?;
            for row in current {
                if declared.contains(row.name.as_str()) {
                    continue;
                }
                let key = format!("{}:{}", item.name, row.name);
                if ctx.ledger.is_seeded(ASSIGN_SECTION, &key).await? {
                    Repos.mcp.remove_from_group(row.id, server_id).await?;
                    ctx.ledger.remove(ASSIGN_SECTION, &key).await?;
                }
            }
        }
        Ok(())
    }

    async fn delete_owned(&self, name: &str, id: Option<Uuid>, ctx: &SeedCtx, o: &mut SeedOutcome) -> Result<(), SeedError> {
        if let Some(id) = id {
            Repos.mcp.delete_system_server(id).await?;
        }
        ctx.ledger.remove(SECTION, name).await?;
        for a in ctx.ledger.list_owned(ASSIGN_SECTION).await? {
            if a.natural_key.starts_with(&format!("{name}:")) {
                ctx.ledger.remove(ASSIGN_SECTION, &a.natural_key).await?;
            }
        }
        o.deleted += 1;
        Ok(())
    }
}

#[async_trait]
impl SeedProvider for McpServersSeedProvider {
    fn section(&self) -> &'static str {
        SECTION
    }

    async fn apply(
        &self,
        section: Option<&SeedSection>,
        mode: SeedMode,
        ctx: &SeedCtx,
    ) -> Result<SeedOutcome, SeedError> {
        let mut outcome = SeedOutcome::default();
        let Some(section) = section else { return Ok(outcome) };
        let mut declared: HashSet<String> = HashSet::new();

        for raw in &section.items {
            let item: ServerItem = match serde_norway::from_value(raw.clone()) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(section = SECTION, error = %e, "seed: skipping malformed mcp_servers item");
                    outcome.skipped += 1;
                    continue;
                }
            };
            declared.insert(item.name.clone());

            let existing = find_system_server_by_name(&ctx.pool, &item.name).await?;
            let seeded = ctx.ledger.is_seeded(SECTION, &item.name).await?;

            // A once-seeded default the admin later DELETED must NOT be resurrected by
            // seed-if-empty (ledger-gated); only reconcile re-creates it.
            if existing.is_none() && seeded && mode != SeedMode::Reconcile {
                outcome.skipped += 1;
                continue;
            }

            let headers = item.resolve_headers(ctx);
            let envs = item.resolve_env(ctx);

            let server_id = match (existing, seeded) {
                (Some(id), true) => {
                    if mode == SeedMode::Reconcile {
                        Repos.mcp
                            .update_system_server(
                                id,
                                UpdateMcpServerRequest {
                                    name: None,
                                    display_name: Some(item.display_name.clone()),
                                    description: item.description.clone(),
                                    enabled: item.enabled,
                                    command: item.command.clone(),
                                    args: item.args.clone(),
                                    environment_variables_entries: if envs.is_empty() { None } else { Some(envs) },
                                    url: item.url.clone(),
                                    headers_entries: if headers.is_empty() { None } else { Some(headers) },
                                    timeout_seconds: item.timeout_seconds,
                                    supports_sampling: item.supports_sampling,
                                    usage_mode: Some(UsageMode::Auto),
                                    max_concurrent_sessions: None,
                                    run_in_sandbox: None,
                                    sandbox_flavor: None,
                                },
                            )
                            .await?;
                        outcome.updated += 1;
                    } else {
                        outcome.skipped += 1;
                    }
                    id
                }
                (Some(id), false) => {
                    ctx.ledger.record(SECTION, &item.name, Some(id)).await?;
                    outcome.adopted += 1;
                    id
                }
                (None, _) => {
                    let created = Repos.mcp
                        .create_system_server(CreateMcpServerRequest {
                            name: item.name.clone(),
                            display_name: item.display_name.clone(),
                            description: item.description.clone(),
                            enabled: item.enabled,
                            transport_type: item.transport(),
                            command: item.command.clone(),
                            args: item.args.clone(),
                            environment_variables_entries: if envs.is_empty() { None } else { Some(envs) },
                            url: item.url.clone(),
                            headers_entries: if headers.is_empty() { None } else { Some(headers) },
                            timeout_seconds: item.timeout_seconds,
                            supports_sampling: item.supports_sampling,
                            usage_mode: Some(UsageMode::Auto),
                            max_concurrent_sessions: None,
                            run_in_sandbox: None,
                            sandbox_flavor: None,
                            hub_id: None,
                        })
                        .await?;
                    ctx.ledger.record(SECTION, &item.name, Some(created.id)).await?;
                    outcome.created += 1;
                    created.id
                }
            };

            self.assign(server_id, &item, mode, ctx).await?;
        }

        for name in &section.remove {
            if let Some(row) = ctx.ledger.lookup(SECTION, name).await? {
                self.delete_owned(name, row.entity_id, ctx, &mut outcome).await?;
            }
        }
        if mode == SeedMode::Reconcile {
            for owned in ctx.ledger.list_owned(SECTION).await? {
                if !declared.contains(&owned.natural_key) {
                    self.delete_owned(&owned.natural_key, owned.entity_id, ctx, &mut outcome).await?;
                }
            }
        }
        Ok(outcome)
    }

    async fn dump(&self, ctx: &SeedCtx) -> Result<Option<serde_norway::Value>, SeedError> {
        let owned = ctx.ledger.list_owned(SECTION).await?;
        if owned.is_empty() {
            return Ok(None);
        }
        let mut items = Vec::new();
        for row in owned {
            let Some(id) = row.entity_id else { continue };
            let r = sqlx::query!(
                "SELECT name, display_name, description, url, enabled, supports_sampling, timeout_seconds, headers_secret_keys FROM mcp_servers WHERE id = $1",
                id
            )
            .fetch_optional(&ctx.pool)
            .await?;
            let Some(r) = r else { continue };
            let mut obj = serde_json::Map::new();
            obj.insert("name".into(), r.name.clone().into());
            obj.insert("display_name".into(), r.display_name.into());
            if let Some(d) = r.description { obj.insert("description".into(), d.into()); }
            if let Some(u) = r.url { obj.insert("url".into(), u.into()); }
            obj.insert("enabled".into(), r.enabled.into());
            // Secret headers emitted as ${…} placeholders, never values.
            let keys = r.headers_secret_keys;
            if !keys.is_empty() {
                let hdrs: serde_json::Map<String, serde_json::Value> = keys
                    .iter()
                    .map(|k| (k.clone(), serde_json::Value::from(format!("${{{}}}", k.to_uppercase()))))
                    .collect();
                obj.insert("headers".into(), serde_json::Value::Object(hdrs));
            }
            let groups: Vec<String> = sqlx::query_scalar!(
                "SELECT g.name FROM user_group_mcp_servers ugm JOIN groups g ON g.id = ugm.group_id WHERE ugm.mcp_server_id = $1 ORDER BY g.name",
                id
            )
            .fetch_all(&ctx.pool)
            .await?;
            if !groups.is_empty() {
                obj.insert("groups".into(), serde_json::Value::Array(groups.into_iter().map(Into::into).collect()));
            }
            items.push(serde_json::Value::Object(obj));
        }
        Ok(Some(serde_norway::to_value(serde_json::json!({ "items": items }))
            .map_err(|e| SeedError::Other(e.to_string()))?))
    }
}
