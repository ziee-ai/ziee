//! Database repository for runtime versions

use sqlx::PgPool;
use crate::modules::llm_local_runtime::runtime_version::models::RuntimeVersion;
use chrono::DateTime;
use uuid::Uuid;

/// Create a new runtime version record
pub async fn create(
    pool: &PgPool,
    engine: &str,
    version: &str,
    platform: &str,
    arch: &str,
    backend: &str,
    binary_path: &str,
) -> Result<RuntimeVersion, sqlx::Error> {
    let record = sqlx::query!(
        r#"INSERT INTO llm_runtime_versions
           (engine, version, platform, arch, backend, binary_path, is_system_default)
           VALUES ($1, $2, $3, $4, $5, $6, false)
           RETURNING id, engine, version, platform, arch, backend, binary_path,
                     is_system_default, created_at"#,
        engine,
        version,
        platform,
        arch,
        backend,
        binary_path
    )
    .fetch_one(pool)
    .await?;

    Ok(RuntimeVersion {
        id: record.id,
        engine: record.engine,
        version: record.version,
        platform: record.platform,
        arch: record.arch,
        backend: record.backend,
        binary_path: record.binary_path,
        is_system_default: record.is_system_default,
        created_at: DateTime::from_timestamp(record.created_at.unix_timestamp(), 0).unwrap_or_default(),
    })
}

/// Get runtime version by ID
pub async fn get_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        engine: r.engine,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Get runtime version by engine and version string
pub async fn get_by_engine_and_version(
    pool: &PgPool,
    engine: &str,
    version: &str,
) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           WHERE engine = $1 AND version = $2"#,
        engine,
        version
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        engine: r.engine,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Maximum page size — acts as a safety cap.
const MAX_PAGE_SIZE: i64 = 500;

/// List all runtime versions with pagination.
pub async fn list_all(
    pool: &PgPool,
    page: i64,
    per_page: i64,
) -> Result<Vec<RuntimeVersion>, sqlx::Error> {
    let offset = (page.max(1) - 1) * per_page.min(MAX_PAGE_SIZE);
    let limit = per_page.clamp(1, MAX_PAGE_SIZE);
    let records = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           ORDER BY engine, created_at DESC
           LIMIT $1 OFFSET $2"#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    Ok(records
        .into_iter()
        .map(|r| RuntimeVersion {
            id: r.id,
            engine: r.engine,
            version: r.version,
            platform: r.platform,
            arch: r.arch,
            backend: r.backend,
            binary_path: r.binary_path,
            is_system_default: r.is_system_default,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
        })
        .collect())
}

/// List runtime versions for a specific engine with pagination.
pub async fn list_by_engine(
    pool: &PgPool,
    engine: &str,
    page: i64,
    per_page: i64,
) -> Result<Vec<RuntimeVersion>, sqlx::Error> {
    let offset = (page.max(1) - 1) * per_page.min(MAX_PAGE_SIZE);
    let limit = per_page.clamp(1, MAX_PAGE_SIZE);
    let records = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           WHERE engine = $1
           ORDER BY created_at DESC
           LIMIT $2 OFFSET $3"#,
        engine,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    Ok(records
        .into_iter()
        .map(|r| RuntimeVersion {
            id: r.id,
            engine: r.engine,
            version: r.version,
            platform: r.platform,
            arch: r.arch,
            backend: r.backend,
            binary_path: r.binary_path,
            is_system_default: r.is_system_default,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
        })
        .collect())
}

/// Get the latest runtime version for an engine
pub async fn get_latest_version(
    pool: &PgPool,
    engine: &str,
) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           WHERE engine = $1
           ORDER BY created_at DESC
           LIMIT 1"#,
        engine
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        engine: r.engine,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Get the system default runtime version for an engine
pub async fn get_system_default(
    pool: &PgPool,
    engine: &str,
) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           WHERE engine = $1 AND is_system_default = true"#,
        engine
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        engine: r.engine,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Clear system default flag for all versions of an engine
pub async fn clear_system_default(pool: &PgPool, engine: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE llm_runtime_versions
           SET is_system_default = false
           WHERE engine = $1"#,
        engine
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Set a runtime version as system default
pub async fn set_system_default(
    pool: &PgPool,
    version_id: Uuid,
    is_default: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE llm_runtime_versions
           SET is_system_default = $2
           WHERE id = $1"#,
        version_id,
        is_default
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Delete a runtime version
pub async fn delete(pool: &PgPool, version_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"DELETE FROM llm_runtime_versions WHERE id = $1"#,
        version_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Direct FK dependents of a runtime version: providers defaulting to it and
/// running instances. The FKs are `ON DELETE SET NULL`, so the DB would
/// silently null these out instead of erroring — the delete guard refuses
/// instead. Model usage is computed separately via *effective resolution* (a
/// model can depend on a version without an explicit pin — e.g. via the
/// system default), so it is not counted here.
pub struct VersionUsage {
    /// Providers defaulting to it via `default_runtime_version_id`.
    pub providers: i64,
    /// Instances currently `running` on it.
    pub running_instances: i64,
}

/// A local model row + its provider name and live-running flag, for the
/// models-by-version interface. The effective engine version is resolved
/// separately (the fallback chain lives in `BinaryManager`).
pub struct LocalModelRow {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub engine: String,
    pub required_runtime_version_id: Option<Uuid>,
    pub running: bool,
}

/// Degenerate-case backstop for the unfiltered local-models scan. This view
/// must return *all* local models (the usage grouping has no natural page), so
/// the cap only guards against a pathological/OOM result set, far above any
/// realistic count of local-provider models in a deployment.
const LOCAL_MODELS_HARD_CAP: i64 = 5000;

/// List local-provider models (optionally filtered by engine), each with its
/// provider name and whether a runtime instance is currently `running`.
pub async fn list_local_models_with_status(
    pool: &PgPool,
    engine: Option<&str>,
) -> Result<Vec<LocalModelRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT m.id, m.name, m.display_name, m.provider_id,
               p.name AS provider_name,
               m.engine_type AS engine,
               m.required_runtime_version_id,
               (i.id IS NOT NULL) AS "running!"
        FROM llm_models m
        JOIN llm_providers p
          ON p.id = m.provider_id AND p.provider_type = 'local'
        LEFT JOIN llm_runtime_instances i
          ON i.model_id = m.id AND i.status = 'running'
        WHERE ($1::text IS NULL OR m.engine_type = $1)
        ORDER BY p.name, m.display_name
        LIMIT $2
        "#,
        engine,
        LOCAL_MODELS_HARD_CAP,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LocalModelRow {
            id: r.id,
            name: r.name,
            display_name: r.display_name,
            provider_id: r.provider_id,
            provider_name: r.provider_name,
            engine: r.engine,
            required_runtime_version_id: r.required_runtime_version_id,
            running: r.running,
        })
        .collect())
}

/// Batched effective-runtime-version resolution for many models at once.
///
/// Mirrors the precedence in `BinaryManager::select_runtime_version`
/// (model-required → provider-default → system-default → latest) but resolves
/// the entire set in a constant number of queries instead of ~4 queries per
/// model. Returns a map `model_id -> effective_version_id` (absent/`None` when
/// no version resolves). Used by the version-usage view, which previously
/// issued an N+1 storm of `select_runtime_version` calls.
pub async fn resolve_effective_versions(
    pool: &PgPool,
    models: &[LocalModelRow],
) -> Result<std::collections::HashMap<Uuid, Option<Uuid>>, sqlx::Error> {
    use std::collections::{HashMap, HashSet};

    if models.is_empty() {
        return Ok(HashMap::new());
    }

    let provider_ids: Vec<Uuid> = models
        .iter()
        .map(|m| m.provider_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let engines: Vec<String> = models
        .iter()
        .map(|m| m.engine.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Provider default version ids (one query).
    let provider_default: HashMap<Uuid, Option<Uuid>> = sqlx::query!(
        "SELECT id, default_runtime_version_id FROM llm_providers WHERE id = ANY($1)",
        &provider_ids
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| (r.id, r.default_runtime_version_id))
    .collect();

    // System default per engine (one query).
    let system_default: HashMap<String, Uuid> = sqlx::query!(
        r#"SELECT DISTINCT ON (engine) engine, id
           FROM llm_runtime_versions
           WHERE engine = ANY($1) AND is_system_default = true"#,
        &engines
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| (r.engine, r.id))
    .collect();

    // Latest version per engine (one query).
    let latest: HashMap<String, Uuid> = sqlx::query!(
        r#"SELECT DISTINCT ON (engine) engine, id
           FROM llm_runtime_versions
           WHERE engine = ANY($1)
           ORDER BY engine, created_at DESC"#,
        &engines
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| (r.engine, r.id))
    .collect();

    // Which referenced version ids (model-required + provider-default) actually
    // exist — `select_runtime_version` only honors them when the row resolves.
    let mut candidate_ids: HashSet<Uuid> = HashSet::new();
    for m in models {
        if let Some(v) = m.required_runtime_version_id {
            candidate_ids.insert(v);
        }
        if let Some(Some(v)) = provider_default.get(&m.provider_id) {
            candidate_ids.insert(*v);
        }
    }
    let candidate_vec: Vec<Uuid> = candidate_ids.into_iter().collect();
    let existing: HashSet<Uuid> = if candidate_vec.is_empty() {
        HashSet::new()
    } else {
        sqlx::query!(
            "SELECT id FROM llm_runtime_versions WHERE id = ANY($1)",
            &candidate_vec
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| r.id)
        .collect()
    };

    let mut out: HashMap<Uuid, Option<Uuid>> = HashMap::with_capacity(models.len());
    for m in models {
        let resolved = m
            .required_runtime_version_id
            .filter(|v| existing.contains(v))
            .or_else(|| {
                provider_default
                    .get(&m.provider_id)
                    .copied()
                    .flatten()
                    .filter(|v| existing.contains(v))
            })
            .or_else(|| system_default.get(&m.engine).copied())
            .or_else(|| latest.get(&m.engine).copied());
        out.insert(m.id, resolved);
    }

    Ok(out)
}

/// Set (or clear) a model's pinned runtime version.
pub async fn set_model_runtime_version(
    pool: &PgPool,
    model_id: Uuid,
    version_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE llm_models SET required_runtime_version_id = $2 WHERE id = $1",
        model_id,
        version_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Count provider-default + running-instance dependents of `version_id`.
pub async fn usage(pool: &PgPool, version_id: Uuid) -> Result<VersionUsage, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
          (SELECT COUNT(*) FROM llm_providers
             WHERE default_runtime_version_id = $1) AS "providers!",
          (SELECT COUNT(*) FROM llm_runtime_instances
             WHERE runtime_version_id = $1 AND status = 'running') AS "running_instances!"
        "#,
        version_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(VersionUsage {
        providers: row.providers,
        running_instances: row.running_instances,
    })
}
