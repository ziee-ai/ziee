// File download handlers

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::file::config::get_jwt_config;
use crate::modules::file::permissions::{FilesDownload, FilesGenerateToken};
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::file::types::{DownloadTokenClaims, DownloadTokenQuery, DownloadTokenResponse};
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;
use uuid::Uuid;

const TOKEN_EXPIRY: i64 = 3600; // 1 hour

/// Download file directly (requires authentication)
pub async fn download_file(
    auth: RequirePermissions<(FilesDownload,)>,
    Path(file_id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let user_id = auth.user.id;

    // Get file and verify ownership
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Extract extension
    let extension = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();

    // Load file data
    let storage = get_file_storage();
    let file_data = storage
        .load_original(user_id, file_id, &extension)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Build response headers using array of tuples (like reference implementation)
    let headers = [
        (
            header::CONTENT_TYPE,
            file.mime_type
                .as_deref()
                .unwrap_or("application/octet-stream")
                .to_string(),
        ),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file.filename),
        ),
        (header::CONTENT_LENGTH, file_data.len().to_string()),
    ];

    Ok((headers, file_data).into_response())
}

/// Generate download token
pub async fn generate_download_token(
    auth: RequirePermissions<(FilesGenerateToken,)>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Json<DownloadTokenResponse>> {
    let user_id = auth.user.id;

    // Verify file exists and user owns it
    Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Generate JWT token
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = DownloadTokenClaims {
        file_id: file_id.to_string(),
        user_id: user_id.to_string(),
        exp: now + TOKEN_EXPIRY as usize,
        iat: now,
    };

    let jwt_config = get_jwt_config();
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_config.secret.as_bytes()),
    )
    .map_err(|e| AppError::internal_error(format!("Failed to generate token: {}", e)))?;

    Ok((
        StatusCode::OK,
        Json(DownloadTokenResponse {
            token,
            expires_in: TOKEN_EXPIRY,
        }),
    ))
}

/// Download file with token (no authentication required)
pub async fn download_with_token(
    Path(file_id): Path<Uuid>,
    Query(query): Query<DownloadTokenQuery>,
) -> Result<Response, StatusCode> {
    // Validate token
    let jwt_config = get_jwt_config();
    let claims = decode::<DownloadTokenClaims>(
        &query.token,
        &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .claims;

    // Verify file_id matches
    let token_file_id = Uuid::parse_str(&claims.file_id)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if token_file_id != file_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let user_id = Uuid::parse_str(&claims.user_id)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Get file
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Extract extension
    let extension = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();

    // Load file data
    let storage = get_file_storage();
    let file_data = storage
        .load_original(user_id, file_id, &extension)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Build response headers using array of tuples (like reference implementation)
    let headers = [
        (
            header::CONTENT_TYPE,
            file.mime_type
                .as_deref()
                .unwrap_or("application/octet-stream")
                .to_string(),
        ),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file.filename),
        ),
        (header::CONTENT_LENGTH, file_data.len().to_string()),
    ];

    Ok((headers, file_data).into_response())
}

/// Download file OpenAPI documentation
pub fn download_file_docs(op: TransformOperation) -> TransformOperation {
    use crate::modules::file::types::BlobType;

    with_permission::<(FilesDownload,)>(op)
        .id("File.download")
        .tag("Files")
        .summary("Download file")
        .description("Download the original file (requires authentication)")
        .response::<200, Json<BlobType>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

/// Generate download token OpenAPI documentation
pub fn generate_download_token_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FilesGenerateToken,)>(op)
        .id("File.generateDownloadToken")
        .tag("Files")
        .summary("Generate download token")
        .description("Generate a time-limited token for downloading a file without authentication")
        .response::<200, Json<DownloadTokenResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
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
