// MCP repository
#![allow(dead_code)]

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::common::secret::{decrypt_secret, encrypt_secret};

use super::models::{
    EnvVarView, HeaderView, McpServer, McpServerOAuthConfig, SetMcpServerOAuthConfigRequest,
    TransportType, UsageMode,
};
use super::types::{
    CreateMcpServerRequest, EnvVarEntry, HeaderEntry, McpServerListResponse,
    UpdateMcpServerRequest,
};

// =====================================================
// Secret-aware env-var / header storage helpers
//
// Per migration 81, env vars and headers each live in THREE columns:
//   - `<col>`            JSONB plain map (non-secret entries)
//   - `<col>_encrypted`  JSONB map of {key: base64(pgp_sym_encrypt(value))}
//   - `<col>_secret_keys` TEXT[] denormalized list of secret key names
//
// These helpers split request entries into the three components on
// write, and assemble entries (+ a flat decrypted map for runtime
// callsites) on read.
//
// Type-erased over `(key, value, is_secret)` tuples so ONE pair of
// helpers serves both env vars and headers.
// =====================================================

/// Inbound entry from a create/update request, type-erased so the same
/// helper handles env vars and headers.
struct EntryRef<'a> {
    key: &'a str,
    value: Option<&'a str>,
    is_secret: bool,
}

impl<'a> From<&'a EnvVarEntry> for EntryRef<'a> {
    fn from(e: &'a EnvVarEntry) -> Self {
        Self { key: &e.key, value: e.value.as_deref(), is_secret: e.is_secret }
    }
}

impl<'a> From<&'a HeaderEntry> for EntryRef<'a> {
    fn from(e: &'a HeaderEntry) -> Self {
        Self { key: &e.key, value: e.value.as_deref(), is_secret: e.is_secret }
    }
}

/// Split a request's entries into the (plain, encrypted, secret_keys)
/// trio for storage. `prior_encrypted` is the existing `<col>_encrypted`
/// JSONB from the row being updated (None on create) — used to
/// preserve secrets the user didn't touch (entries with `value: None`
/// + `is_secret: true`).
///
/// Contract for the four (is_secret, value) combinations:
///   * `(true,  Some(v))` → encrypt v into encrypted_map (or store
///     plaintext utf8 as a fallback when storage_key is unset for
///     dev parity); key added to secret_keys.
///   * `(true,  None)` → carry forward `prior_encrypted[key]` if any;
///     key added to secret_keys regardless (the entry is still a
///     secret, just unchanged).
///   * `(false, Some(v))` → store v in plain_map; key NOT in secret_keys.
///   * `(false, None)` → store empty string in plain_map (explicit
///     clear); key NOT in secret_keys. UI flips this toggle by
///     clearing the password input simultaneously.
async fn split_entries_for_storage(
    pool: &PgPool,
    entries: Vec<EntryRef<'_>>,
    prior_encrypted: Option<&serde_json::Value>,
) -> Result<(serde_json::Value, serde_json::Value, Vec<String>), AppError> {
    let storage_key = crate::core::secrets::storage_key();
    let mut plain_map = serde_json::Map::new();
    let mut enc_map = serde_json::Map::new();
    let mut secret_keys: Vec<String> = Vec::new();

    for entry in entries {
        if entry.is_secret {
            secret_keys.push(entry.key.to_string());
            match entry.value {
                Some(v) => {
                    let bytes = match encrypt_secret(pool, v, storage_key).await? {
                        Some(b) => b,
                        // Dev parity — no storage key configured. Store
                        // utf8 bytes verbatim so the read path's
                        // dev-fallback branch round-trips cleanly.
                        None => v.as_bytes().to_vec(),
                    };
                    enc_map.insert(
                        entry.key.to_string(),
                        serde_json::Value::String(B64.encode(&bytes)),
                    );
                }
                None => {
                    if let Some(prior_val) =
                        prior_encrypted.and_then(|v| v.as_object()).and_then(|o| o.get(entry.key))
                    {
                        enc_map.insert(entry.key.to_string(), prior_val.clone());
                    }
                }
            }
        } else {
            plain_map.insert(
                entry.key.to_string(),
                serde_json::Value::String(entry.value.unwrap_or("").to_string()),
            );
        }
    }

    Ok((
        serde_json::Value::Object(plain_map),
        serde_json::Value::Object(enc_map),
        secret_keys,
    ))
}

/// Read-side counterpart to `split_entries_for_storage`. Decrypts the
/// `<col>_encrypted` map and merges with the plain map into ONE flat
/// `serde_json::Value` (consumed by internal runtime callsites — stdio
/// spawn env, header `${VAR}` interpolation), AND produces a redacted
/// Vec of `(key, value, is_secret)` tuples for the API response (where
/// secret values are replaced with `None`).
///
/// Ordering: non-secret entries first (alphabetical for stability),
/// then secret entries (alphabetical) so the form editor in the UI
/// renders them in a predictable order across reloads.
async fn assemble_entries_from_storage(
    pool: &PgPool,
    plain: &serde_json::Value,
    encrypted: &serde_json::Value,
    secret_keys: &[String],
) -> Result<(serde_json::Value, Vec<(String, Option<String>, bool)>), AppError> {
    let storage_key = crate::core::secrets::storage_key();
    let plain_obj = plain.as_object().cloned().unwrap_or_default();
    let enc_obj = encrypted.as_object();

    let mut runtime_map = serde_json::Map::new();
    let mut views: Vec<(String, Option<String>, bool)> = Vec::new();

    // Non-secret entries: stable alphabetical order.
    let mut plain_keys: Vec<&String> = plain_obj
        .keys()
        .filter(|k| !secret_keys.iter().any(|s| s == *k))
        .collect();
    plain_keys.sort();
    for key in plain_keys {
        let value_str = plain_obj
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_default();
        runtime_map.insert(key.clone(), serde_json::Value::String(value_str.clone()));
        views.push((key.clone(), Some(value_str), false));
    }

    // Secret entries: decrypt for the runtime map, redact for the view.
    let mut sk_sorted = secret_keys.to_vec();
    sk_sorted.sort();
    sk_sorted.dedup();
    for key in &sk_sorted {
        let b64 = enc_obj
            .and_then(|o| o.get(key))
            .and_then(|v| v.as_str());
        let decrypted = match b64 {
            Some(b64_str) => match B64.decode(b64_str) {
                Ok(bytes) => match storage_key {
                    Some(sk) => decrypt_secret(pool, &bytes, sk).await.unwrap_or_else(|e| {
                        tracing::error!(
                            error = ?e,
                            key = %key,
                            "Failed to decrypt MCP secret column; runtime will see empty value"
                        );
                        String::new()
                    }),
                    // Dev parity — `split_entries_for_storage` stored
                    // utf8 bytes verbatim when storage_key was unset.
                    None => String::from_utf8(bytes).unwrap_or_default(),
                },
                Err(e) => {
                    tracing::error!(error = ?e, key = %key, "Invalid base64 in MCP encrypted column");
                    String::new()
                }
            },
            None => String::new(),
        };
        runtime_map.insert(key.clone(), serde_json::Value::String(decrypted));
        views.push((key.clone(), None, true));
    }

    Ok((serde_json::Value::Object(runtime_map), views))
}

fn views_to_env(views: Vec<(String, Option<String>, bool)>) -> Vec<EnvVarView> {
    views
        .into_iter()
        .map(|(key, value, is_secret)| EnvVarView { key, value, is_secret })
        .collect()
}

fn views_to_headers(views: Vec<(String, Option<String>, bool)>) -> Vec<HeaderView> {
    views
        .into_iter()
        .map(|(key, value, is_secret)| HeaderView { key, value, is_secret })
        .collect()
}

/// Raw column values for one MCP server row — decouples the SELECT
/// SQL (which varies per WHERE clause across get_*, list_*, etc.)
/// from the in-memory assembly + decrypt step (which is identical).
/// Each SELECT site unpacks its sqlx::query! row into this struct,
/// then calls `assemble_mcp_server` to get a fully-populated
/// `McpServer` value.
pub(crate) struct McpServerColumnsRaw {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub is_system: bool,
    pub is_built_in: bool,
    pub transport_type: String,
    pub command: Option<String>,
    pub args: Option<serde_json::Value>,
    pub environment_variables: Option<serde_json::Value>,
    pub environment_variables_encrypted: serde_json::Value,
    pub environment_variables_secret_keys: Vec<String>,
    pub url: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub headers_encrypted: serde_json::Value,
    pub headers_secret_keys: Vec<String>,
    pub timeout_seconds: i32,
    pub supports_sampling: bool,
    pub usage_mode: String,
    pub max_concurrent_sessions: Option<i32>,
    pub run_in_sandbox: bool,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

pub(crate) async fn assemble_mcp_server(
    pool: &PgPool,
    raw: McpServerColumnsRaw,
) -> Result<McpServer, AppError> {
    let env_plain = raw
        .environment_variables
        .unwrap_or_else(|| serde_json::json!({}));
    let env_enc = raw.environment_variables_encrypted;
    let env_secret_keys = raw.environment_variables_secret_keys;
    let (env_runtime, env_views) =
        assemble_entries_from_storage(pool, &env_plain, &env_enc, &env_secret_keys).await?;

    let hdr_plain = raw.headers.unwrap_or_else(|| serde_json::json!({}));
    let hdr_enc = raw.headers_encrypted;
    let hdr_secret_keys = raw.headers_secret_keys;
    let (hdr_runtime, hdr_views) =
        assemble_entries_from_storage(pool, &hdr_plain, &hdr_enc, &hdr_secret_keys).await?;

    Ok(McpServer {
        id: raw.id,
        user_id: raw.user_id,
        name: raw.name,
        display_name: raw.display_name,
        description: raw.description,
        enabled: raw.enabled,
        is_system: raw.is_system,
        is_built_in: raw.is_built_in,
        transport_type: TransportType::from_str(&raw.transport_type)?,
        command: raw.command,
        args: raw.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: env_runtime,
        environment_variables_entries: views_to_env(env_views),
        url: raw.url,
        headers: hdr_runtime,
        headers_entries: views_to_headers(hdr_views),
        timeout_seconds: raw.timeout_seconds,
        supports_sampling: raw.supports_sampling,
        usage_mode: UsageMode::from_str(&raw.usage_mode)?,
        max_concurrent_sessions: raw.max_concurrent_sessions,
        run_in_sandbox: raw.run_in_sandbox,
        created_at: DateTime::from_timestamp(raw.created_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid created_at timestamp"))?,
        updated_at: DateTime::from_timestamp(raw.updated_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid updated_at timestamp"))?,
    })
}

/// MCP Repository
pub struct McpRepository {
    pool: PgPool,
}

impl McpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the underlying pool — used by the connection-health
    /// module to run direct SQL against the same DB without taking
    /// a new PgPool handle.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // User server operations
    pub async fn create_user_server(
        &self,
        user_id: Uuid,
        request: CreateMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        create_user_mcp_server(&self.pool, user_id, request).await
    }

    pub async fn get_user_server(
        &self,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<McpServer>, AppError> {
        get_user_mcp_server(&self.pool, id, user_id).await
    }

    pub async fn list_user_servers(
        &self,
        user_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<McpServerListResponse, AppError> {
        let (servers, total) = list_user_mcp_servers(&self.pool, user_id, page, per_page).await?;
        let total_pages = (total + per_page - 1) / per_page;
        Ok(McpServerListResponse {
            servers,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    pub async fn update_user_server(
        &self,
        id: Uuid,
        user_id: Uuid,
        request: UpdateMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        update_user_mcp_server(&self.pool, id, user_id, request).await
    }

    pub async fn delete_user_server(&self, id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        delete_user_mcp_server(&self.pool, id, user_id).await
    }

    // System server operations
    pub async fn create_system_server(
        &self,
        request: CreateMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        create_system_mcp_server(&self.pool, request).await
    }

    pub async fn get_any_server(&self, id: Uuid) -> Result<Option<McpServer>, AppError> {
        get_any_mcp_server(&self.pool, id).await
    }

    /// All `enabled = true` MCP servers that are NOT built-in.
    /// Used by the boot-time connection-health check to probe every
    /// user-configurable enabled server (built-in ones are owned by
    /// their respective modules — code_sandbox, memory_mcp — and
    /// don't need this enforcement).
    pub async fn list_enabled_for_health_check(&self) -> Result<Vec<McpServer>, AppError> {
        list_enabled_for_health_check(&self.pool).await
    }

    // OAuth client_credentials config (external HTTP servers; Phase 4)
    pub async fn get_oauth_config(
        &self,
        server_id: Uuid,
    ) -> Result<Option<McpServerOAuthConfig>, AppError> {
        get_mcp_server_oauth_config(&self.pool, server_id).await
    }

    pub async fn set_oauth_config(
        &self,
        server_id: Uuid,
        request: SetMcpServerOAuthConfigRequest,
    ) -> Result<McpServerOAuthConfig, AppError> {
        set_mcp_server_oauth_config(&self.pool, server_id, request).await
    }

    pub async fn delete_oauth_config(&self, server_id: Uuid) -> Result<(), AppError> {
        delete_mcp_server_oauth_config(&self.pool, server_id).await
    }

    pub async fn get_system_server(&self, id: Uuid) -> Result<Option<McpServer>, AppError> {
        get_system_mcp_server(&self.pool, id).await
    }

    pub async fn list_system_servers(
        &self,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<McpServerListResponse, AppError> {
        let (servers, total) =
            list_system_mcp_servers(&self.pool, page, per_page, search, enabled).await?;
        let total_pages = (total + per_page - 1) / per_page;
        Ok(McpServerListResponse {
            servers,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    pub async fn update_system_server(
        &self,
        id: Uuid,
        request: UpdateMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        update_system_mcp_server(&self.pool, id, request).await
    }

    pub async fn delete_system_server(&self, id: Uuid) -> Result<(), AppError> {
        delete_system_mcp_server(&self.pool, id).await
    }

    // Group assignment operations
    pub async fn get_group_mcp_servers(&self, group_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        get_group_mcp_servers(&self.pool, group_id).await
    }

    pub async fn get_system_servers_for_group(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<McpServer>, AppError> {
        get_system_servers_for_group(&self.pool, group_id).await
    }

    pub async fn assign_to_group(&self, server_id: Uuid, group_id: Uuid) -> Result<(), AppError> {
        assign_mcp_server_to_group(&self.pool, server_id, group_id).await
    }

    pub async fn remove_from_group(&self, server_id: Uuid, group_id: Uuid) -> Result<(), AppError> {
        remove_mcp_server_from_group(&self.pool, server_id, group_id).await
    }

    pub async fn set_group_servers(
        &self,
        group_id: Uuid,
        server_ids: Vec<Uuid>,
    ) -> Result<(), AppError> {
        set_group_mcp_servers(&self.pool, group_id, server_ids).await
    }

    pub async fn get_server_groups(&self, server_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        get_server_groups(&self.pool, server_id).await
    }

    pub async fn set_server_groups(
        &self,
        server_id: Uuid,
        group_ids: Vec<Uuid>,
    ) -> Result<(), AppError> {
        set_server_groups(&self.pool, server_id, group_ids).await
    }

    // List accessible servers
    pub async fn list_accessible(
        &self,
        user_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        enabled: Option<bool>,
        is_system: Option<bool>,
    ) -> Result<McpServerListResponse, AppError> {
        let (servers, total) = list_accessible_mcp_servers(
            &self.pool, user_id, page, per_page, search, enabled, is_system,
        )
        .await?;
        let total_pages = (total + per_page - 1) / per_page;
        Ok(McpServerListResponse {
            servers,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    // Check if user has access to a server
    pub async fn can_user_access_server(&self, user_id: Uuid, server_id: Uuid) -> Result<bool, AppError> {
        // Check if user owns this server
        let user_server = self.get_user_server(server_id, user_id).await?;
        if user_server.is_some() {
            return Ok(true);
        }

        // Check if user has access via system server and groups
        // Get user's groups
        let user_groups = sqlx::query!(
            "SELECT group_id FROM user_groups WHERE user_id = $1",
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        let group_ids: Vec<Uuid> = user_groups.iter().map(|r| r.group_id).collect();

        if group_ids.is_empty() {
            return Ok(false);
        }

        // Check if any group has access to this system server
        let has_access = sqlx::query!(
            "SELECT EXISTS(
                SELECT 1 FROM user_group_mcp_servers
                WHERE mcp_server_id = $1
                AND group_id = ANY($2)
            ) as has_access",
            server_id,
            &group_ids
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(has_access.has_access.unwrap_or(false))
    }
}

// =====================================================
// User Server Operations
// =====================================================

/// Create a new user MCP server
pub async fn create_user_mcp_server(
    pool: &PgPool,
    user_id: Uuid,
    request: CreateMcpServerRequest,
) -> Result<McpServer, AppError> {
    // Validate transport-specific fields
    validate_transport_config(&request.transport_type, &request)?;

    let args = serde_json::to_value(request.args.clone().unwrap_or_default())
        .map_err(|e| AppError::internal_error(format!("Failed to serialize args: {}", e)))?;

    let env_entries: Vec<EnvVarEntry> = request
        .environment_variables_entries
        .clone()
        .unwrap_or_default();
    let header_entries: Vec<HeaderEntry> = request.headers_entries.clone().unwrap_or_default();
    validate_header_entries(&header_entries)?;
    let (env_plain, env_enc, env_secret_keys) =
        split_entries_for_storage(pool, env_entries.iter().map(|e| EntryRef::from(e)).collect(), None).await?;
    let (hdr_plain, hdr_enc, hdr_secret_keys) =
        split_entries_for_storage(pool, header_entries.iter().map(|e| EntryRef::from(e)).collect(), None).await?;

    let supports_sampling = request.supports_sampling.unwrap_or(false);
    let usage_mode = request.usage_mode.clone().unwrap_or(UsageMode::Auto);

    let row = sqlx::query!(
        r#"
        INSERT INTO mcp_servers (
            user_id, name, display_name, description,
            transport_type, command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds, enabled, is_system,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, false,
                $17, $18, $19, false)
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        "#,
        user_id,
        request.name,
        request.display_name,
        request.description,
        request.transport_type.to_string(),
        request.command,
        args,
        env_plain,
        env_enc,
        &env_secret_keys,
        request.url,
        hdr_plain,
        hdr_enc,
        &hdr_secret_keys,
        request.timeout_seconds.unwrap_or(30) as i32,
        request.enabled.unwrap_or(false),
        supports_sampling,
        usage_mode.to_string(),
        request.max_concurrent_sessions,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e
            && db_err.is_unique_violation() {
                return AppError::conflict("Server name");
            }
        AppError::from(e)
    })?;

    let raw = McpServerColumnsRaw {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: row.transport_type,
        command: row.command,
        args: row.args,
        environment_variables: row.environment_variables,
        environment_variables_encrypted: row.environment_variables_encrypted,
        environment_variables_secret_keys: row.environment_variables_secret_keys,
        url: row.url,
        headers: row.headers,
        headers_encrypted: row.headers_encrypted,
        headers_secret_keys: row.headers_secret_keys,
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: row.usage_mode,
        max_concurrent_sessions: row.max_concurrent_sessions,
        run_in_sandbox: row.run_in_sandbox,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };
    assemble_mcp_server(pool, raw).await
}

/// Get user MCP server by ID (must be owned by user)
pub async fn get_user_mcp_server(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<McpServer>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        FROM mcp_servers
        WHERE id = $1 AND user_id = $2 AND is_system = false
        "#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let raw = McpServerColumnsRaw {
                id: r.id,
                user_id: r.user_id,
                name: r.name,
                display_name: r.display_name,
                description: r.description,
                enabled: r.enabled,
                is_system: r.is_system,
                is_built_in: r.is_built_in,
                transport_type: r.transport_type,
                command: r.command,
                args: r.args,
                environment_variables: r.environment_variables,
                environment_variables_encrypted: r.environment_variables_encrypted,
                environment_variables_secret_keys: r.environment_variables_secret_keys,
                url: r.url,
                headers: r.headers,
                headers_encrypted: r.headers_encrypted,
                headers_secret_keys: r.headers_secret_keys,
                timeout_seconds: r.timeout_seconds,
                supports_sampling: r.supports_sampling,
                usage_mode: r.usage_mode,
                max_concurrent_sessions: r.max_concurrent_sessions,
                run_in_sandbox: r.run_in_sandbox,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            Ok(Some(assemble_mcp_server(pool, raw).await?))
        }
        None => Ok(None),
    }
}

/// List user's own MCP servers with pagination
pub async fn list_user_mcp_servers(
    pool: &PgPool,
    user_id: Uuid,
    page: i64,
    per_page: i64,
) -> Result<(Vec<McpServer>, i64), AppError> {
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        FROM mcp_servers
        WHERE user_id = $1 AND is_system = false
        ORDER BY display_name ASC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        per_page,
        offset
    )
    .fetch_all(pool)
    .await?;

    let mut servers: Vec<McpServer> = Vec::with_capacity(rows.len());
    for r in rows {
        let raw = McpServerColumnsRaw {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: r.transport_type,
            command: r.command,
            args: r.args,
            environment_variables: r.environment_variables,
            environment_variables_encrypted: r.environment_variables_encrypted,
            environment_variables_secret_keys: r.environment_variables_secret_keys,
            url: r.url,
            headers: r.headers,
            headers_encrypted: r.headers_encrypted,
            headers_secret_keys: r.headers_secret_keys,
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: r.usage_mode,
            max_concurrent_sessions: r.max_concurrent_sessions,
            run_in_sandbox: r.run_in_sandbox,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        servers.push(assemble_mcp_server(pool, raw).await?);
    }

    let total = sqlx::query!(
        "SELECT COUNT(*) as count FROM mcp_servers WHERE user_id = $1 AND is_system = false",
        user_id
    )
    .fetch_one(pool)
    .await?
    .count
    .unwrap_or(0);

    Ok((servers, total))
}

/// All `enabled = true` MCP servers that are NOT built-in. No
/// pagination, no ordering — the boot health check iterates them
/// once and the dataset is small (typically <100). Built-in servers
/// (filesystem, memory_mcp, code_sandbox) are owned by their
/// modules and their reachability isn't gated on this column.
pub async fn list_enabled_for_health_check(pool: &PgPool) -> Result<Vec<McpServer>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        FROM mcp_servers
        WHERE enabled = true AND is_built_in = false
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut servers: Vec<McpServer> = Vec::with_capacity(rows.len());
    for r in rows {
        let raw = McpServerColumnsRaw {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: r.transport_type,
            command: r.command,
            args: r.args,
            environment_variables: r.environment_variables,
            environment_variables_encrypted: r.environment_variables_encrypted,
            environment_variables_secret_keys: r.environment_variables_secret_keys,
            url: r.url,
            headers: r.headers,
            headers_encrypted: r.headers_encrypted,
            headers_secret_keys: r.headers_secret_keys,
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: r.usage_mode,
            max_concurrent_sessions: r.max_concurrent_sessions,
            run_in_sandbox: r.run_in_sandbox,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        servers.push(assemble_mcp_server(pool, raw).await?);
    }
    Ok(servers)
}

/// Update user MCP server
pub async fn update_user_mcp_server(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    request: UpdateMcpServerRequest,
) -> Result<McpServer, AppError> {
    // Get the existing server to validate transport type
    let existing = get_user_mcp_server(pool, id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Validate transport-specific updates
    validate_transport_update(&existing.transport_type, &request)?;

    // Fetch the raw prior columns so we can pass `*_encrypted` JSON
    // into `split_entries_for_storage` (preserves untouched secrets)
    // and reuse the prior columns directly when the request omits the
    // entries field.
    let prior = sqlx::query!(
        r#"
        SELECT
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            headers, headers_encrypted, headers_secret_keys
        FROM mcp_servers
        WHERE id = $1 AND user_id = $2 AND is_system = false
        "#,
        id,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::RowNotFound = e {
            return AppError::not_found("Server");
        }
        AppError::from(e)
    })?;

    let args = request.args.and_then(|a| serde_json::to_value(a).ok());

    // env: if request supplied entries, split; otherwise reuse prior columns
    let (env_plain, env_enc, env_secret_keys) = match &request.environment_variables_entries {
        Some(entries) => {
            split_entries_for_storage(
                pool,
                entries.iter().map(|e| EntryRef::from(e)).collect(),
                Some(&prior.environment_variables_encrypted),
            )
            .await?
        }
        None => (
            prior
                .environment_variables
                .clone()
                .unwrap_or_else(|| serde_json::json!({})),
            prior.environment_variables_encrypted.clone(),
            prior.environment_variables_secret_keys.clone(),
        ),
    };

    // headers: same pattern, with validation upfront when supplied
    let (hdr_plain, hdr_enc, hdr_secret_keys) = match &request.headers_entries {
        Some(entries) => {
            validate_header_entries(entries)?;
            split_entries_for_storage(
                pool,
                entries.iter().map(|e| EntryRef::from(e)).collect(),
                Some(&prior.headers_encrypted),
            )
            .await?
        }
        None => (
            prior.headers.clone().unwrap_or_else(|| serde_json::json!({})),
            prior.headers_encrypted.clone(),
            prior.headers_secret_keys.clone(),
        ),
    };

    let row = sqlx::query!(
        r#"
        UPDATE mcp_servers SET
            name = COALESCE($3, name),
            display_name = COALESCE($4, display_name),
            description = COALESCE($5, description),
            enabled = COALESCE($6, enabled),
            command = COALESCE($7, command),
            args = COALESCE($8, args),
            environment_variables = $9,
            environment_variables_encrypted = $10,
            environment_variables_secret_keys = $11,
            url = COALESCE($12, url),
            headers = $13,
            headers_encrypted = $14,
            headers_secret_keys = $15,
            timeout_seconds = COALESCE($16, timeout_seconds),
            supports_sampling = COALESCE($17, supports_sampling),
            usage_mode = COALESCE($18, usage_mode),
            max_concurrent_sessions = COALESCE($19, max_concurrent_sessions),
            run_in_sandbox = COALESCE($20, run_in_sandbox),
            updated_at = NOW()
        WHERE id = $1 AND user_id = $2 AND is_system = false
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        "#,
        id,
        user_id,
        request.name,
        request.display_name,
        request.description,
        request.enabled,
        request.command,
        args,
        env_plain,
        env_enc,
        &env_secret_keys,
        request.url,
        hdr_plain,
        hdr_enc,
        &hdr_secret_keys,
        request.timeout_seconds.map(|t| t as i32),
        request.supports_sampling,
        request.usage_mode.as_ref().map(|m| m.to_string()),
        request.max_concurrent_sessions,
        request.run_in_sandbox,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e
            && db_err.is_unique_violation() {
                return AppError::conflict("Server name");
            }
        if let sqlx::Error::RowNotFound = e {
            return AppError::not_found("Server");
        }
        AppError::from(e)
    })?;

    let raw = McpServerColumnsRaw {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: row.transport_type,
        command: row.command,
        args: row.args,
        environment_variables: row.environment_variables,
        environment_variables_encrypted: row.environment_variables_encrypted,
        environment_variables_secret_keys: row.environment_variables_secret_keys,
        url: row.url,
        headers: row.headers,
        headers_encrypted: row.headers_encrypted,
        headers_secret_keys: row.headers_secret_keys,
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: row.usage_mode,
        max_concurrent_sessions: row.max_concurrent_sessions,
        run_in_sandbox: row.run_in_sandbox,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };
    assemble_mcp_server(pool, raw).await
}

/// Delete user MCP server
pub async fn delete_user_mcp_server(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query!(
        "DELETE FROM mcp_servers WHERE id = $1 AND user_id = $2 AND is_system = false",
        id,
        user_id
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Server"));
    }

    Ok(())
}

// =====================================================
// System Server Operations (Admin)
// =====================================================

/// Create a new system MCP server
pub async fn create_system_mcp_server(
    pool: &PgPool,
    request: CreateMcpServerRequest,
) -> Result<McpServer, AppError> {
    // Validate transport-specific fields
    validate_transport_config(&request.transport_type, &request)?;

    let args = serde_json::to_value(request.args.clone().unwrap_or_default())
        .map_err(|e| AppError::internal_error(format!("Failed to serialize args: {}", e)))?;

    let env_entries: Vec<EnvVarEntry> = request
        .environment_variables_entries
        .clone()
        .unwrap_or_default();
    let header_entries: Vec<HeaderEntry> = request.headers_entries.clone().unwrap_or_default();
    validate_header_entries(&header_entries)?;
    let (env_plain, env_enc, env_secret_keys) =
        split_entries_for_storage(pool, env_entries.iter().map(|e| EntryRef::from(e)).collect(), None).await?;
    let (hdr_plain, hdr_enc, hdr_secret_keys) =
        split_entries_for_storage(pool, header_entries.iter().map(|e| EntryRef::from(e)).collect(), None).await?;

    let supports_sampling = request.supports_sampling.unwrap_or(false);
    let usage_mode = request.usage_mode.clone().unwrap_or(UsageMode::Auto);

    let row = sqlx::query!(
        r#"
        INSERT INTO mcp_servers (
            name, display_name, description,
            transport_type, command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds, enabled, is_system,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, true,
                $16, $17, $18, $19)
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        "#,
        request.name,
        request.display_name,
        request.description,
        request.transport_type.to_string(),
        request.command,
        args,
        env_plain,
        env_enc,
        &env_secret_keys,
        request.url,
        hdr_plain,
        hdr_enc,
        &hdr_secret_keys,
        request.timeout_seconds.unwrap_or(30) as i32,
        request.enabled.unwrap_or(false),
        supports_sampling,
        usage_mode.to_string(),
        request.max_concurrent_sessions,
        request.run_in_sandbox.unwrap_or(false),
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e
            && db_err.is_unique_violation() {
                return AppError::conflict("Server name");
            }
        AppError::from(e)
    })?;

    let raw = McpServerColumnsRaw {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: row.transport_type,
        command: row.command,
        args: row.args,
        environment_variables: row.environment_variables,
        environment_variables_encrypted: row.environment_variables_encrypted,
        environment_variables_secret_keys: row.environment_variables_secret_keys,
        url: row.url,
        headers: row.headers,
        headers_encrypted: row.headers_encrypted,
        headers_secret_keys: row.headers_secret_keys,
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: row.usage_mode,
        max_concurrent_sessions: row.max_concurrent_sessions,
        run_in_sandbox: row.run_in_sandbox,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };
    assemble_mcp_server(pool, raw).await
}

// ─── OAuth client_credentials config (Phase 4) ───────────────────────────────

fn oauth_row_to_model(
    server_id: Uuid,
    client_id: String,
    client_secret: String,
    scopes: Option<String>,
    resource: Option<String>,
    created_at: time::OffsetDateTime,
    updated_at: time::OffsetDateTime,
) -> McpServerOAuthConfig {
    McpServerOAuthConfig {
        server_id,
        client_id,
        client_secret,
        scopes,
        resource,
        created_at: DateTime::from_timestamp(created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(updated_at.unix_timestamp(), 0).unwrap(),
    }
}

pub async fn get_mcp_server_oauth_config(
    pool: &PgPool,
    server_id: Uuid,
) -> Result<Option<McpServerOAuthConfig>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT server_id, client_id, client_secret, scopes, resource, created_at, updated_at
        FROM mcp_server_oauth_configs
        WHERE server_id = $1
        "#,
        server_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        oauth_row_to_model(
            r.server_id, r.client_id, r.client_secret, r.scopes, r.resource,
            r.created_at, r.updated_at,
        )
    }))
}

pub async fn set_mcp_server_oauth_config(
    pool: &PgPool,
    server_id: Uuid,
    request: SetMcpServerOAuthConfigRequest,
) -> Result<McpServerOAuthConfig, AppError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO mcp_server_oauth_configs
            (server_id, client_id, client_secret, scopes, resource, updated_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (server_id) DO UPDATE SET
            client_id = EXCLUDED.client_id,
            client_secret = EXCLUDED.client_secret,
            scopes = EXCLUDED.scopes,
            resource = EXCLUDED.resource,
            updated_at = NOW()
        RETURNING server_id, client_id, client_secret, scopes, resource, created_at, updated_at
        "#,
        server_id,
        request.client_id,
        request.client_secret,
        request.scopes,
        request.resource,
    )
    .fetch_one(pool)
    .await?;

    Ok(oauth_row_to_model(
        row.server_id, row.client_id, row.client_secret, row.scopes, row.resource,
        row.created_at, row.updated_at,
    ))
}

pub async fn delete_mcp_server_oauth_config(
    pool: &PgPool,
    server_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        "DELETE FROM mcp_server_oauth_configs WHERE server_id = $1",
        server_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_any_mcp_server(pool: &PgPool, id: Uuid) -> Result<Option<McpServer>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        FROM mcp_servers
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let raw = McpServerColumnsRaw {
                id: r.id,
                user_id: r.user_id,
                name: r.name,
                display_name: r.display_name,
                description: r.description,
                enabled: r.enabled,
                is_system: r.is_system,
                is_built_in: r.is_built_in,
                transport_type: r.transport_type,
                command: r.command,
                args: r.args,
                environment_variables: r.environment_variables,
                environment_variables_encrypted: r.environment_variables_encrypted,
                environment_variables_secret_keys: r.environment_variables_secret_keys,
                url: r.url,
                headers: r.headers,
                headers_encrypted: r.headers_encrypted,
                headers_secret_keys: r.headers_secret_keys,
                timeout_seconds: r.timeout_seconds,
                supports_sampling: r.supports_sampling,
                usage_mode: r.usage_mode,
                max_concurrent_sessions: r.max_concurrent_sessions,
                run_in_sandbox: r.run_in_sandbox,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            Ok(Some(assemble_mcp_server(pool, raw).await?))
        }
        None => Ok(None),
    }
}

pub async fn get_system_mcp_server(pool: &PgPool, id: Uuid) -> Result<Option<McpServer>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        FROM mcp_servers
        WHERE id = $1 AND is_system = true
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let raw = McpServerColumnsRaw {
                id: r.id,
                user_id: r.user_id,
                name: r.name,
                display_name: r.display_name,
                description: r.description,
                enabled: r.enabled,
                is_system: r.is_system,
                is_built_in: r.is_built_in,
                transport_type: r.transport_type,
                command: r.command,
                args: r.args,
                environment_variables: r.environment_variables,
                environment_variables_encrypted: r.environment_variables_encrypted,
                environment_variables_secret_keys: r.environment_variables_secret_keys,
                url: r.url,
                headers: r.headers,
                headers_encrypted: r.headers_encrypted,
                headers_secret_keys: r.headers_secret_keys,
                timeout_seconds: r.timeout_seconds,
                supports_sampling: r.supports_sampling,
                usage_mode: r.usage_mode,
                max_concurrent_sessions: r.max_concurrent_sessions,
                run_in_sandbox: r.run_in_sandbox,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            Ok(Some(assemble_mcp_server(pool, raw).await?))
        }
        None => Ok(None),
    }
}

/// List all system MCP servers with pagination
pub async fn list_system_mcp_servers(
    pool: &PgPool,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    enabled: Option<bool>,
) -> Result<(Vec<McpServer>, i64), AppError> {
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        FROM mcp_servers
        WHERE is_system = true
          AND ($3::text IS NULL
               OR name ILIKE '%' || $3 || '%'
               OR display_name ILIKE '%' || $3 || '%'
               OR description ILIKE '%' || $3 || '%')
          AND ($4::boolean IS NULL OR enabled = $4)
        ORDER BY display_name ASC
        LIMIT $1 OFFSET $2
        "#,
        per_page,
        offset,
        search,
        enabled,
    )
    .fetch_all(pool)
    .await?;

    let mut servers: Vec<McpServer> = Vec::with_capacity(rows.len());
    for r in rows {
        let raw = McpServerColumnsRaw {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: r.transport_type,
            command: r.command,
            args: r.args,
            environment_variables: r.environment_variables,
            environment_variables_encrypted: r.environment_variables_encrypted,
            environment_variables_secret_keys: r.environment_variables_secret_keys,
            url: r.url,
            headers: r.headers,
            headers_encrypted: r.headers_encrypted,
            headers_secret_keys: r.headers_secret_keys,
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: r.usage_mode,
            max_concurrent_sessions: r.max_concurrent_sessions,
            run_in_sandbox: r.run_in_sandbox,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        servers.push(assemble_mcp_server(pool, raw).await?);
    }

    let total = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM mcp_servers
        WHERE is_system = true
          AND ($1::text IS NULL
               OR name ILIKE '%' || $1 || '%'
               OR display_name ILIKE '%' || $1 || '%'
               OR description ILIKE '%' || $1 || '%')
          AND ($2::boolean IS NULL OR enabled = $2)
        "#,
        search,
        enabled,
    )
    .fetch_one(pool)
    .await?
    .count
    .unwrap_or(0);

    Ok((servers, total))
}

/// Update system MCP server
pub async fn update_system_mcp_server(
    pool: &PgPool,
    id: Uuid,
    request: UpdateMcpServerRequest,
) -> Result<McpServer, AppError> {
    // Get the existing server to validate transport type
    let existing = get_system_mcp_server(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    // Validate transport-specific updates
    validate_transport_update(&existing.transport_type, &request)?;

    // Fetch the raw prior columns so we can pass `*_encrypted` JSON
    // into `split_entries_for_storage` (preserves untouched secrets)
    // and reuse the prior columns directly when the request omits the
    // entries field.
    let prior = sqlx::query!(
        r#"
        SELECT
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            headers, headers_encrypted, headers_secret_keys
        FROM mcp_servers
        WHERE id = $1 AND is_system = true
        "#,
        id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::RowNotFound = e {
            return AppError::not_found("Server");
        }
        AppError::from(e)
    })?;

    let args = request.args.and_then(|a| serde_json::to_value(a).ok());

    let (env_plain, env_enc, env_secret_keys) = match &request.environment_variables_entries {
        Some(entries) => {
            split_entries_for_storage(
                pool,
                entries.iter().map(|e| EntryRef::from(e)).collect(),
                Some(&prior.environment_variables_encrypted),
            )
            .await?
        }
        None => (
            prior
                .environment_variables
                .clone()
                .unwrap_or_else(|| serde_json::json!({})),
            prior.environment_variables_encrypted.clone(),
            prior.environment_variables_secret_keys.clone(),
        ),
    };

    let (hdr_plain, hdr_enc, hdr_secret_keys) = match &request.headers_entries {
        Some(entries) => {
            validate_header_entries(entries)?;
            split_entries_for_storage(
                pool,
                entries.iter().map(|e| EntryRef::from(e)).collect(),
                Some(&prior.headers_encrypted),
            )
            .await?
        }
        None => (
            prior.headers.clone().unwrap_or_else(|| serde_json::json!({})),
            prior.headers_encrypted.clone(),
            prior.headers_secret_keys.clone(),
        ),
    };

    let row = sqlx::query!(
        r#"
        UPDATE mcp_servers SET
            name = COALESCE($2, name),
            display_name = COALESCE($3, display_name),
            description = COALESCE($4, description),
            enabled = COALESCE($5, enabled),
            command = COALESCE($6, command),
            args = COALESCE($7, args),
            environment_variables = $8,
            environment_variables_encrypted = $9,
            environment_variables_secret_keys = $10,
            url = COALESCE($11, url),
            headers = $12,
            headers_encrypted = $13,
            headers_secret_keys = $14,
            timeout_seconds = COALESCE($15, timeout_seconds),
            supports_sampling = COALESCE($16, supports_sampling),
            usage_mode = COALESCE($17, usage_mode),
            max_concurrent_sessions = COALESCE($18, max_concurrent_sessions),
            run_in_sandbox = COALESCE($19, run_in_sandbox),
            updated_at = NOW()
        WHERE id = $1 AND is_system = true
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args,
            environment_variables, environment_variables_encrypted, environment_variables_secret_keys,
            url,
            headers, headers_encrypted, headers_secret_keys,
            timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            run_in_sandbox,
            created_at, updated_at
        "#,
        id,
        request.name,
        request.display_name,
        request.description,
        request.enabled,
        request.command,
        args,
        env_plain,
        env_enc,
        &env_secret_keys,
        request.url,
        hdr_plain,
        hdr_enc,
        &hdr_secret_keys,
        request.timeout_seconds.map(|t| t as i32),
        request.supports_sampling,
        request.usage_mode.as_ref().map(|m| m.to_string()),
        request.max_concurrent_sessions,
        request.run_in_sandbox,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e
            && db_err.is_unique_violation() {
                return AppError::conflict("Server name");
            }
        if let sqlx::Error::RowNotFound = e {
            return AppError::not_found("Server");
        }
        AppError::from(e)
    })?;

    let raw = McpServerColumnsRaw {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: row.transport_type,
        command: row.command,
        args: row.args,
        environment_variables: row.environment_variables,
        environment_variables_encrypted: row.environment_variables_encrypted,
        environment_variables_secret_keys: row.environment_variables_secret_keys,
        url: row.url,
        headers: row.headers,
        headers_encrypted: row.headers_encrypted,
        headers_secret_keys: row.headers_secret_keys,
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: row.usage_mode,
        max_concurrent_sessions: row.max_concurrent_sessions,
        run_in_sandbox: row.run_in_sandbox,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };
    assemble_mcp_server(pool, raw).await
}

/// Delete system MCP server
pub async fn delete_system_mcp_server(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let server = sqlx::query!(
        "SELECT is_built_in FROM mcp_servers WHERE id = $1 AND is_system = true",
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("Server"))?;

    if server.is_built_in {
        return Err(AppError::bad_request("BUILT_IN_SERVER", "Cannot delete a built-in system server"));
    }

    sqlx::query!(
        "DELETE FROM mcp_servers WHERE id = $1 AND is_system = true",
        id
    )
    .execute(pool)
    .await?;

    Ok(())
}

// =====================================================
// Group Assignment Operations
// =====================================================

/// Get server IDs assigned to a group
pub async fn get_group_mcp_servers(pool: &PgPool, group_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let server_ids = sqlx::query!(
        "SELECT mcp_server_id FROM user_group_mcp_servers WHERE group_id = $1",
        group_id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.mcp_server_id)
    .collect();

    Ok(server_ids)
}

/// Get full system MCP server details for a group (for UI widgets)
pub async fn get_system_servers_for_group(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<McpServer>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id, s.user_id, s.name, s.display_name, s.description,
               s.enabled, s.is_system, s.is_built_in, s.transport_type,
               s.command, s.args,
               s.environment_variables, s.environment_variables_encrypted, s.environment_variables_secret_keys,
               s.url,
               s.headers, s.headers_encrypted, s.headers_secret_keys,
               s.timeout_seconds,
               s.supports_sampling, s.usage_mode, s.max_concurrent_sessions,
               s.run_in_sandbox,
               s.created_at, s.updated_at
        FROM mcp_servers s
        INNER JOIN user_group_mcp_servers ugms ON s.id = ugms.mcp_server_id
        WHERE ugms.group_id = $1 AND s.is_system = true
        ORDER BY s.display_name ASC
        "#,
        group_id
    )
    .fetch_all(pool)
    .await?;

    let mut servers: Vec<McpServer> = Vec::with_capacity(rows.len());
    for r in rows {
        let raw = McpServerColumnsRaw {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: r.transport_type,
            command: r.command,
            args: r.args,
            environment_variables: r.environment_variables,
            environment_variables_encrypted: r.environment_variables_encrypted,
            environment_variables_secret_keys: r.environment_variables_secret_keys,
            url: r.url,
            headers: r.headers,
            headers_encrypted: r.headers_encrypted,
            headers_secret_keys: r.headers_secret_keys,
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: r.usage_mode,
            max_concurrent_sessions: r.max_concurrent_sessions,
            run_in_sandbox: r.run_in_sandbox,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        servers.push(assemble_mcp_server(pool, raw).await?);
    }

    Ok(servers)
}

/// Assign MCP server to group
pub async fn assign_mcp_server_to_group(
    pool: &PgPool,
    group_id: Uuid,
    server_id: Uuid,
) -> Result<(), AppError> {
    // Verify server is a system server
    let server = sqlx::query!("SELECT is_system FROM mcp_servers WHERE id = $1", server_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    if !server.is_system {
        return Err(AppError::bad_request(
            "INVALID_SERVER",
            "Only system servers can be assigned to groups",
        ));
    }

    sqlx::query!(
        r#"
        INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
        VALUES ($1, $2)
        ON CONFLICT (group_id, mcp_server_id) DO NOTHING
        "#,
        group_id,
        server_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Remove MCP server from group
pub async fn remove_mcp_server_from_group(
    pool: &PgPool,
    group_id: Uuid,
    server_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query!(
        "DELETE FROM user_group_mcp_servers WHERE group_id = $1 AND mcp_server_id = $2",
        group_id,
        server_id
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Server assignment"));
    }

    Ok(())
}

/// Set group's MCP servers (replaces all assignments)
pub async fn set_group_mcp_servers(
    pool: &PgPool,
    group_id: Uuid,
    server_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    // Start transaction
    let mut tx = pool.begin().await?;

    // Verify all servers are system servers
    for server_id in &server_ids {
        let server = sqlx::query!("SELECT is_system FROM mcp_servers WHERE id = $1", server_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::not_found("Server"))?;

        if !server.is_system {
            return Err(AppError::bad_request(
                "INVALID_SERVER",
                "Only system servers can be assigned to groups",
            ));
        }
    }

    // Delete all existing assignments
    sqlx::query!(
        "DELETE FROM user_group_mcp_servers WHERE group_id = $1",
        group_id
    )
    .execute(&mut *tx)
    .await?;

    // Insert new assignments
    for server_id in server_ids {
        sqlx::query!(
            "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id) VALUES ($1, $2)",
            group_id,
            server_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

/// Get groups assigned to an MCP server (server-centric)
pub async fn get_server_groups(pool: &PgPool, server_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let group_ids = sqlx::query!(
        "SELECT group_id FROM user_group_mcp_servers WHERE mcp_server_id = $1",
        server_id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.group_id)
    .collect();

    Ok(group_ids)
}

/// Set groups for an MCP server (server-centric, replaces all assignments)
pub async fn set_server_groups(
    pool: &PgPool,
    server_id: Uuid,
    group_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    // Verify server is a system server
    let server = sqlx::query!("SELECT is_system FROM mcp_servers WHERE id = $1", server_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("Server"))?;

    if !server.is_system {
        return Err(AppError::bad_request(
            "INVALID_SERVER",
            "Only system servers can be assigned to groups",
        ));
    }

    // Start transaction
    let mut tx = pool.begin().await?;

    // Delete all existing assignments for this server
    sqlx::query!(
        "DELETE FROM user_group_mcp_servers WHERE mcp_server_id = $1",
        server_id
    )
    .execute(&mut *tx)
    .await?;

    // Insert new assignments
    for group_id in group_ids {
        sqlx::query!(
            "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id) VALUES ($1, $2)",
            group_id,
            server_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

// =====================================================
// Combined View (Accessible Servers)
// =====================================================

/// List user's accessible MCP servers (own servers + group-assigned system servers)
pub async fn list_accessible_mcp_servers(
    pool: &PgPool,
    user_id: Uuid,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    enabled: Option<bool>,
    is_system: Option<bool>,
) -> Result<(Vec<McpServer>, i64), AppError> {
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"
        SELECT DISTINCT
            s.id, s.user_id, s.name, s.display_name, s.description,
            s.enabled, s.is_system, s.is_built_in, s.transport_type,
            s.command, s.args,
            s.environment_variables, s.environment_variables_encrypted, s.environment_variables_secret_keys,
            s.url,
            s.headers, s.headers_encrypted, s.headers_secret_keys,
            s.timeout_seconds,
            s.supports_sampling, s.usage_mode, s.max_concurrent_sessions,
            s.run_in_sandbox,
            s.created_at, s.updated_at
        FROM mcp_servers s
        LEFT JOIN user_group_mcp_servers ugms ON s.id = ugms.mcp_server_id
        LEFT JOIN user_groups ug ON ugms.group_id = ug.group_id
        WHERE
            (s.user_id = $1 OR (s.is_system = true AND ug.user_id = $1))
            AND ($4::text IS NULL
                 OR s.name ILIKE '%' || $4 || '%'
                 OR s.display_name ILIKE '%' || $4 || '%'
                 OR s.description ILIKE '%' || $4 || '%')
            AND ($5::boolean IS NULL OR s.enabled = $5)
            AND ($6::boolean IS NULL OR s.is_system = $6)
        ORDER BY s.is_system ASC, s.display_name ASC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        per_page,
        offset,
        search,
        enabled,
        is_system,
    )
    .fetch_all(pool)
    .await?;

    let mut servers: Vec<McpServer> = Vec::with_capacity(rows.len());
    for r in rows {
        let raw = McpServerColumnsRaw {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: r.transport_type,
            command: r.command,
            args: r.args,
            environment_variables: r.environment_variables,
            environment_variables_encrypted: r.environment_variables_encrypted,
            environment_variables_secret_keys: r.environment_variables_secret_keys,
            url: r.url,
            headers: r.headers,
            headers_encrypted: r.headers_encrypted,
            headers_secret_keys: r.headers_secret_keys,
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: r.usage_mode,
            max_concurrent_sessions: r.max_concurrent_sessions,
            run_in_sandbox: r.run_in_sandbox,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        servers.push(assemble_mcp_server(pool, raw).await?);
    }

    // Count total accessible servers — predicates MUST match the
    // list query above so the UI's <Pagination total> is accurate.
    let total = sqlx::query!(
        r#"
        SELECT COUNT(DISTINCT s.id) as count
        FROM mcp_servers s
        LEFT JOIN user_group_mcp_servers ugms ON s.id = ugms.mcp_server_id
        LEFT JOIN user_groups ug ON ugms.group_id = ug.group_id
        WHERE
            (s.user_id = $1 OR (s.is_system = true AND ug.user_id = $1))
            AND ($2::text IS NULL
                 OR s.name ILIKE '%' || $2 || '%'
                 OR s.display_name ILIKE '%' || $2 || '%'
                 OR s.description ILIKE '%' || $2 || '%')
            AND ($3::boolean IS NULL OR s.enabled = $3)
            AND ($4::boolean IS NULL OR s.is_system = $4)
        "#,
        user_id,
        search,
        enabled,
        is_system,
    )
    .fetch_one(pool)
    .await?
    .count
    .unwrap_or(0);

    Ok((servers, total))
}

// =====================================================
// Validation Helpers
// =====================================================

/// Validates the transport-specific required fields on a
/// `CreateMcpServerRequest` (stdio→command; http/sse→url+url-format).
///
/// Exposed `pub(crate)` so the hub-install path can call this BEFORE
/// touching the database — without it, the `replace_existing`
/// re-install path would delete the prior system MCP server, then
/// fail on `create_system_server`'s own validation, leaving the
/// admin with NO system server for that hub_id. The native create
/// flow runs this internally so callers there don't need to.
pub(crate) fn validate_transport_config(
    transport_type: &TransportType,
    request: &CreateMcpServerRequest,
) -> Result<(), AppError> {
    match transport_type {
        TransportType::Stdio => {
            if request.command.is_none()
                || request
                    .command
                    .as_ref()
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            {
                return Err(AppError::bad_request(
                    "INVALID_TRANSPORT",
                    "command is required for stdio transport",
                ));
            }
        }
        TransportType::Http | TransportType::Sse => {
            if request.url.is_none() || request.url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                return Err(AppError::bad_request(
                    "INVALID_TRANSPORT",
                    "url is required for http/sse transport",
                ));
            }
            // Validate URL format
            if let Some(url) = &request.url {
                validate_url(url)?;
            }
        }
    }
    Ok(())
}

fn validate_transport_update(
    transport_type: &TransportType,
    request: &UpdateMcpServerRequest,
) -> Result<(), AppError> {
    match transport_type {
        TransportType::Stdio => {
            if let Some(command) = &request.command
                && command.is_empty() {
                    return Err(AppError::bad_request(
                        "INVALID_TRANSPORT",
                        "command cannot be empty for stdio transport",
                    ));
                }
        }
        TransportType::Http | TransportType::Sse => {
            if let Some(url) = &request.url {
                if url.is_empty() {
                    return Err(AppError::bad_request(
                        "INVALID_TRANSPORT",
                        "url cannot be empty for http/sse transport",
                    ));
                }
                validate_url(url)?;
            }
        }
    }
    Ok(())
}

fn validate_url(url: &str) -> Result<(), AppError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::bad_request(
            "INVALID_URL",
            "url must start with http:// or https://",
        ));
    }
    Ok(())
}

/// Validate that each header entry's value (if supplied) parses to a
/// valid HTTP header — RFC 7230 §3.2.4 syntax, no interior control
/// chars. Mirrors the prior `normalize_and_validate_headers` save-
/// time validation but operates on the new structured entry shape.
///
/// Entries with `value: None` (saved-secret untouched) are skipped —
/// the prior encrypted value's shape was validated when it was
/// originally set, and we don't decrypt at save time. Untrimmed
/// values are accepted; `split_entries_for_storage` trims later.
pub(crate) fn validate_header_entries(entries: &[HeaderEntry]) -> Result<(), AppError> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut probe = serde_json::Map::new();
    for entry in entries {
        if let Some(value) = entry.value.as_deref() {
            probe.insert(entry.key.clone(), serde_json::Value::String(value.to_string()));
        }
    }
    if probe.is_empty() {
        return Ok(());
    }
    let as_value = serde_json::Value::Object(probe);
    // Save-time validation runs against LITERAL header values (no
    // `${VAR}` expansion) — the runtime call path re-runs
    // parse_header_map against the resolved env later.
    let (_parsed, errors) = super::client::http::parse_header_map(
        &as_value,
        &serde_json::Value::Object(Default::default()),
    );
    if let Some(first) = errors.first() {
        return Err(AppError::bad_request(
            "INVALID_HEADER",
            format!(
                "Invalid value for header {:?}: {}",
                first.name, first.reason
            ),
        ));
    }
    Ok(())
}
