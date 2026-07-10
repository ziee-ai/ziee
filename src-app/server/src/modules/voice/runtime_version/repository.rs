//! Database repository for whisper runtime versions (`voice_runtime_versions`).
//!
//! Single-engine analog of `llm_local_runtime::runtime_version::repository`
//! (no `engine` column). Plain async fns over the pool; queries are
//! compile-time-checked against the build DB (migration 151 creates the table).

use crate::modules::voice::runtime_version::models::RuntimeVersion;
use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

/// Create a new runtime version record.
pub async fn create(
    pool: &PgPool,
    version: &str,
    platform: &str,
    arch: &str,
    backend: &str,
    binary_path: &str,
) -> Result<RuntimeVersion, sqlx::Error> {
    let record = sqlx::query!(
        r#"INSERT INTO voice_runtime_versions
           (version, platform, arch, backend, binary_path, is_system_default)
           VALUES ($1, $2, $3, $4, $5, false)
           RETURNING id, version, platform, arch, backend, binary_path,
                     is_system_default, created_at"#,
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
        version: record.version,
        platform: record.platform,
        arch: record.arch,
        backend: record.backend,
        binary_path: record.binary_path,
        is_system_default: record.is_system_default,
        created_at: DateTime::from_timestamp(record.created_at.unix_timestamp(), 0)
            .unwrap_or_default(),
    })
}

/// Get a runtime version by id.
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM voice_runtime_versions
           WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Get a runtime version by its (version, platform, arch, backend) identity —
/// the dedup lookup the download+register path uses before inserting.
pub async fn get_by_identity(
    pool: &PgPool,
    version: &str,
    platform: &str,
    arch: &str,
    backend: &str,
) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM voice_runtime_versions
           WHERE version = $1 AND platform = $2 AND arch = $3 AND backend = $4"#,
        version,
        platform,
        arch,
        backend
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
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

/// List all runtime versions (paginated, newest first).
pub async fn list_all(
    pool: &PgPool,
    page: i64,
    per_page: i64,
) -> Result<Vec<RuntimeVersion>, sqlx::Error> {
    let limit = per_page.clamp(1, MAX_PAGE_SIZE);
    let offset = (page.max(1) - 1) * limit;
    let records = sqlx::query!(
        r#"SELECT id, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM voice_runtime_versions
           ORDER BY created_at DESC
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
            version: r.version,
            platform: r.platform,
            arch: r.arch,
            backend: r.backend,
            binary_path: r.binary_path,
            is_system_default: r.is_system_default,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0)
                .unwrap_or_default(),
        })
        .collect())
}

/// Get the latest runtime version (by `created_at`).
pub async fn get_latest_version(pool: &PgPool) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM voice_runtime_versions
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Get the system default runtime version, if one is set.
pub async fn get_system_default(pool: &PgPool) -> Result<Option<RuntimeVersion>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM voice_runtime_versions
           WHERE is_system_default = true"#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| RuntimeVersion {
        id: r.id,
        version: r.version,
        platform: r.platform,
        arch: r.arch,
        backend: r.backend,
        binary_path: r.binary_path,
        is_system_default: r.is_system_default,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap_or_default(),
    }))
}

/// Clear the system-default flag on every version (there is at most one).
pub async fn clear_system_default(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE voice_runtime_versions
           SET is_system_default = false
           WHERE is_system_default = true"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Set (or clear) the system-default flag on a specific version.
pub async fn set_system_default(
    pool: &PgPool,
    version_id: Uuid,
    is_default: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE voice_runtime_versions
           SET is_system_default = $2
           WHERE id = $1"#,
        version_id,
        is_default
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete a runtime version row.
pub async fn delete(pool: &PgPool, version_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"DELETE FROM voice_runtime_versions WHERE id = $1"#,
        version_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// FK dependents of a whisper runtime version. The singleton
/// `voice_runtime_instance.runtime_version_id` is `ON DELETE SET NULL`, so the
/// DB would silently null the instance out rather than erroring — the delete
/// guard counts it here and refuses instead.
pub struct VersionUsage {
    /// The singleton instance currently `running` on this version (0 or 1).
    pub running_instances: i64,
    /// The singleton instance referencing this version regardless of run state.
    pub referencing_instances: i64,
}

/// Count instance dependents of `version_id`.
pub async fn usage(pool: &PgPool, version_id: Uuid) -> Result<VersionUsage, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
          (SELECT COUNT(*) FROM voice_runtime_instance
             WHERE runtime_version_id = $1 AND status = 'running') AS "running_instances!",
          (SELECT COUNT(*) FROM voice_runtime_instance
             WHERE runtime_version_id = $1) AS "referencing_instances!"
        "#,
        version_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(VersionUsage {
        running_instances: row.running_instances,
        referencing_instances: row.referencing_instances,
    })
}
