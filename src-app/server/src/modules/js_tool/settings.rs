//! Runtime-configurable limits for the built-in `run_js` tool.
//!
//! Singleton row in `js_tool_settings` (migration 135). At runtime,
//! [`super::settings_cache`] caches the row; the PUT handler invalidates the
//! cache so the next `run_js` invocation reads the new caps. The hot path maps
//! the row into [`super::limits::JsCaps`] via `JsCaps::from_settings`.
//!
//! Defaults match the prior hardcoded constants so a fresh install behaves
//! identically. Mirrors `code_sandbox::resource_limits`.

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::AppError;
use crate::modules::js_tool::repository::JsToolRepository;

/// One row of `js_tool_settings`. Field names match the SQL columns (snake_case)
/// so the sqlx `query_as` mapping is trivial. Byte caps are `i64` (BIGINT — up
/// to 4 GiB exceeds i32); the rest are `i32`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct JsToolSettings {
    /// `rquickjs` `set_memory_limit` (bytes).
    pub memory_bytes: i64,
    /// `rquickjs` `set_max_stack_size` (bytes).
    pub max_stack_bytes: i64,
    /// Active-execution wall-clock backstop (seconds; excludes approval waits).
    pub wall_secs: i32,
    /// Per-call approval wait before resolving as cancel (seconds).
    pub approval_timeout_secs: i32,
    /// Process-global concurrent-interpreter admission cap.
    pub max_concurrent_runs: i32,
    /// Per-run parallel sub-tool dispatch cap.
    pub max_concurrent_dispatch: i32,
    /// Per-run recorded sub-call trace cap.
    pub max_trace_entries: i32,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Admin PUT payload. All fields optional; absent fields preserve their existing
/// value (COALESCE PATCH). Mirrors `UpdateCodeSandboxResourceLimits`.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateJsToolSettings {
    pub memory_bytes: Option<i64>,
    pub max_stack_bytes: Option<i64>,
    pub wall_secs: Option<i32>,
    pub approval_timeout_secs: Option<i32>,
    pub max_concurrent_runs: Option<i32>,
    pub max_concurrent_dispatch: Option<i32>,
    pub max_trace_entries: Option<i32>,
}

impl UpdateJsToolSettings {
    /// Validate ranges before any DB write. The DB has `CHECK` constraints as a
    /// backstop, but a structured error here returns a clearer 422 than a
    /// Postgres constraint violation. Bounds mirror the SQL `CHECK`s in
    /// migration 135 (and prevent an admin from footgunning the server —
    /// e.g. `max_concurrent_runs × memory_bytes` OOM, or a multi-hour `wall`).
    pub fn validate(&self) -> Result<(), AppError> {
        fn bad(field: &str, msg: impl Into<String>) -> AppError {
            AppError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "JS_TOOL_LIMIT_OUT_OF_RANGE",
                format!("invalid {field}: {}", msg.into()),
            )
        }
        if let Some(v) = self.memory_bytes
            && !(16 * 1024 * 1024..=4 * 1024 * 1024 * 1024).contains(&v)
        {
            return Err(bad("memory_bytes", "must be in 16 MiB ..= 4 GiB"));
        }
        if let Some(v) = self.max_stack_bytes
            && !(64 * 1024..=64 * 1024 * 1024).contains(&v)
        {
            return Err(bad("max_stack_bytes", "must be in 64 KiB ..= 64 MiB"));
        }
        if let Some(v) = self.wall_secs
            && !(1..=3600).contains(&v)
        {
            return Err(bad("wall_secs", "must be in 1..=3600"));
        }
        if let Some(v) = self.approval_timeout_secs
            && !(5..=3600).contains(&v)
        {
            return Err(bad("approval_timeout_secs", "must be in 5..=3600"));
        }
        if let Some(v) = self.max_concurrent_runs
            && !(1..=256).contains(&v)
        {
            return Err(bad("max_concurrent_runs", "must be in 1..=256"));
        }
        if let Some(v) = self.max_concurrent_dispatch
            && !(1..=64).contains(&v)
        {
            return Err(bad("max_concurrent_dispatch", "must be in 1..=64"));
        }
        if let Some(v) = self.max_trace_entries
            && !(1..=10_000).contains(&v)
        {
            return Err(bad("max_trace_entries", "must be in 1..=10000"));
        }
        Ok(())
    }
}

impl JsToolRepository {
    /// Read the singleton settings row. Always `Some` because the migration
    /// seeds it; a missing row means a partially-migrated DB → internal error.
    pub async fn get_settings(&self) -> Result<JsToolSettings, AppError> {
        let row: Option<JsToolSettings> = sqlx::query_as(
            r#"
            SELECT memory_bytes, max_stack_bytes, wall_secs, approval_timeout_secs,
                   max_concurrent_runs, max_concurrent_dispatch, max_trace_entries,
                   created_at, updated_at
            FROM js_tool_settings
            WHERE id = TRUE
            "#,
        )
        .fetch_optional(self.pool())
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "js_tool: read settings");
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "database error",
            )
        })?;
        row.ok_or_else(|| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JS_TOOL_SETTINGS_MISSING",
                "js_tool_settings singleton row is missing — run migrations",
            )
        })
    }

    /// PATCH-style update: only `Some` fields are written. Returns the
    /// post-update row. Validation happens at the call site
    /// ([`UpdateJsToolSettings::validate`]).
    pub async fn update_settings(
        &self,
        patch: &UpdateJsToolSettings,
    ) -> Result<JsToolSettings, AppError> {
        let row: Option<JsToolSettings> = sqlx::query_as(
            r#"
            UPDATE js_tool_settings SET
                memory_bytes            = COALESCE($1, memory_bytes),
                max_stack_bytes         = COALESCE($2, max_stack_bytes),
                wall_secs               = COALESCE($3, wall_secs),
                approval_timeout_secs   = COALESCE($4, approval_timeout_secs),
                max_concurrent_runs     = COALESCE($5, max_concurrent_runs),
                max_concurrent_dispatch = COALESCE($6, max_concurrent_dispatch),
                max_trace_entries       = COALESCE($7, max_trace_entries),
                updated_at              = NOW()
            WHERE id = TRUE
            RETURNING memory_bytes, max_stack_bytes, wall_secs, approval_timeout_secs,
                      max_concurrent_runs, max_concurrent_dispatch, max_trace_entries,
                      created_at, updated_at
            "#,
        )
        .bind(patch.memory_bytes)
        .bind(patch.max_stack_bytes)
        .bind(patch.wall_secs)
        .bind(patch.approval_timeout_secs)
        .bind(patch.max_concurrent_runs)
        .bind(patch.max_concurrent_dispatch)
        .bind(patch.max_trace_entries)
        .fetch_optional(self.pool())
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "js_tool: update settings");
            // Surface a CHECK-constraint violation as 422 so the UI renders it.
            if let sqlx::Error::Database(db) = &e
                && db.constraint().is_some()
            {
                return AppError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "JS_TOOL_LIMIT_DB_CONSTRAINT",
                    format!(
                        "value rejected by DB constraint {:?}: {}",
                        db.constraint(),
                        db.message()
                    ),
                );
            }
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "database error",
            )
        })?;
        row.ok_or_else(|| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JS_TOOL_SETTINGS_MISSING",
                "js_tool_settings singleton row is missing — run migrations",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(p: UpdateJsToolSettings) -> bool {
        p.validate().is_ok()
    }
    fn err(p: UpdateJsToolSettings) -> bool {
        p.validate().is_err()
    }

    // TEST-38: validate() accepts empty + in-range boundaries, rejects out-of-range.
    #[test]
    fn empty_patch_validates() {
        assert!(ok(UpdateJsToolSettings::default()));
    }

    #[test]
    fn memory_bytes_bounds() {
        assert!(err(UpdateJsToolSettings { memory_bytes: Some(16 * 1024 * 1024 - 1), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { memory_bytes: Some(16 * 1024 * 1024), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { memory_bytes: Some(4 * 1024 * 1024 * 1024), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { memory_bytes: Some(4 * 1024 * 1024 * 1024 + 1), ..Default::default() }));
    }

    #[test]
    fn max_stack_bytes_bounds() {
        assert!(err(UpdateJsToolSettings { max_stack_bytes: Some(64 * 1024 - 1), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { max_stack_bytes: Some(64 * 1024), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_stack_bytes: Some(64 * 1024 * 1024 + 1), ..Default::default() }));
    }

    #[test]
    fn secs_and_counts_bounds() {
        assert!(err(UpdateJsToolSettings { wall_secs: Some(0), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { wall_secs: Some(1), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { wall_secs: Some(3601), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { approval_timeout_secs: Some(4), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { approval_timeout_secs: Some(5), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_concurrent_runs: Some(0), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { max_concurrent_runs: Some(256), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_concurrent_runs: Some(257), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_concurrent_dispatch: Some(0), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { max_concurrent_dispatch: Some(64), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_concurrent_dispatch: Some(65), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_trace_entries: Some(0), ..Default::default() }));
        assert!(ok(UpdateJsToolSettings { max_trace_entries: Some(10_000), ..Default::default() }));
        assert!(err(UpdateJsToolSettings { max_trace_entries: Some(10_001), ..Default::default() }));
    }
}
