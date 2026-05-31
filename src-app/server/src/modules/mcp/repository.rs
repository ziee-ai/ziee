// MCP repository
#![allow(dead_code)]

use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{
    McpServer, McpServerOAuthConfig, SetMcpServerOAuthConfigRequest, TransportType, UsageMode,
};
use super::types::{CreateMcpServerRequest, McpServerListResponse, UpdateMcpServerRequest};

/// MCP Repository
pub struct McpRepository {
    pool: PgPool,
}

impl McpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
    ) -> Result<McpServerListResponse, AppError> {
        let (servers, total) = list_system_mcp_servers(&self.pool, page, per_page).await?;
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
    ) -> Result<McpServerListResponse, AppError> {
        let (servers, total) =
            list_accessible_mcp_servers(&self.pool, user_id, page, per_page).await?;
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

    let env_vars = serde_json::to_value(request.environment_variables.clone().unwrap_or_default())
        .map_err(|e| {
            AppError::internal_error(format!("Failed to serialize environment_variables: {}", e))
        })?;

    let headers = serde_json::to_value(normalize_and_validate_headers(&request.headers)?)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize headers: {}", e)))?;

    let supports_sampling = request.supports_sampling.unwrap_or(false);
    let usage_mode = request.usage_mode.clone().unwrap_or(UsageMode::Auto);

    let row = sqlx::query!(
        r#"
        INSERT INTO mcp_servers (
            user_id, name, display_name, description,
            transport_type, command, args, environment_variables,
            url, headers, timeout_seconds, enabled, is_system,
            supports_sampling, usage_mode, max_concurrent_sessions
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, false,
                $13, $14, $15)
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        "#,
        user_id,
        request.name,
        request.display_name,
        request.description,
        request.transport_type.to_string(),
        request.command,
        args,
        env_vars,
        request.url,
        headers,
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

    let server = McpServer {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: TransportType::from_str(&row.transport_type)?,
        command: row.command,
        args: row.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: row
            .environment_variables
            .unwrap_or_else(|| serde_json::json!({})),
        url: row.url,
        headers: row.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: UsageMode::from_str(&row.usage_mode)?,
        max_concurrent_sessions: row.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid created_at timestamp"))?,
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid updated_at timestamp"))?,
    };

    Ok(server)
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
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        FROM mcp_servers
        WHERE id = $1 AND user_id = $2 AND is_system = false
        "#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| McpServer {
        id: r.id,
        user_id: r.user_id,
        name: r.name,
        display_name: r.display_name,
        description: r.description,
        enabled: r.enabled,
        is_system: r.is_system,
        is_built_in: r.is_built_in,
        transport_type: TransportType::from_str(&r.transport_type).unwrap(),
        command: r.command,
        args: r.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: r
            .environment_variables
            .unwrap_or_else(|| serde_json::json!({})),
        url: r.url,
        headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: r.timeout_seconds,
        supports_sampling: r.supports_sampling,
        usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
        max_concurrent_sessions: r.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
    }))
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
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
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

    let servers: Vec<McpServer> = rows
        .into_iter()
        .map(|r| McpServer {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: TransportType::from_str(&r.transport_type).unwrap(),
            command: r.command,
            args: r.args.unwrap_or_else(|| serde_json::json!([])),
            environment_variables: r
                .environment_variables
                .unwrap_or_else(|| serde_json::json!({})),
            url: r.url,
            headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
            max_concurrent_sessions: r.max_concurrent_sessions,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect();

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

    let args = request.args.and_then(|a| serde_json::to_value(a).ok());
    let env_vars = request
        .environment_variables
        .and_then(|e| serde_json::to_value(e).ok());
    let headers = match &request.headers {
        Some(_) => Some(
            serde_json::to_value(normalize_and_validate_headers(&request.headers)?).map_err(
                |e| AppError::internal_error(format!("Failed to serialize headers: {}", e)),
            )?,
        ),
        None => None,
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
            environment_variables = COALESCE($9, environment_variables),
            url = COALESCE($10, url),
            headers = COALESCE($11, headers),
            timeout_seconds = COALESCE($12, timeout_seconds),
            supports_sampling = COALESCE($13, supports_sampling),
            usage_mode = COALESCE($14, usage_mode),
            max_concurrent_sessions = COALESCE($15, max_concurrent_sessions),
            updated_at = NOW()
        WHERE id = $1 AND user_id = $2 AND is_system = false
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
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
        env_vars,
        request.url,
        headers,
        request.timeout_seconds.map(|t| t as i32),
        request.supports_sampling,
        request.usage_mode.as_ref().map(|m| m.to_string()),
        request.max_concurrent_sessions,
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

    let server = McpServer {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: TransportType::from_str(&row.transport_type)?,
        command: row.command,
        args: row.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: row
            .environment_variables
            .unwrap_or_else(|| serde_json::json!({})),
        url: row.url,
        headers: row.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: UsageMode::from_str(&row.usage_mode)?,
        max_concurrent_sessions: row.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid created_at timestamp"))?,
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid updated_at timestamp"))?,
    };

    Ok(server)
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

    let env_vars = serde_json::to_value(request.environment_variables.clone().unwrap_or_default())
        .map_err(|e| {
            AppError::internal_error(format!("Failed to serialize environment_variables: {}", e))
        })?;

    let headers = serde_json::to_value(normalize_and_validate_headers(&request.headers)?)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize headers: {}", e)))?;

    let supports_sampling = request.supports_sampling.unwrap_or(false);
    let usage_mode = request.usage_mode.clone().unwrap_or(UsageMode::Auto);

    let row = sqlx::query!(
        r#"
        INSERT INTO mcp_servers (
            name, display_name, description,
            transport_type, command, args, environment_variables,
            url, headers, timeout_seconds, enabled, is_system,
            supports_sampling, usage_mode, max_concurrent_sessions
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, true,
                $12, $13, $14)
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        "#,
        request.name,
        request.display_name,
        request.description,
        request.transport_type.to_string(),
        request.command,
        args,
        env_vars,
        request.url,
        headers,
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

    let server = McpServer {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: TransportType::from_str(&row.transport_type)?,
        command: row.command,
        args: row.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: row
            .environment_variables
            .unwrap_or_else(|| serde_json::json!({})),
        url: row.url,
        headers: row.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: UsageMode::from_str(&row.usage_mode)?,
        max_concurrent_sessions: row.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid created_at timestamp"))?,
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid updated_at timestamp"))?,
    };

    Ok(server)
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
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        FROM mcp_servers
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| McpServer {
        id: r.id,
        user_id: r.user_id,
        name: r.name,
        display_name: r.display_name,
        description: r.description,
        enabled: r.enabled,
        is_system: r.is_system,
        is_built_in: r.is_built_in,
        transport_type: TransportType::from_str(&r.transport_type).unwrap(),
        command: r.command,
        args: r.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: r.environment_variables.unwrap_or_else(|| serde_json::json!({})),
        url: r.url,
        headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: r.timeout_seconds,
        supports_sampling: r.supports_sampling,
        usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
        max_concurrent_sessions: r.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
    }))
}

pub async fn get_system_mcp_server(pool: &PgPool, id: Uuid) -> Result<Option<McpServer>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        FROM mcp_servers
        WHERE id = $1 AND is_system = true
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| McpServer {
        id: r.id,
        user_id: r.user_id,
        name: r.name,
        display_name: r.display_name,
        description: r.description,
        enabled: r.enabled,
        is_system: r.is_system,
        is_built_in: r.is_built_in,
        transport_type: TransportType::from_str(&r.transport_type).unwrap(),
        command: r.command,
        args: r.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: r
            .environment_variables
            .unwrap_or_else(|| serde_json::json!({})),
        url: r.url,
        headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: r.timeout_seconds,
        supports_sampling: r.supports_sampling,
        usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
        max_concurrent_sessions: r.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
    }))
}

/// List all system MCP servers with pagination
pub async fn list_system_mcp_servers(
    pool: &PgPool,
    page: i64,
    per_page: i64,
) -> Result<(Vec<McpServer>, i64), AppError> {
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"
        SELECT
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        FROM mcp_servers
        WHERE is_system = true
        ORDER BY display_name ASC
        LIMIT $1 OFFSET $2
        "#,
        per_page,
        offset
    )
    .fetch_all(pool)
    .await?;

    let servers: Vec<McpServer> = rows
        .into_iter()
        .map(|r| McpServer {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: TransportType::from_str(&r.transport_type).unwrap(),
            command: r.command,
            args: r.args.unwrap_or_else(|| serde_json::json!([])),
            environment_variables: r
                .environment_variables
                .unwrap_or_else(|| serde_json::json!({})),
            url: r.url,
            headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
            max_concurrent_sessions: r.max_concurrent_sessions,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect();

    let total = sqlx::query!("SELECT COUNT(*) as count FROM mcp_servers WHERE is_system = true")
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

    let args = request.args.and_then(|a| serde_json::to_value(a).ok());
    let env_vars = request
        .environment_variables
        .and_then(|e| serde_json::to_value(e).ok());
    let headers = match &request.headers {
        Some(_) => Some(
            serde_json::to_value(normalize_and_validate_headers(&request.headers)?).map_err(
                |e| AppError::internal_error(format!("Failed to serialize headers: {}", e)),
            )?,
        ),
        None => None,
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
            environment_variables = COALESCE($8, environment_variables),
            url = COALESCE($9, url),
            headers = COALESCE($10, headers),
            timeout_seconds = COALESCE($11, timeout_seconds),
            supports_sampling = COALESCE($12, supports_sampling),
            usage_mode = COALESCE($13, usage_mode),
            max_concurrent_sessions = COALESCE($14, max_concurrent_sessions),
            updated_at = NOW()
        WHERE id = $1 AND is_system = true
        RETURNING
            id, user_id, name, display_name, description,
            enabled, is_system, is_built_in, transport_type,
            command, args, environment_variables, url, headers, timeout_seconds,
            supports_sampling, usage_mode, max_concurrent_sessions,
            created_at, updated_at
        "#,
        id,
        request.name,
        request.display_name,
        request.description,
        request.enabled,
        request.command,
        args,
        env_vars,
        request.url,
        headers,
        request.timeout_seconds.map(|t| t as i32),
        request.supports_sampling,
        request.usage_mode.as_ref().map(|m| m.to_string()),
        request.max_concurrent_sessions,
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

    let server = McpServer {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_system: row.is_system,
        is_built_in: row.is_built_in,
        transport_type: TransportType::from_str(&row.transport_type)?,
        command: row.command,
        args: row.args.unwrap_or_else(|| serde_json::json!([])),
        environment_variables: row
            .environment_variables
            .unwrap_or_else(|| serde_json::json!({})),
        url: row.url,
        headers: row.headers.unwrap_or_else(|| serde_json::json!({})),
        timeout_seconds: row.timeout_seconds,
        supports_sampling: row.supports_sampling,
        usage_mode: UsageMode::from_str(&row.usage_mode)?,
        max_concurrent_sessions: row.max_concurrent_sessions,
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid created_at timestamp"))?,
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0)
            .ok_or_else(|| AppError::internal_error("Invalid updated_at timestamp"))?,
    };

    Ok(server)
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
               s.command, s.args, s.environment_variables, s.url, s.headers, s.timeout_seconds,
               s.supports_sampling, s.usage_mode, s.max_concurrent_sessions,
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

    let servers = rows
        .into_iter()
        .map(|r| McpServer {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: TransportType::from_str(&r.transport_type).unwrap(),
            command: r.command,
            args: r.args.unwrap_or_else(|| serde_json::json!([])),
            environment_variables: r
                .environment_variables
                .unwrap_or_else(|| serde_json::json!({})),
            url: r.url,
            headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
            max_concurrent_sessions: r.max_concurrent_sessions,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect();

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
) -> Result<(Vec<McpServer>, i64), AppError> {
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"
        SELECT DISTINCT
            s.id, s.user_id, s.name, s.display_name, s.description,
            s.enabled, s.is_system, s.is_built_in, s.transport_type,
            s.command, s.args, s.environment_variables, s.url, s.headers, s.timeout_seconds,
            s.supports_sampling, s.usage_mode, s.max_concurrent_sessions,
            s.created_at, s.updated_at
        FROM mcp_servers s
        LEFT JOIN user_group_mcp_servers ugms ON s.id = ugms.mcp_server_id
        LEFT JOIN user_groups ug ON ugms.group_id = ug.group_id
        WHERE
            s.user_id = $1
            OR (s.is_system = true AND ug.user_id = $1)
        ORDER BY s.is_system ASC, s.display_name ASC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        per_page,
        offset
    )
    .fetch_all(pool)
    .await?;

    let servers: Vec<McpServer> = rows
        .into_iter()
        .map(|r| McpServer {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_system: r.is_system,
            is_built_in: r.is_built_in,
            transport_type: TransportType::from_str(&r.transport_type).unwrap(),
            command: r.command,
            args: r.args.unwrap_or_else(|| serde_json::json!([])),
            environment_variables: r
                .environment_variables
                .unwrap_or_else(|| serde_json::json!({})),
            url: r.url,
            headers: r.headers.unwrap_or_else(|| serde_json::json!({})),
            timeout_seconds: r.timeout_seconds,
            supports_sampling: r.supports_sampling,
            usage_mode: UsageMode::from_str(&r.usage_mode).unwrap(),
            max_concurrent_sessions: r.max_concurrent_sessions,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect();

    // Count total accessible servers
    let total = sqlx::query!(
        r#"
        SELECT COUNT(DISTINCT s.id) as count
        FROM mcp_servers s
        LEFT JOIN user_group_mcp_servers ugms ON s.id = ugms.mcp_server_id
        LEFT JOIN user_groups ug ON ugms.group_id = ug.group_id
        WHERE
            s.user_id = $1
            OR (s.is_system = true AND ug.user_id = $1)
        "#,
        user_id
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

fn validate_transport_config(
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

/// Trim + validate a request's HTTP headers before they're persisted.
///
/// Returns the cleaned map (original key casing preserved, each value trimmed of
/// surrounding whitespace) ready to store, or a 400 `INVALID_HEADER` naming the
/// first header whose value still can't form a valid HTTP header (an interior
/// newline, control char, etc.). `None` → empty map. The actual char-validation
/// is delegated to the client's `parse_header_map` so the save boundary and the
/// runtime connect path agree on exactly what counts as valid. We deliberately
/// trim rather than reject trailing whitespace: a token pasted with a trailing
/// newline is an extremely common artifact, and RFC 7230 §3.2.4 says surrounding
/// whitespace isn't part of the field value anyway.
pub(crate) fn normalize_and_validate_headers(
    headers: &Option<std::collections::HashMap<String, String>>,
) -> Result<std::collections::HashMap<String, String>, AppError> {
    let Some(map) = headers else {
        return Ok(std::collections::HashMap::new());
    };

    let as_value = serde_json::to_value(map)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize headers: {}", e)))?;
    let (_parsed, errors) = super::client::http::parse_header_map(&as_value);
    if let Some(first) = errors.first() {
        return Err(AppError::bad_request(
            "INVALID_HEADER",
            format!(
                "Invalid value for header {:?}: {}",
                first.name, first.reason
            ),
        ));
    }

    // All entries valid → persist them trimmed, preserving the user's key casing.
    Ok(map
        .iter()
        .map(|(k, v)| (k.clone(), v.trim().to_string()))
        .collect())
}
