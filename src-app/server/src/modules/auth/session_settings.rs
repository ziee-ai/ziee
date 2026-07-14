//! Deployment-wide JWT session settings — REST handlers (app-side HTTP/aide
//! boundary).
//!
//! The schema-bound DTOs (`SessionSettings` / `UpdateSessionSettingsRequest`)
//! and the repository (`query!` macros) moved to `ziee-auth` (Chunk BA-full);
//! this module keeps the permission-gated GET/PUT handlers + the `SyncEntity`
//! notify (the app's aide/permission-extractor + concrete sync enum can't live
//! in the SDK). Re-exports the DTOs/repo so `crate::modules::auth::
//! session_settings::…` call sites are unchanged.

use aide::transform::TransformOperation;
use axum::Extension;
use axum::{Json, debug_handler, http::StatusCode};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncOrigin};

// Re-export the moved DTOs + repository (equivalence-preserving shim).
pub use ziee_auth::auth::session_settings::{SessionSettings, UpdateSessionSettingsRequest};

use super::context::{AuthContext, AuthSyncAction, AuthSyncEntity};
use super::permissions::{SessionSettingsManage, SessionSettingsRead};

// ─────────────────────────── REST handlers ───────────────────────────

#[debug_handler]
pub async fn get_session_settings(
    _auth: RequirePermissions<(SessionSettingsRead,)>,
    Extension(ctx): Extension<AuthContext>,
) -> ApiResult<Json<SessionSettings>> {
    let row = ctx.session_settings().get().await?;
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
    Extension(ctx): Extension<AuthContext>,
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

    let row = ctx
        .session_settings()
        .update(body.access_token_expiry_hours, body.refresh_token_expiry_days)
        .await?;

    ctx.sync.publish(
        AuthSyncEntity::SessionSettings,
        AuthSyncAction::Update,
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
