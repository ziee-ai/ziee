// File download handlers

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, HeaderMap, StatusCode};
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
) -> Result<(HeaderMap, Vec<u8>), AppError> {
    let user_id = auth.user.id;

    // Get file and verify ownership
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Extract extension
    let extension = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();

    // Load file data
    let storage = get_file_storage();
    let file_data = storage.load_original(user_id, file_id, &extension).await?;

    // Build response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        file.mime_type
            .as_deref()
            .unwrap_or("application/octet-stream")
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", file.filename)
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        file_data.len().to_string().parse().unwrap(),
    );

    Ok((headers, file_data))
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
) -> Result<(HeaderMap, Vec<u8>), AppError> {
    // Validate token
    let jwt_config = get_jwt_config();
    let claims = decode::<DownloadTokenClaims>(
        &query.token,
        &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::unauthorized("INVALID_TOKEN", format!("Invalid token: {}", e)))?
    .claims;

    // Verify file_id matches
    let token_file_id = Uuid::parse_str(&claims.file_id)
        .map_err(|e| AppError::bad_request("INVALID_TOKEN", format!("Invalid file ID: {}", e)))?;

    if token_file_id != file_id {
        return Err(AppError::forbidden(
            "FILE_ID_MISMATCH",
            "Token file ID does not match request",
        ));
    }

    let user_id = Uuid::parse_str(&claims.user_id)
        .map_err(|e| AppError::bad_request("INVALID_TOKEN", format!("Invalid user ID: {}", e)))?;

    // Get file
    let file = Repos.file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    // Extract extension
    let extension = file
        .filename
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_lowercase();

    // Load file data
    let storage = get_file_storage();
    let file_data = storage.load_original(user_id, file_id, &extension).await?;

    // Build response headers
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        file.mime_type
            .as_deref()
            .unwrap_or("application/octet-stream")
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", file.filename)
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        file_data.len().to_string().parse().unwrap(),
    );

    Ok((headers, file_data))
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
