//! Deployment-wide JWT session settings (singleton).
//!
//! Backs the admin-configurable access-token TTL + max session length
//! (migration 129). The YAML `jwt.access_token_expiry_hours` /
//! `jwt.refresh_token_expiry_days` values are copied into the row ONCE at
//! boot (`seed_from_config_once`); thereafter the DB row is authoritative
//! and is read at every token mint (`mint_session_tokens` in handlers.rs),
//! falling back to the YAML values only if the DB read fails.
//!
//! Pattern mirror: `web_search_settings` (repository COALESCE
//! partial-update + GET/PUT handlers + `SyncEntity` notify).

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

use super::permissions::{SessionSettingsManage, SessionSettingsRead};

// ─────────────────────────────── DTOs ───────────────────────────────

/// Deployment-wide JWT session settings (singleton row). Returned by GET.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SessionSettings {
    /// Access-token TTL in hours (1..=8760).
    pub access_token_expiry_hours: i32,
    /// Max session length in days (1..=3650) — the refresh-token TTL.
    /// Active sessions roll on every refresh, so this is the idle bound.
    pub refresh_token_expiry_days: i32,
    pub updated_at: DateTime<Utc>,
}

/// PUT body for the session settings. Every field optional → absent = leave.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateSessionSettingsRequest {
    #[serde(default)]
    pub access_token_expiry_hours: Option<i32>,
    #[serde(default)]
    pub refresh_token_expiry_days: Option<i32>,
}

// ───────────────────────────── Repository ─────────────────────────────

#[derive(Clone, Debug)]
pub struct SessionSettingsRepository {
    pool: PgPool,
}

impl SessionSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self) -> Result<SessionSettings, AppError> {
        let row = sqlx::query_as!(
            SessionSettings,
            r#"
            SELECT
                access_token_expiry_hours,
                refresh_token_expiry_days,
                updated_at as "updated_at: _"
            FROM session_settings
            WHERE id = TRUE
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    pub async fn update(
        &self,
        access_token_expiry_hours: Option<i32>,
        refresh_token_expiry_days: Option<i32>,
    ) -> Result<SessionSettings, AppError> {
        // Also latch `seeded_from_config = TRUE`: an admin edit is an
        // explicit choice that must survive, so the one-time boot seed
        // (`seed_from_config_once`, a detached task) becomes a no-op even if
        // the admin's PUT happens to land before the seed task runs — closing
        // a first-boot clobber window.
        let row = sqlx::query_as!(
            SessionSettings,
            r#"
            UPDATE session_settings SET
                access_token_expiry_hours = COALESCE($1, access_token_expiry_hours),
                refresh_token_expiry_days = COALESCE($2, refresh_token_expiry_days),
                seeded_from_config        = TRUE,
                updated_at                = NOW()
            WHERE id = TRUE
            RETURNING
                access_token_expiry_hours,
                refresh_token_expiry_days,
                updated_at as "updated_at: _"
            "#,
            access_token_expiry_hours,
            refresh_token_expiry_days,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// One-time boot copy of the YAML jwt lifetimes into the singleton row.
    ///
    /// Runs on every boot but writes only while `seeded_from_config` is
    /// FALSE (i.e. exactly once per deployment lifetime — a fresh install
    /// or the first boot after the migration). Carries an operator's
    /// customized YAML values into the DB so upgrading doesn't silently
    /// reset their lifetimes to the migration defaults. Values outside
    /// the DB CHECK ranges are clamped rather than failing boot.
    pub async fn seed_from_config_once(
        &self,
        config_access_hours: i64,
        config_refresh_days: i64,
    ) -> Result<(), AppError> {
        let access = config_access_hours.clamp(1, 8760) as i32;
        let refresh = config_refresh_days.clamp(1, 3650) as i32;
        sqlx::query!(
            r#"
            UPDATE session_settings SET
                access_token_expiry_hours = $1,
                refresh_token_expiry_days = $2,
                seeded_from_config        = TRUE,
                updated_at                = NOW()
            WHERE id = TRUE AND seeded_from_config = FALSE
            "#,
            access,
            refresh,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }
}

// ─────────────────────────── REST handlers ───────────────────────────

#[debug_handler]
pub async fn get_session_settings(
    _auth: RequirePermissions<(SessionSettingsRead,)>,
) -> ApiResult<Json<SessionSettings>> {
    let row = Repos.session_settings.get().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_session_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SessionSettingsRead,)>(op)
        .id("Auth.getSessionSettings")
        .tag("auth")
        .summary("Read session settings (access-token TTL + max session length)")
        .response::<200, Json<SessionSettings>>()
}

#[debug_handler]
pub async fn update_session_settings(
    _auth: RequirePermissions<(SessionSettingsManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateSessionSettingsRequest>,
) -> ApiResult<Json<SessionSettings>> {
    if let Some(n) = body.access_token_expiry_hours
        && !(1..=8760).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "access_token_expiry_hours out of range (1..=8760)",
        )
        .into());
    }
    if let Some(n) = body.refresh_token_expiry_days
        && !(1..=3650).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "refresh_token_expiry_days out of range (1..=3650)",
        )
        .into());
    }

    let row = Repos
        .session_settings
        .update(body.access_token_expiry_hours, body.refresh_token_expiry_days)
        .await?;

    sync_publish(
        SyncEntity::SessionSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<SessionSettingsRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_session_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SessionSettingsManage,)>(op)
        .id("Auth.updateSessionSettings")
        .tag("auth")
        .summary("Update session settings (access-token TTL + max session length)")
        .response::<200, Json<SessionSettings>>()
}
