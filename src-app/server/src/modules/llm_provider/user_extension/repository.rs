// Repository for the `user_group_llm_providers` join table.
//
// This bridge subdir is the legitimate home for `user::models::Group`
// imports inside llm_provider — every method here either reads or
// writes the provider↔group join.
//
// Exposed as `Repos.user_group_llm_provider` via `core/repository.rs`.
//
// External callers:
//   - chat/core/handlers/streaming.rs   → user_has_access_to_provider
//   - chat/core/handlers/providers.rs   → get_for_user
// In-bridge callers: handlers.rs (5 HTTP handlers).

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::secret::resolve_optional_secret;
use crate::modules::llm_provider::models::LlmProvider;
use crate::modules::user::models::Group;

/// Convert `time::OffsetDateTime` → `chrono::DateTime<Utc>` with full
/// nanosecond precision. Mirrors the helper in
/// `llm_provider/repositories/admin.rs` (kept duplicated here so the
/// bridge stays self-contained — same one-liner either way).
fn to_chrono(ts: time::OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp_nanos(ts.unix_timestamp_nanos() as i64)
}

#[derive(Clone, Debug)]
pub struct UserGroupLlmProviderRepository {
    pool: PgPool,
}

impl UserGroupLlmProviderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// All groups that have access to a provider.
    pub async fn get_provider_groups(
        &self,
        provider_id: Uuid,
    ) -> Result<Vec<Group>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT g.id, g.name, g.description, g.permissions, g.is_system, g.is_active, g.is_default, g.created_at, g.updated_at
             FROM groups g
             INNER JOIN user_group_llm_providers ugp ON g.id = ugp.group_id
             WHERE ugp.provider_id = $1
             ORDER BY g.name ASC"#,
            provider_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Group {
                id: r.id,
                name: r.name,
                description: r.description,
                permissions: r.permissions,
                is_system: r.is_system,
                is_active: r.is_active,
                is_default: r.is_default,
                created_at: to_chrono(r.created_at),
                updated_at: to_chrono(r.updated_at),
            })
            .collect())
    }

    /// Assign a provider to a user group. Idempotent — re-assigning is a
    /// no-op (returns Ok without an error).
    pub async fn assign_to_group(
        &self,
        provider_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let existing = sqlx::query!(
            "SELECT id FROM user_group_llm_providers WHERE group_id = $1 AND provider_id = $2",
            group_id,
            provider_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            return Ok(());
        }

        let relationship_id = Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO user_group_llm_providers (id, group_id, provider_id) VALUES ($1, $2, $3)",
            relationship_id,
            group_id,
            provider_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Remove a provider from a user group. Returns true iff a row was
    /// actually deleted (false = nothing to remove, 404-able).
    pub async fn remove_from_group(
        &self,
        group_id: Uuid,
        provider_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM user_group_llm_providers WHERE group_id = $1 AND provider_id = $2",
            group_id,
            provider_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// All providers assigned to a user group. Returns full LlmProvider
    /// rows (with api_key decrypted).
    pub async fn get_for_group(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<LlmProvider>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT p.id, p.name, p.provider_type, p.enabled, p.api_key, p.api_key_encrypted, p.base_url, p.built_in, p.proxy_settings, p.created_at, p.updated_at,
                      p.default_runtime_version_id
             FROM llm_providers p
             INNER JOIN user_group_llm_providers ugp ON p.id = ugp.provider_id
             WHERE ugp.group_id = $1
             ORDER BY p.built_in DESC, p.name ASC"#,
            group_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut providers = Vec::with_capacity(rows.len());
        for r in rows {
            let api_key = resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await;
            providers.push(LlmProvider {
                id: r.id,
                name: r.name,
                provider_type: r.provider_type,
                enabled: r.enabled,
                api_key,
                base_url: r.base_url,
                built_in: r.built_in,
                proxy_settings: r
                    .proxy_settings
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default(),
                created_at: to_chrono(r.created_at),
                updated_at: to_chrono(r.updated_at),
                default_runtime_version_id: r.default_runtime_version_id,
            });
        }
        Ok(providers)
    }

    /// All providers available to a user via their group memberships.
    /// Filters out disabled providers + inactive groups at the SQL level.
    pub async fn get_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<LlmProvider>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT DISTINCT p.id, p.name, p.provider_type, p.enabled, p.api_key, p.api_key_encrypted, p.base_url, p.built_in, p.proxy_settings, p.created_at, p.updated_at,
                      p.default_runtime_version_id
             FROM llm_providers p
             INNER JOIN user_group_llm_providers ugp ON p.id = ugp.provider_id
             INNER JOIN user_groups ug ON ugp.group_id = ug.group_id
             INNER JOIN groups g ON ug.group_id = g.id
             WHERE ug.user_id = $1
               AND g.is_active = true
               AND p.enabled = true
             ORDER BY p.built_in DESC, p.name ASC"#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut providers = Vec::with_capacity(rows.len());
        for r in rows {
            let api_key = resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await;
            providers.push(LlmProvider {
                id: r.id,
                name: r.name,
                provider_type: r.provider_type,
                enabled: r.enabled,
                api_key,
                base_url: r.base_url,
                built_in: r.built_in,
                proxy_settings: r
                    .proxy_settings
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default(),
                created_at: to_chrono(r.created_at),
                updated_at: to_chrono(r.updated_at),
                default_runtime_version_id: r.default_runtime_version_id,
            });
        }
        Ok(providers)
    }

    /// True iff `user_id` can use `provider_id` (via group membership +
    /// active group + enabled provider).
    pub async fn user_has_access_to_provider(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT EXISTS(
                 SELECT 1
                 FROM user_group_llm_providers ugp
                 INNER JOIN user_groups ug ON ugp.group_id = ug.group_id
                 INNER JOIN groups g ON ug.group_id = g.id
                 INNER JOIN llm_providers p ON ugp.provider_id = p.id
                 WHERE ug.user_id = $1
                   AND ugp.provider_id = $2
                   AND g.is_active = true
                   AND p.enabled = true
               ) as "has_access!""#,
            user_id,
            provider_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(result.has_access)
    }
}
