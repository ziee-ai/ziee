#[allow(unused_imports)]
use crate::modules::permissions::{PermissionCheck, PermissionInfo};

// =====================================================
// LLM Provider Management Permissions
// =====================================================

/// Permission to view LLM providers
pub struct LlmProvidersRead;
impl PermissionCheck for LlmProvidersRead {
    const NAME: &'static str = "LlmProvidersRead";
    const PERMISSION: &'static str = "llm_providers::read";
    const DESCRIPTION: &'static str = "View LLM providers and list available providers";
    const MODULE: &'static str = "llm_provider";
}

/// Permission to create new LLM providers
pub struct LlmProvidersCreate;
impl PermissionCheck for LlmProvidersCreate {
    const NAME: &'static str = "LlmProvidersCreate";
    const PERMISSION: &'static str = "llm_providers::create";
    const DESCRIPTION: &'static str = "Create new LLM provider configurations";
    const MODULE: &'static str = "llm_provider";
}

/// Permission to edit existing LLM providers
pub struct LlmProvidersEdit;
impl PermissionCheck for LlmProvidersEdit {
    const NAME: &'static str = "LlmProvidersEdit";
    const PERMISSION: &'static str = "llm_providers::edit";
    const DESCRIPTION: &'static str = "Edit existing LLM provider information and settings";
    const MODULE: &'static str = "llm_provider";
}

/// Permission to delete LLM providers
pub struct LlmProvidersDelete;
impl PermissionCheck for LlmProvidersDelete {
    const NAME: &'static str = "LlmProvidersDelete";
    const PERMISSION: &'static str = "llm_providers::delete";
    const DESCRIPTION: &'static str = "Delete non-built-in LLM providers";
    const MODULE: &'static str = "llm_provider";
}

/// Permission to assign LLM providers to user groups
pub struct LlmProvidersAssignGroups;
impl PermissionCheck for LlmProvidersAssignGroups {
    const NAME: &'static str = "LlmProvidersAssignGroups";
    const PERMISSION: &'static str = "llm_providers::assign_groups";
    const DESCRIPTION: &'static str = "Assign LLM providers to user groups";
    const MODULE: &'static str = "llm_provider";
}

// =====================================================
// Helper Function to Collect All Permissions
// =====================================================
