// File download — the ziee-RETAINED handler.
//
// Chunk `ziee-file-http` moved the store-generic download handlers
// (`download_file`, `generate_download_token`) + the `content_disposition`
// helper + the two cache-control consts into `ziee_file::http`. This module
// keeps only `download_with_token`, which re-verifies identity BY user-id from
// the signed claim (`Repos.user.get_by_id` + active + `check_permission_union`)
// — a path that does not fit the request-`Parts`-based `IdentityResolver` seam,
// so it stays ziee-side. It re-exports the moved helper + consts from the SDK so
// existing `crate::modules::file::handlers::download::{content_disposition,
// FILE_CONTENT_CACHE_CONTROL, FILE_HEAD_CACHE_CONTROL}` paths (e.g. chat export)
// resolve unchanged.

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::config::get_jwt_config;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::types::{DOWNLOAD_TOKEN_AUDIENCE, DownloadTokenClaims, DownloadTokenQuery};
use uuid::Uuid;

// Re-export the moved helper so existing intra- and cross-module paths
// (`handlers::download::content_disposition` — the export handler + chat export)
// keep resolving through the SDK; it is also used below by `download_with_token`.
pub use ziee_file::http::content_disposition;
// Content cache-control for the token-download response (used below).
use ziee_file::http::FILE_CONTENT_CACHE_CONTROL;

/// Download file with token (no authentication required)
pub async fn download_with_token(
    Path(file_id): Path<Uuid>,
    Query(query): Query<DownloadTokenQuery>,
) -> ApiResult<Response> {
    // Validate token. Enforces iss + aud=DOWNLOAD_TOKEN_AUDIENCE so a
    // download token cannot be replayed against the access-token
    // validator (which expects aud="ziee-api"). Closes
    // 02-permissions F-03.
    let jwt_config = get_jwt_config();
    let mut validation = Validation::default();
    validation.set_audience(&[DOWNLOAD_TOKEN_AUDIENCE]);
    validation.set_issuer(&[jwt_config.issuer.as_str()]);
    let claims = decode::<DownloadTokenClaims>(
        &query.token,
        &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::unauthorized("INVALID_TOKEN", "Invalid or expired download token"))?
    .claims;

    // Verify file_id matches
    let token_file_id = Uuid::parse_str(&claims.file_id)
        .map_err(|_| AppError::unauthorized("INVALID_TOKEN", "Malformed token"))?;

    if token_file_id != file_id {
        return Err(AppError::forbidden("TOKEN_FILE_MISMATCH", "Token does not match file").into());
    }

    let user_id = Uuid::parse_str(&claims.user_id)
        .map_err(|_| AppError::unauthorized("INVALID_TOKEN", "Malformed token"))?;

    // SECURITY: re-check current state at download time. The audit's
    // 05-file F-06 (High) noted the original handler only validated the
    // token + file ownership — it did NOT re-check that the user is
    // still active and still holds files::download. So a download token
    // issued at time T1 (when the user had access) keeps working at
    // T2 even after admin disabled the account or revoked the
    // permission.
    let user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::unauthorized("USER_NOT_FOUND", "User no longer exists"))?;
    if !user.is_active {
        return Err(AppError::forbidden("USER_INACTIVE", "User account is disabled").into());
    }
    // Re-verify FilesDownload via the same checker the extractor uses,
    // pulling in the user's current group permissions.
    let groups = Repos
        .user
        .get_user_groups(user.id)
        .await?;
    if !user.is_admin
        && !crate::modules::permissions::checker::check_permission_union(
            &user,
            &groups,
            "files::download",
        )
    {
        return Err(AppError::forbidden("PERMISSION_REVOKED", "files::download no longer granted").into());
    }

    // Get file (this query already filters by user ownership).
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Extract extension (filename is on the parent — stable across versions)
    let extension = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();

    // Resolve which version's blob to serve: the pinned version from the SIGNED
    // claims, else the current head.
    let (blob_version_id, mime_type) = match claims.version {
        Some(v) => {
            let ver = Repos
                .file
                .get_version(file_id, v, user_id)
                .await?
                .ok_or_else(|| AppError::not_found("File version"))?;
            (ver.blob_version_id, ver.mime_type)
        }
        None => (file.blob_version_id, file.mime_type.clone()),
    };

    // Load that version's bytes (keyed by its blob_version_id).
    let storage = get_file_storage();
    let file_data = storage
        .load_original(user_id, blob_version_id, &extension)
        .await
        .map_err(|_| AppError::not_found("File"))?;

    // Build response headers using array of tuples (like reference implementation)
    let headers = [
        (
            header::CONTENT_TYPE,
            mime_type
                .as_deref()
                .unwrap_or("application/octet-stream")
                .to_string(),
        ),
        (
            header::CONTENT_DISPOSITION,
            content_disposition(&file.filename),
        ),
        (header::CONTENT_LENGTH, file_data.len().to_string()),
        // Private, bounded cache so reloads reuse bytes without a round-trip
        // while still re-checking access after the max-age window. See
        // FILE_CONTENT_CACHE_CONTROL.
        (header::CACHE_CONTROL, FILE_CONTENT_CACHE_CONTROL.to_string()),
    ];

    Ok((StatusCode::OK, (headers, file_data).into_response()))
}

/// Download file with token OpenAPI documentation
pub fn download_with_token_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;

    op.id("File.downloadWithToken")
        .tag("Files")
        .summary("Download file with token")
        .description("Download file using a time-limited token (no authentication required)")
        .response::<200, Json<BlobType>>()
        .response_with::<400, (), _>(|res| res.description("Invalid token"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}
