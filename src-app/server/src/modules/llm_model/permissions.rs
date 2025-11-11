// LLM Model permissions
// Following ziee-chat patterns from llm_provider module

use crate::modules::permissions::PermissionCheck;

// LLM Models permissions
pub struct LlmModelsRead;
impl PermissionCheck for LlmModelsRead {
    const NAME: &'static str = "LlmModelsRead";
    const PERMISSION: &'static str = "llm_models::read";
    const DESCRIPTION: &'static str = "Read LLM models";
    const MODULE: &'static str = "llm_model";
}

pub struct LlmModelsCreate;
impl PermissionCheck for LlmModelsCreate {
    const NAME: &'static str = "LlmModelsCreate";
    const PERMISSION: &'static str = "llm_models::create";
    const DESCRIPTION: &'static str = "Create new LLM models";
    const MODULE: &'static str = "llm_model";
}

pub struct LlmModelsEdit;
impl PermissionCheck for LlmModelsEdit {
    const NAME: &'static str = "LlmModelsEdit";
    const PERMISSION: &'static str = "llm_models::edit";
    const DESCRIPTION: &'static str = "Edit existing LLM models";
    const MODULE: &'static str = "llm_model";
}

pub struct LlmModelsDelete;
impl PermissionCheck for LlmModelsDelete {
    const NAME: &'static str = "LlmModelsDelete";
    const PERMISSION: &'static str = "llm_models::delete";
    const DESCRIPTION: &'static str = "Delete LLM models";
    const MODULE: &'static str = "llm_model";
}

// Download management permissions
pub struct LlmModelsDownloadsRead;
impl PermissionCheck for LlmModelsDownloadsRead {
    const NAME: &'static str = "LlmModelsDownloadsRead";
    const PERMISSION: &'static str = "llm_models::downloads_read";
    const DESCRIPTION: &'static str = "Read download instances";
    const MODULE: &'static str = "llm_model";
}

pub struct LlmModelsDownloadsCancel;
impl PermissionCheck for LlmModelsDownloadsCancel {
    const NAME: &'static str = "LlmModelsDownloadsCancel";
    const PERMISSION: &'static str = "llm_models::downloads_cancel";
    const DESCRIPTION: &'static str = "Cancel active downloads";
    const MODULE: &'static str = "llm_model";
}

pub struct LlmModelsDownloadsDelete;
impl PermissionCheck for LlmModelsDownloadsDelete {
    const NAME: &'static str = "LlmModelsDownloadsDelete";
    const PERMISSION: &'static str = "llm_models::downloads_delete";
    const DESCRIPTION: &'static str = "Delete download instances";
    const MODULE: &'static str = "llm_model";
}
