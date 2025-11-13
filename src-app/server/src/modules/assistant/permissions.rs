// Assistant permissions
// Two namespaces:
// - assistants::* for user-created assistants
// - assistant_templates::* for system-wide template assistants

use crate::modules::permissions::PermissionCheck;

// ============================================================
// User Assistants Permissions (assistants::*)
// ============================================================

pub struct AssistantsCreate;
impl PermissionCheck for AssistantsCreate {
    const NAME: &'static str = "AssistantsCreate";
    const PERMISSION: &'static str = "assistants::create";
    const DESCRIPTION: &'static str = "Create user assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsRead;
impl PermissionCheck for AssistantsRead {
    const NAME: &'static str = "AssistantsRead";
    const PERMISSION: &'static str = "assistants::read";
    const DESCRIPTION: &'static str = "Read user assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsEdit;
impl PermissionCheck for AssistantsEdit {
    const NAME: &'static str = "AssistantsEdit";
    const PERMISSION: &'static str = "assistants::edit";
    const DESCRIPTION: &'static str = "Edit user assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsDelete;
impl PermissionCheck for AssistantsDelete {
    const NAME: &'static str = "AssistantsDelete";
    const PERMISSION: &'static str = "assistants::delete";
    const DESCRIPTION: &'static str = "Delete user assistants";
    const MODULE: &'static str = "assistant";
}

#[allow(dead_code)]
pub struct AssistantsSetDefault;
impl PermissionCheck for AssistantsSetDefault {
    const NAME: &'static str = "AssistantsSetDefault";
    const PERMISSION: &'static str = "assistants::set_default";
    const DESCRIPTION: &'static str = "Set default user assistant";
    const MODULE: &'static str = "assistant";
}

// ============================================================
// Template Assistants Permissions (assistant_templates::*)
// ============================================================

pub struct AssistantsTemplateCreate;
impl PermissionCheck for AssistantsTemplateCreate {
    const NAME: &'static str = "AssistantsTemplateCreate";
    const PERMISSION: &'static str = "assistant_templates::create";
    const DESCRIPTION: &'static str = "Create system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateRead;
impl PermissionCheck for AssistantsTemplateRead {
    const NAME: &'static str = "AssistantsTemplateRead";
    const PERMISSION: &'static str = "assistant_templates::read";
    const DESCRIPTION: &'static str = "Read system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateEdit;
impl PermissionCheck for AssistantsTemplateEdit {
    const NAME: &'static str = "AssistantsTemplateEdit";
    const PERMISSION: &'static str = "assistant_templates::edit";
    const DESCRIPTION: &'static str = "Edit system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateDelete;
impl PermissionCheck for AssistantsTemplateDelete {
    const NAME: &'static str = "AssistantsTemplateDelete";
    const PERMISSION: &'static str = "assistant_templates::delete";
    const DESCRIPTION: &'static str = "Delete system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

#[allow(dead_code)]
pub struct AssistantsTemplateSetDefault;
impl PermissionCheck for AssistantsTemplateSetDefault {
    const NAME: &'static str = "AssistantsTemplateSetDefault";
    const PERMISSION: &'static str = "assistant_templates::set_default";
    const DESCRIPTION: &'static str = "Set default template assistant";
    const MODULE: &'static str = "assistant";
}
