//! bio_mcp upsert — mirrors memory_mcp's `upsert_builtin_server`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct BioMcpRepository {
    pool: PgPool,
}

impl BioMcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in BioMCP server row.
    ///
    /// Unlike the zero-config built-ins (files/memory/elicitation), the bio
    /// row is **admin-configurable**: its id is deliberately NOT in the
    /// `update_system_mcp_server` deny-list, so admins edit its `headers`
    /// (the upstream API keys, e.g. `NCBI_API_KEY`) and `enabled` toggle via
    /// the standard system-server drawer. Therefore the `ON CONFLICT DO
    /// UPDATE` clause only re-asserts IDENTITY columns (`is_system`,
    /// `is_built_in`, `transport_type`, `url`) on each boot — the loopback
    /// `url` carries the live port, which changes between restarts. The
    /// admin-owned columns (`enabled`, `display_name`, `description`,
    /// `headers`, `timeout_seconds`, …) are left untouched on conflict so an
    /// admin's keys + toggle survive restarts.
    pub async fn upsert_builtin_server(
        &self,
        server_id: Uuid,
        loopback_url: &str,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        sqlx::query!(
            r#"
            INSERT INTO mcp_servers (
                id, user_id, name, display_name, description,
                enabled, is_system, is_built_in,
                transport_type, url, headers,
                timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
                created_at, updated_at
            ) VALUES (
                $1, NULL, 'bio', 'BioMCP',
                'Built-in biomedical database connectors (PubMed, ClinicalTrials.gov, variants, trials, drugs, and more) via BioMCP. Connected-only: query terms are sent to public upstream APIs. Configure deployment API keys (NCBI_API_KEY, S2_API_KEY, OPENFDA_API_KEY, NCI_API_KEY, ONCOKB_TOKEN, ALPHAGENOME_API_KEY, DISGENET_API_KEY) as secret entries in this server''s Headers; they are injected into the managed sidecar''s environment, never sent over HTTP.',
                true, true, true,
                'http', $2, '{}'::jsonb,
                120, false, 'auto', 4,
                NOW(), NOW()
            )
            ON CONFLICT (id) DO UPDATE SET
                is_system = EXCLUDED.is_system,
                is_built_in = EXCLUDED.is_built_in,
                transport_type = EXCLUDED.transport_type,
                url = EXCLUDED.url,
                updated_at = NOW()
            "#,
            server_id,
            loopback_url
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        tx.commit().await.map_err(AppError::database_error)?;
        Ok(())
    }
}
