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
        created_at: DateTime::from_timestamp(record.created_at.unix_timestamp(), 0).unwrap(),
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
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
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
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
    }))
}

/// List all runtime versions
pub async fn list_all(pool: &PgPool) -> Result<Vec<RuntimeVersion>, sqlx::Error> {
    let records = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           ORDER BY engine, created_at DESC"#
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
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        })
        .collect())
}

/// List runtime versions for a specific engine
pub async fn list_by_engine(
    pool: &PgPool,
    engine: &str,
) -> Result<Vec<RuntimeVersion>, sqlx::Error> {
    let records = sqlx::query!(
        r#"SELECT id, engine, version, platform, arch, backend, binary_path,
                  is_system_default, created_at
           FROM llm_runtime_versions
           WHERE engine = $1
           ORDER BY created_at DESC"#,
        engine
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
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
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
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
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
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
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
