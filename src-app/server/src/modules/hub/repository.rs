// Hub repository
#![allow(dead_code)]

use chrono::DateTime;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use super::models::{HubCategory, HubEntity, HubEntityType};
use crate::common::AppError;

/// Hub Repository
pub struct HubRepository {
    pool: PgPool,
}

impl HubRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn track_hub_entity(
        &self,
        entity_type: HubEntityType,
        entity_id: Uuid,
        hub_id: &str,
        hub_category: HubCategory,
        created_by: Option<Uuid>,
        hub_version: Option<&str>,
    ) -> Result<HubEntity, AppError> {
        track_hub_entity(
            &self.pool,
            entity_type,
            entity_id,
            hub_id,
            hub_category,
            created_by,
            hub_version,
        )
        .await
    }

    pub async fn get_created_assistant_ids(
        &self,
        user_id: Uuid,
    ) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
        get_created_assistant_ids(&self.pool, user_id).await
    }

    pub async fn get_created_mcp_server_ids(
        &self,
        user_id: Uuid,
    ) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
        get_created_mcp_server_ids(&self.pool, user_id).await
    }

    pub async fn get_created_model_ids(&self) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
        get_created_model_ids(&self.pool).await
    }

    pub async fn delete_hub_tracking(
        &self,
        entity_type: HubEntityType,
        entity_id: Uuid,
    ) -> Result<(), AppError> {
        delete_hub_tracking(&self.pool, entity_type, entity_id).await
    }

    /// Every hub_entities row whose installed hub_version differs from
    /// the current catalog version (NULL is also "behind"). Backs
    /// `GET /api/hub/updates`.
    pub async fn list_outdated_entities(
        &self,
        current_version: &str,
    ) -> Result<Vec<OutdatedHubEntity>, AppError> {
        list_outdated_entities(&self.pool, current_version).await
    }

    /// The admin-pinned catalog version (without leading `v`), or None
    /// when tracking latest. Reads the hub_settings singleton.
    pub async fn get_pinned_version(&self) -> Result<Option<String>, AppError> {
        get_pinned_version(&self.pool).await
    }

    /// Look up the `hub_entities` row for a hub-installed TEMPLATE
    /// assistant (matched by hub_id with `created_by IS NULL`).
    /// Returns Some(entity_id) when a template install already exists
    /// for this hub_id — used by the `Hub.createAssistantTemplateFromHub`
    /// handler to refuse duplicate installs (each duplicate fans out
    /// to every new user via the clone-on-signup hook, multiplying
    /// the runtime cost).
    pub async fn find_template_install(
        &self,
        hub_id: &str,
    ) -> Result<Option<Uuid>, AppError> {
        find_template_install(&self.pool, hub_id).await
    }

    /// System-wide template installs keyed by hub_id (companion to
    /// `get_created_assistant_ids` which is per-user). Used to merge
    /// `created_template_ids` into the catalog response so the hub
    /// card can disable the "Use as Template" button when a template
    /// is already installed.
    pub async fn get_template_install_ids(
        &self,
    ) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
        get_template_install_ids(&self.pool).await
    }

    /// Set (or clear, with None) the admin-pinned catalog version.
    pub async fn set_pinned_version(
        &self,
        version: Option<&str>,
    ) -> Result<(), AppError> {
        set_pinned_version(&self.pool, version).await
    }
}

/// Row returned by `list_outdated_entities` — one installed hub
/// entity whose version doesn't match the current catalog. The
/// `installed_version` is None for rows installed before migration 69.
#[derive(Debug, Clone)]
pub struct OutdatedHubEntity {
    pub hub_id: String,
    pub hub_category: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub installed_version: Option<String>,
    /// True when this hub_entities row has `created_by IS NULL` —
    /// i.e. it's a system-wide install (template assistant, or any
    /// future entity type that bypasses per-user ownership). The
    /// `/hub/updates` UI uses this to route the Re-install action
    /// to `Hub.createAssistantTemplateFromHub` instead of the
    /// user-scoped `Hub.createAssistantFromHub` — without this
    /// discriminator, a "Re-install" on a template-origin row would
    /// silently create a USER assistant owned by the clicking admin,
    /// leaving the stale template still outdated.
    pub is_template_install: bool,
}

/// Create hub entity tracking record. `hub_version` is the catalog
/// version the entity was installed from — stamped so `/hub/updates`
/// can tell whether the install is behind the current catalog. NULL
/// only for legacy rows that predate migration 69.
pub async fn track_hub_entity(
    pool: &PgPool,
    entity_type: HubEntityType,
    entity_id: Uuid,
    hub_id: &str,
    hub_category: HubCategory,
    created_by: Option<Uuid>,
    hub_version: Option<&str>,
) -> Result<HubEntity, AppError> {
    let entity_type_str = entity_type.as_str();
    let hub_category_str = hub_category.as_str();

    let record = sqlx::query!(
        r#"
        INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by, hub_version)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (entity_type, entity_id)
        DO UPDATE SET hub_id = EXCLUDED.hub_id, hub_category = EXCLUDED.hub_category, hub_version = EXCLUDED.hub_version
        RETURNING id, entity_type, entity_id, hub_id, hub_category, created_at, created_by
        "#,
        entity_type_str,
        entity_id,
        hub_id,
        hub_category_str,
        created_by,
        hub_version
    )
    .fetch_one(pool)
    .await?;

    Ok(HubEntity {
        id: record.id,
        entity_type: record.entity_type,
        entity_id: record.entity_id,
        hub_id: record.hub_id,
        hub_category: record.hub_category,
        created_at: DateTime::from_timestamp(record.created_at.unix_timestamp(), 0).unwrap(),
        created_by: record.created_by,
    })
}

/// Get created entity IDs for assistants (user-specific)
pub async fn get_created_assistant_ids(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
    let records = sqlx::query!(
        r#"
        SELECT he.hub_id, ARRAY_AGG(he.entity_id) as entity_ids
        FROM hub_entities he
        INNER JOIN assistants a ON a.id = he.entity_id
        WHERE he.entity_type = 'assistant'
          AND he.created_by = $1
        GROUP BY he.hub_id
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::new();
    for record in records {
        if let Some(entity_ids) = record.entity_ids {
            map.insert(record.hub_id, entity_ids);
        }
    }

    Ok(map)
}

/// Get TEMPLATE assistant IDs that have been installed from each
/// hub catalog id (system-wide — templates have NULL created_by).
/// Used to surface a "Template installed" indicator on the hub card
/// so admins don't accidentally install duplicates. Returns a map
/// hub_id → list of template assistant ids; usually 0-or-1 entries
/// per hub_id (the backend rejects duplicates) but kept as Vec for
/// future flexibility.
pub async fn get_template_install_ids(
    pool: &PgPool,
) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
    let records = sqlx::query!(
        r#"
        SELECT he.hub_id, ARRAY_AGG(he.entity_id) as entity_ids
        FROM hub_entities he
        INNER JOIN assistants a ON a.id = he.entity_id
        WHERE he.entity_type = 'assistant'
          AND he.created_by IS NULL
          AND a.is_template = true
        GROUP BY he.hub_id
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::new();
    for record in records {
        if let Some(entity_ids) = record.entity_ids {
            map.insert(record.hub_id, entity_ids);
        }
    }

    Ok(map)
}

/// Get created entity IDs for MCP servers (user-specific, including system servers user has access to)
pub async fn get_created_mcp_server_ids(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
    let records = sqlx::query!(
        r#"
        SELECT he.hub_id, ARRAY_AGG(DISTINCT ms.id) as entity_ids
        FROM hub_entities he
        INNER JOIN mcp_servers ms ON ms.id = he.entity_id
        WHERE he.entity_type = 'mcp_server'
          AND he.created_by = $1
          AND (ms.user_id = $1 OR ms.is_system = true)
        GROUP BY he.hub_id
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::new();
    for record in records {
        if let Some(entity_ids) = record.entity_ids {
            map.insert(record.hub_id, entity_ids);
        }
    }

    Ok(map)
}

/// Get created entity IDs for models (system-wide, no user filter).
///
/// Returns ALL hub-tracked downloads regardless of completion state
/// (previously filtered by `di.model_id IS NOT NULL`, which hid
/// in-progress downloads from the hub list — the UI couldn't tell
/// whether a fresh start was already in flight, leading to duplicate
/// download attempts and a flaky test race in
/// hub::test_create_model_from_hub / test_duplicate_download_prevention).
pub async fn get_created_model_ids(pool: &PgPool) -> Result<HashMap<String, Vec<Uuid>>, AppError> {
    let records = sqlx::query!(
        r#"
        SELECT he.hub_id, ARRAY_AGG(he.entity_id) as entity_ids
        FROM hub_entities he
        INNER JOIN download_instances di ON di.id = he.entity_id
        WHERE he.entity_type = 'llm_model'
        GROUP BY he.hub_id
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::new();
    for record in records {
        if let Some(entity_ids) = record.entity_ids {
            map.insert(record.hub_id, entity_ids);
        }
    }

    Ok(map)
}

/// List entities whose installed hub_version is not the current
/// catalog version. NULL counts as "behind" so legacy rows always
/// surface as updatable.
pub async fn list_outdated_entities(
    pool: &PgPool,
    current_version: &str,
) -> Result<Vec<OutdatedHubEntity>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT hub_id, hub_category, entity_type, entity_id, hub_version, created_by
        FROM hub_entities
        WHERE hub_version IS DISTINCT FROM $1
        ORDER BY hub_category, hub_id
        "#,
        current_version
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OutdatedHubEntity {
            hub_id: r.hub_id,
            hub_category: r.hub_category,
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            installed_version: r.hub_version,
            is_template_install: r.created_by.is_none(),
        })
        .collect())
}

/// Find an existing TEMPLATE install (created_by IS NULL) for a hub
/// assistant. Returns the `entity_id` (= assistant id) when one
/// exists. Used to enforce idempotency in
/// `Hub.createAssistantTemplateFromHub` — without this guard the
/// admin clicking "Use as Template" twice would create two identical
/// templates, each of which clones into every new user's assistant
/// list via the signup hook.
pub async fn find_template_install(
    pool: &PgPool,
    hub_id: &str,
) -> Result<Option<Uuid>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT entity_id
        FROM hub_entities
        WHERE entity_type = 'assistant'
          AND hub_id = $1
          AND created_by IS NULL
        LIMIT 1
        "#,
        hub_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.entity_id))
}

/// Read the pinned catalog version from the hub_settings singleton.
pub async fn get_pinned_version(pool: &PgPool) -> Result<Option<String>, AppError> {
    let row = sqlx::query!("SELECT pinned_version FROM hub_settings WHERE id = TRUE")
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|r| r.pinned_version))
}

/// Set or clear the pinned catalog version on the hub_settings singleton.
pub async fn set_pinned_version(
    pool: &PgPool,
    version: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE hub_settings SET pinned_version = $1, updated_at = NOW() WHERE id = TRUE",
        version
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete hub tracking record
pub async fn delete_hub_tracking(
    pool: &PgPool,
    entity_type: HubEntityType,
    entity_id: Uuid,
) -> Result<(), AppError> {
    let entity_type_str = entity_type.as_str();

    sqlx::query!(
        "DELETE FROM hub_entities WHERE entity_type = $1 AND entity_id = $2",
        entity_type_str,
        entity_id
    )
    .execute(pool)
    .await?;

    Ok(())
}
