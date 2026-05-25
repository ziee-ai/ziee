// Provider lookup helpers. OAuth session + auth-link mutations live
// on `AuthRepository` (see ../repository.rs) — this file is just
// for lightweight by-name lookups used during the OAuth callback
// and admin CRUD flows.

use sqlx::PgPool;
use uuid::Uuid;

use super::models::AuthProvider;
use crate::common::AppError;

/// Look up an auth provider by its `name` column (the identifier
/// in the URL path).
pub async fn get_provider_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        WHERE name = $1
        "#,
        name
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)
}

/// Look up an auth provider by id. Used by the admin CRUD handlers.
pub async fn get_provider_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)
}

/// List all configured auth providers (admin view — returns ALL
/// rows including disabled ones; secret-masking happens at the
/// response-building layer in handlers.rs).
pub async fn list_providers(pool: &PgPool) -> Result<Vec<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        ORDER BY name ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// List enabled auth providers — the subset surfaced on the
/// `/login` page via `GET /api/auth/providers` (public, no auth).
/// Excludes the built-in `local` provider since the username/password
/// form is rendered statically.
pub async fn list_public_providers(pool: &PgPool) -> Result<Vec<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        WHERE enabled = true AND provider_type <> 'local'
        ORDER BY name ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// Create a new auth_providers row.
pub async fn create_provider(
    pool: &PgPool,
    name: &str,
    provider_type: &str,
    enabled: bool,
    config: &serde_json::Value,
) -> Result<AuthProvider, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        INSERT INTO auth_providers (name, provider_type, enabled, config)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, provider_type, enabled, config,
                  created_at as "created_at: _",
                  updated_at as "updated_at: _"
        "#,
        name,
        provider_type,
        enabled,
        config,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)
}

/// Update an auth_providers row. Pass `None` for fields you want to
/// leave unchanged.
pub async fn update_provider(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    enabled: Option<bool>,
    config: Option<&serde_json::Value>,
) -> Result<AuthProvider, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        UPDATE auth_providers
        SET name = COALESCE($2, name),
            enabled = COALESCE($3, enabled),
            config = COALESCE($4, config)
        WHERE id = $1
        RETURNING id, name, provider_type, enabled, config,
                  created_at as "created_at: _",
                  updated_at as "updated_at: _"
        "#,
        id,
        name,
        enabled,
        config,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)
}

/// Delete an auth_providers row. CASCADE drops any user_auth_links
/// + oauth_sessions referencing it.
pub async fn delete_provider(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"
        DELETE FROM auth_providers WHERE id = $1
        "#,
        id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

/// Count user_auth_links that reference a provider — used by the
/// delete-provider confirm flow to warn the admin how many users
/// will lose this login method.
pub async fn count_links_for_provider(pool: &PgPool, id: Uuid) -> Result<i64, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!" FROM user_auth_links WHERE provider_id = $1
        "#,
        id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.count)
}
