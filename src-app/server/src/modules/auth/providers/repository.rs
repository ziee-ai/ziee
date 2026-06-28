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
               updated_at as "updated_at: _",
               last_test_at as "last_test_at: _",
               last_test_ok,
               last_test_message
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
               updated_at as "updated_at: _",
               last_test_at as "last_test_at: _",
               last_test_ok,
               last_test_message
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
               updated_at as "updated_at: _",
               last_test_at as "last_test_at: _",
               last_test_ok,
               last_test_message
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
               updated_at as "updated_at: _",
               last_test_at as "last_test_at: _",
               last_test_ok,
               last_test_message
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
                  updated_at as "updated_at: _",
                  last_test_at as "last_test_at: _",
                  last_test_ok,
                  last_test_message
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
                  updated_at as "updated_at: _",
                  last_test_at as "last_test_at: _",
                  last_test_ok,
                  last_test_message
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

/// Atomically delete a provider and return its name + the number of
/// `user_auth_links` that were cascade-removed.
///
/// All three steps (existence-lock, link count, delete) run inside ONE
/// transaction with the provider row locked `FOR UPDATE`. This replaces the
/// previous get→count→delete sequence of three separate, non-atomic queries:
///   - Concurrent deletes both passed the standalone existence check, then
///     one DELETE affected 0 rows → a confusing 404 to the loser. Under the
///     row lock the second deleter now blocks until the first commits, then
///     observes the row already gone and gets a clean `Ok(None)` (→ 404,
///     which is correct: the provider genuinely no longer exists).
///   - `affected_user_links` could be stale (links added/removed between the
///     count and the delete). Taken under the lock in the same tx, the count
///     now exactly matches what the FK cascade removes.
///
/// Returns `Ok(None)` when the provider does not exist.
pub async fn delete_provider_with_link_count(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(String, i64)>, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Lock the row for the duration of the tx; serializes concurrent deletes.
    let Some(row) = sqlx::query!(
        r#"SELECT name FROM auth_providers WHERE id = $1 FOR UPDATE"#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    else {
        tx.rollback().await.map_err(AppError::database_error)?;
        return Ok(None);
    };

    let affected = sqlx::query!(
        r#"SELECT COUNT(*) as "count!" FROM user_auth_links WHERE provider_id = $1"#,
        id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .count;

    sqlx::query!(r#"DELETE FROM auth_providers WHERE id = $1"#, id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;
    Ok(Some((row.name, affected)))
}

/// Flip `enabled` to false on the row. Called by the health-enforcement
/// layer when an enable-transition probe fails or a manual Test of an
/// enabled row turns up failing. Touches `updated_at` so frontend
/// caches see the row as fresh.
pub async fn set_enabled_false(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE auth_providers SET enabled = FALSE, updated_at = NOW() WHERE id = $1",
        id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Record the outcome of a test_connection call on the row.
/// Updates last_test_at/ok/message so the admin UI can show the
/// result on the next list refresh.
pub async fn record_test_result(
    pool: &PgPool,
    id: Uuid,
    ok: bool,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE auth_providers
        SET last_test_at = NOW(),
            last_test_ok = $2,
            last_test_message = $3
        WHERE id = $1
        "#,
        id,
        ok,
        message,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
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
