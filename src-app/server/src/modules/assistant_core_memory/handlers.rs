use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use uuid::Uuid;

use super::models::{CoreMemoryBlock, UpsertCoreMemoryBlockRequest};
use super::permissions::{CoreMemoryRead, CoreMemoryWrite};
use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};

const MAX_BLOCK_LABEL_LEN: usize = 64;
const MAX_CONTENT_LEN: usize = 50_000;

fn is_valid_block_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_BLOCK_LABEL_LEN
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[debug_handler]
pub async fn list_blocks(
    auth: RequirePermissions<(CoreMemoryRead,)>,
    Path(assistant_id): Path<Uuid>,
) -> ApiResult<Json<Vec<CoreMemoryBlock>>> {
    let rows = Repos
        .assistant_core_memory
        .list_for_user_assistant(auth.user.id, assistant_id)
        .await?;
    Ok((StatusCode::OK, Json(rows)))
}

pub fn list_blocks_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CoreMemoryRead,)>(op)
        .id("CoreMemory.list")
        .tag("CoreMemory")
        .summary("List the caller's core memory blocks for an assistant")
        .response::<200, Json<Vec<CoreMemoryBlock>>>()
}

#[debug_handler]
pub async fn upsert_block(
    auth: RequirePermissions<(CoreMemoryWrite,)>,
    Json(body): Json<UpsertCoreMemoryBlockRequest>,
) -> ApiResult<Json<CoreMemoryBlock>> {
    if !is_valid_block_label(&body.block_label) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "block_label must be 1-64 chars of [a-z0-9_-]",
        )
        .into());
    }
    if body.content.len() > MAX_CONTENT_LEN {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "content exceeds 50000 char limit",
        )
        .into());
    }
    if !(1..=50_000).contains(&body.char_limit) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "char_limit must be 1..=50000",
        )
        .into());
    }

    let row = Repos
        .assistant_core_memory
        .upsert(
            auth.user.id,
            body.assistant_id,
            &body.block_label,
            &body.content,
            body.char_limit,
        )
        .await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn upsert_block_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CoreMemoryWrite,)>(op)
        .id("CoreMemory.upsert")
        .tag("CoreMemory")
        .summary("Create or update a core memory block")
        .response::<200, Json<CoreMemoryBlock>>()
}

#[debug_handler]
pub async fn delete_block(
    auth: RequirePermissions<(CoreMemoryWrite,)>,
    Path((assistant_id, block_label)): Path<(Uuid, String)>,
) -> ApiResult<StatusCode> {
    if !is_valid_block_label(&block_label) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "block_label must be 1-64 chars of [a-z0-9_-]",
        )
        .into());
    }
    let deleted = Repos
        .assistant_core_memory
        .delete(auth.user.id, assistant_id, &block_label)
        .await?;
    if !deleted {
        return Err(AppError::not_found("CoreMemoryBlock").into());
    }
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_block_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(CoreMemoryWrite,)>(op)
        .id("CoreMemory.delete")
        .tag("CoreMemory")
        .summary("Delete a core memory block")
        .response_with::<204, (), _>(|r| r.description("Deleted"))
}
