//! Permission keys for the Document-RAG (file_rag) module.
//!
//! Only admin settings are gated here. Per-user retrieval (the
//! `semantic_search` MCP tool) reuses `files::read` at the MCP layer and the
//! conversation-scoped file set, so it needs no file_rag-specific user perm.
//! Administrators hold both keys below via the `*` wildcard.

use crate::modules::permissions::types::PermissionCheck;

/// Read deployment-wide Document-RAG admin settings.
pub struct FileRagAdminRead;
impl PermissionCheck for FileRagAdminRead {
    const NAME: &'static str = "FileRagAdminRead";
    const PERMISSION: &'static str = "file_rag::admin::read";
    const DESCRIPTION: &'static str = "Read Document-RAG admin settings (embedding model, tuning).";
    const MODULE: &'static str = "file_rag";
}

/// Mutate deployment-wide Document-RAG admin settings + trigger backfill/reembed.
pub struct FileRagAdminManage;
impl PermissionCheck for FileRagAdminManage {
    const NAME: &'static str = "FileRagAdminManage";
    const PERMISSION: &'static str = "file_rag::admin::manage";
    const DESCRIPTION: &'static str =
        "Update Document-RAG admin settings, trigger re-embed and backfill.";
    const MODULE: &'static str = "file_rag";
}
