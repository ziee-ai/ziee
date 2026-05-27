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
use crate::modules::file::types::{DOWNLOAD_TOKEN_AUDIENCE, DownloadTokenClaims, DownloadTokenQuery, DownloadTokenResponse};
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;
use uuid::Uuid;

const TOKEN_EXPIRY: i64 = 3600; // 1 hour

/// Build a `Content-Disposition: attachment; filename=...; filename*=UTF-8''...`
/// header value that is safe regardless of what the stored filename
/// contains. Closes 05-file F-08 (Medium): the previous implementation
/// did `format!("attachment; filename=\"{}\"", file.filename)` which
/// inserted user-controlled bytes directly into the header — a
/// filename like `evil";\r\nSet-Cookie: foo=bar` would inject
/// arbitrary response headers (CRLF injection) or break out of the
/// quoted form to confuse downstream parsers.
///
/// Rationale:
///   - filename= form: ASCII-only, all unsafe bytes replaced with '_'.
///     Browsers fall back to this when filename* is missing or unparseable.
///   - filename*=UTF-8''<percent-encoded>: RFC 5987 form. Carries the
///     real (multibyte) filename. Percent-encoding makes CRLF / quote
///     injection impossible.
fn content_disposition(filename: &str) -> String {
    let ascii: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let percent: String = filename
        .bytes()
        .flat_map(|b| {
            // RFC 5987 attr-char set + percent-encode everything else.
            if b.is_ascii_alphanumeric() || matches!(b, b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~') {
                vec![b]
            } else {
                format!("%{:02X}", b).into_bytes()
            }
        })
        .map(|b| b as char)
        .collect();
    format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii, percent
    )
}

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
            content_disposition(&file.filename),
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

    // Generate JWT token. Sets iss + aud (audience=ziee-download)
    // so the validator below can refuse cross-audience replay against
    // the access-token validator. Closes 02-permissions F-03.
    let jwt_config = get_jwt_config();
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = DownloadTokenClaims {
        file_id: file_id.to_string(),
        user_id: user_id.to_string(),
        exp: now + TOKEN_EXPIRY as usize,
        iat: now,
        iss: jwt_config.issuer.clone(),
        aud: DOWNLOAD_TOKEN_AUDIENCE.to_string(),
    };

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
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !user.is_active {
        return Err(StatusCode::FORBIDDEN);
    }
    // Re-verify FilesDownload via the same checker the extractor uses,
    // pulling in the user's current group permissions.
    let groups = Repos
        .user
        .get_user_groups(user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !user.is_admin
        && !crate::modules::permissions::checker::check_permission_union(
            &user,
            &groups,
            "files::download",
        )
    {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get file (this query already filters by user ownership).
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
            content_disposition(&file.filename),
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
