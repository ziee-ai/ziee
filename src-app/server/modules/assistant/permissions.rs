// Assistant permissions
// Two namespaces:
// - assistants::* for user-created assistants
// - assistants-template::* for system-wide template assistants

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

pub struct AssistantsSetDefault;
impl PermissionCheck for AssistantsSetDefault {
    const NAME: &'static str = "AssistantsSetDefault";
    const PERMISSION: &'static str = "assistants::set_default";
    const DESCRIPTION: &'static str = "Set default user assistant";
    const MODULE: &'static str = "assistant";
}

// ============================================================
// Template Assistants Permissions (assistants-template::*)
// ============================================================

pub struct AssistantsTemplateCreate;
impl PermissionCheck for AssistantsTemplateCreate {
    const NAME: &'static str = "AssistantsTemplateCreate";
    const PERMISSION: &'static str = "assistants-template::create";
    const DESCRIPTION: &'static str = "Create system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateRead;
impl PermissionCheck for AssistantsTemplateRead {
    const NAME: &'static str = "AssistantsTemplateRead";
    const PERMISSION: &'static str = "assistants-template::read";
    const DESCRIPTION: &'static str = "Read system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateEdit;
impl PermissionCheck for AssistantsTemplateEdit {
    const NAME: &'static str = "AssistantsTemplateEdit";
    const PERMISSION: &'static str = "assistants-template::edit";
    const DESCRIPTION: &'static str = "Edit system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateDelete;
impl PermissionCheck for AssistantsTemplateDelete {
    const NAME: &'static str = "AssistantsTemplateDelete";
    const PERMISSION: &'static str = "assistants-template::delete";
    const DESCRIPTION: &'static str = "Delete system-wide template assistants";
    const MODULE: &'static str = "assistant";
}

pub struct AssistantsTemplateSetDefault;
impl PermissionCheck for AssistantsTemplateSetDefault {
    const NAME: &'static str = "AssistantsTemplateSetDefault";
    const PERMISSION: &'static str = "assistants-template::set_default";
    const DESCRIPTION: &'static str = "Set default template assistant";
    const MODULE: &'static str = "assistant";
}

// Export all permissions for registration
pub fn all_permissions() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // User assistants permissions
        (
            AssistantsCreate::PERMISSION,
            AssistantsCreate::DESCRIPTION,
            AssistantsCreate::MODULE,
        ),
        (
            AssistantsRead::PERMISSION,
            AssistantsRead::DESCRIPTION,
            AssistantsRead::MODULE,
        ),
        (
            AssistantsEdit::PERMISSION,
            AssistantsEdit::DESCRIPTION,
            AssistantsEdit::MODULE,
        ),
        (
            AssistantsDelete::PERMISSION,
            AssistantsDelete::DESCRIPTION,
            AssistantsDelete::MODULE,
        ),
        (
            AssistantsSetDefault::PERMISSION,
            AssistantsSetDefault::DESCRIPTION,
            AssistantsSetDefault::MODULE,
        ),
        // Template assistants permissions
        (
            AssistantsTemplateCreate::PERMISSION,
            AssistantsTemplateCreate::DESCRIPTION,
            AssistantsTemplateCreate::MODULE,
        ),
        (
            AssistantsTemplateRead::PERMISSION,
            AssistantsTemplateRead::DESCRIPTION,
            AssistantsTemplateRead::MODULE,
        ),
        (
            AssistantsTemplateEdit::PERMISSION,
            AssistantsTemplateEdit::DESCRIPTION,
            AssistantsTemplateEdit::MODULE,
        ),
        (
            AssistantsTemplateDelete::PERMISSION,
            AssistantsTemplateDelete::DESCRIPTION,
            AssistantsTemplateDelete::MODULE,
        ),
        (
            AssistantsTemplateSetDefault::PERMISSION,
            AssistantsTemplateSetDefault::DESCRIPTION,
            AssistantsTemplateSetDefault::MODULE,
        ),
    ]
}
