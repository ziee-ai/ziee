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

#[cfg(test)]
mod tests {
    use super::*;

    /// The permission strings must stay stable — handlers gate on these exact
    /// strings and the Administrators wildcard grant relies on them.
    #[test]
    fn permission_strings_are_stable() {
        assert_eq!(FileRagAdminRead::PERMISSION, "file_rag::admin::read");
        assert_eq!(FileRagAdminManage::PERMISSION, "file_rag::admin::manage");
    }

    #[test]
    fn permission_modules_are_consistent() {
        for module in [FileRagAdminRead::MODULE, FileRagAdminManage::MODULE] {
            assert_eq!(module, "file_rag");
        }
    }

    #[test]
    fn permission_names_are_distinct() {
        let names = [FileRagAdminRead::NAME, FileRagAdminManage::NAME];
        let mut sorted = names.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            names.len(),
            "permission NAME constants must be distinct"
        );
    }

    #[test]
    fn permission_descriptions_are_non_empty() {
        for desc in [FileRagAdminRead::DESCRIPTION, FileRagAdminManage::DESCRIPTION] {
            assert!(!desc.is_empty(), "permission DESCRIPTION must be non-empty");
        }
    }
}
