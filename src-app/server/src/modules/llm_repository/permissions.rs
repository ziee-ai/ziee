#[allow(unused_imports)]
use crate::modules::permissions::{PermissionCheck, PermissionInfo};

// =====================================================
// LLM Repository Management Permissions
// =====================================================

/// Permission to view LLM repositories
pub struct LlmRepositoriesRead;
impl PermissionCheck for LlmRepositoriesRead {
    const NAME: &'static str = "LlmRepositoriesRead";
    const PERMISSION: &'static str = "llm_repositories::read";
    const DESCRIPTION: &'static str = "View LLM repositories and list repositories";
    const MODULE: &'static str = "llm_repository";
}

/// Permission to create new LLM repositories
pub struct LlmRepositoriesCreate;
impl PermissionCheck for LlmRepositoriesCreate {
    const NAME: &'static str = "LlmRepositoriesCreate";
    const PERMISSION: &'static str = "llm_repositories::create";
    const DESCRIPTION: &'static str = "Create new LLM repositories";
    const MODULE: &'static str = "llm_repository";
}

/// Permission to edit existing LLM repositories
pub struct LlmRepositoriesEdit;
impl PermissionCheck for LlmRepositoriesEdit {
    const NAME: &'static str = "LlmRepositoriesEdit";
    const PERMISSION: &'static str = "llm_repositories::edit";
    const DESCRIPTION: &'static str = "Edit existing LLM repository information and authentication";
    const MODULE: &'static str = "llm_repository";
}

/// Permission to delete LLM repositories
pub struct LlmRepositoriesDelete;
impl PermissionCheck for LlmRepositoriesDelete {
    const NAME: &'static str = "LlmRepositoriesDelete";
    const PERMISSION: &'static str = "llm_repositories::delete";
    const DESCRIPTION: &'static str = "Delete non-built-in LLM repositories";
    const MODULE: &'static str = "llm_repository";
}

// =====================================================
// Helper Function to Collect All Permissions
// =====================================================
