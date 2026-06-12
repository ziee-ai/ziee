//! Per-turn sandbox version-back.
//!
//! A chat extension whose `after_llm_call` fires once the MCP tool loop has
//! finished (order > MCP's 30 — the registry stops calling `after_llm_call` at
//! the first extension that returns `Continue`, so this only runs when MCP
//! returns `Complete` = turn end). It checksum-diffs every provenance-tracked
//! workspace file and appends a new version of the backing file when its bytes
//! changed in the sandbox this turn (per-turn coalescing; no-op when unchanged).

use async_trait::async_trait;
use axum::response::sse::Event;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionAction, ExtensionEntry, ExtensionMetadata,
    StreamContext,
};
use crate::modules::chat::core::models::Message;
use crate::modules::file::storage::manager::get_file_storage;

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "code_sandbox_version_back",
    order: 35,
};

pub struct SandboxVersionBackExtension;

pub fn create(_pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(SandboxVersionBackExtension)
}

#[distributed_slice(CHAT_EXTENSIONS)]
static SANDBOX_VERSION_BACK_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};

#[async_trait]
impl ChatExtension for SandboxVersionBackExtension {
    fn name(&self) -> &str {
        "code_sandbox_version_back"
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        // Best-effort: a version-back failure must never break the chat turn.
        if let Err(e) = reconcile_workspace_versions(
            context.conversation_id,
            context.user_id,
            Some(final_message.id),
        )
        .await
        {
            tracing::warn!(error = ?e, "code_sandbox: workspace version-back failed");
        }
        Ok(ExtensionAction::Complete)
    }
}

/// Checksum-diff every provenance-tracked workspace file; append a new version
/// of the backing file when its bytes changed since the last commit. Idempotent
/// (a re-run with no further change is a no-op).
pub async fn reconcile_workspace_versions(
    conversation_id: Uuid,
    user_id: Uuid,
    turn_message_id: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(state) = crate::modules::code_sandbox::config::get_state() else {
        return Ok(()); // sandbox not initialized / disabled
    };
    let provenance = Repos
        .code_sandbox
        .list_workspace_provenance(conversation_id)
        .await?;
    if provenance.is_empty() {
        return Ok(());
    }
    let workspace = state.workspace_root.join(conversation_id.to_string());
    let storage = get_file_storage();

    for row in provenance {
        // Defense-in-depth: stage_editable_files validates the relpath (no '/',
        // no NUL) before insertion, but never trust a path read back from storage
        // for a filesystem join — reject anything that could escape the workspace.
        if row.workspace_relpath.contains('/')
            || row.workspace_relpath.contains("..")
            || row.workspace_relpath.contains('\0')
        {
            tracing::warn!(
                relpath = %row.workspace_relpath,
                "version_back: refusing suspicious workspace path"
            );
            continue;
        }
        let dest = workspace.join(&row.workspace_relpath);
        let bytes = match tokio::fs::read(&dest).await {
            Ok(b) => b,
            Err(_) => continue, // deleted in workspace → keep last version
        };
        let new_checksum = storage.calculate_checksum(&bytes);
        // Compare to the base version's checksum (no-op if unchanged).
        let base = match Repos.file.get_version_by_id(row.base_version_id, user_id).await? {
            Some(v) => v,
            None => {
                tracing::warn!(
                    base_version_id = %row.base_version_id,
                    relpath = %row.workspace_relpath,
                    "version_back: base version missing; workspace change not committed"
                );
                continue;
            }
        };
        if base.checksum.as_deref() == Some(new_checksum.as_str()) {
            continue;
        }
        let Some(file) = Repos.file.get_by_id_and_user(row.file_id, user_id).await? else {
            continue; // file deleted mid-turn (provenance cascade); nothing to version
        };
        if let Some(version) = crate::modules::file::versioning::commit_new_version(
            user_id,
            &file,
            bytes,
            "sandbox",
            turn_message_id,
        )
        .await?
        {
            if let Err(e) = Repos
                .code_sandbox
                .update_workspace_base(conversation_id, &row.workspace_relpath, version.id)
                .await
            {
                tracing::info!(
                    error = ?e,
                    file_id = %row.file_id,
                    "version_back: workspace file removed before base update"
                );
            }
        }
    }
    Ok(())
}
