//! Settings Repository
//!
//! Desktop-specific key-value settings storage

use sqlx::PgPool;

pub struct SettingsRepository {
    pool: PgPool,
}

impl SettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a setting value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_scalar::<_, String>(
            "SELECT value FROM desktop_settings WHERE key = $1"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Set a setting value (upsert)
    pub async fn set(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO desktop_settings (key, value, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()
            "#
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a setting
    pub async fn delete(&self, key: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM desktop_settings WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all settings
    pub async fn get_all(&self) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT key, value FROM desktop_settings ORDER BY key"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
