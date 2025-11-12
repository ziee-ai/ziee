use crate::modules::permissions::{PermissionCheck, PermissionInfo};

// =====================================================
// Conversation Permissions
// =====================================================

/// Permission to create new conversations
pub struct ConversationsCreate;
impl PermissionCheck for ConversationsCreate {
    const NAME: &'static str = "ConversationsCreate";
    const PERMISSION: &'static str = "conversations::create";
    const DESCRIPTION: &'static str = "Create new chat conversations";
    const MODULE: &'static str = "chat";
}

/// Permission to read conversations
pub struct ConversationsRead;
impl PermissionCheck for ConversationsRead {
    const NAME: &'static str = "ConversationsRead";
    const PERMISSION: &'static str = "conversations::read";
    const DESCRIPTION: &'static str = "View chat conversations";
    const MODULE: &'static str = "chat";
}

/// Permission to update conversation metadata
pub struct ConversationsEdit;
impl PermissionCheck for ConversationsEdit {
    const NAME: &'static str = "ConversationsEdit";
    const PERMISSION: &'static str = "conversations::edit";
    const DESCRIPTION: &'static str = "Edit conversation titles and metadata";
    const MODULE: &'static str = "chat";
}

/// Permission to delete conversations
pub struct ConversationsDelete;
impl PermissionCheck for ConversationsDelete {
    const NAME: &'static str = "ConversationsDelete";
    const PERMISSION: &'static str = "conversations::delete";
    const DESCRIPTION: &'static str = "Delete chat conversations";
    const MODULE: &'static str = "chat";
}

// =====================================================
// Message Permissions
// =====================================================

/// Permission to send messages in conversations
pub struct MessagesCreate;
impl PermissionCheck for MessagesCreate {
    const NAME: &'static str = "MessagesCreate";
    const PERMISSION: &'static str = "messages::create";
    const DESCRIPTION: &'static str = "Send messages in conversations";
    const MODULE: &'static str = "chat";
}

/// Permission to read messages
pub struct MessagesRead;
impl PermissionCheck for MessagesRead {
    const NAME: &'static str = "MessagesRead";
    const PERMISSION: &'static str = "messages::read";
    const DESCRIPTION: &'static str = "Read messages in conversations";
    const MODULE: &'static str = "chat";
}

/// Permission to delete messages
pub struct MessagesDelete;
impl PermissionCheck for MessagesDelete {
    const NAME: &'static str = "MessagesDelete";
    const PERMISSION: &'static str = "messages::delete";
    const DESCRIPTION: &'static str = "Delete messages from conversations";
    const MODULE: &'static str = "chat";
}

// =====================================================
// Branch Permissions
// =====================================================

/// Permission to create message branches (edit/regenerate)
pub struct BranchesCreate;
impl PermissionCheck for BranchesCreate {
    const NAME: &'static str = "BranchesCreate";
    const PERMISSION: &'static str = "branches::create";
    const DESCRIPTION: &'static str = "Create message branches for edit/regenerate";
    const MODULE: &'static str = "chat";
}

/// Permission to switch between conversation branches
pub struct BranchesSwitch;
impl PermissionCheck for BranchesSwitch {
    const NAME: &'static str = "BranchesSwitch";
    const PERMISSION: &'static str = "branches::switch";
    const DESCRIPTION: &'static str = "Switch between conversation branches";
    const MODULE: &'static str = "chat";
}

// =====================================================
// Helper Function
// =====================================================

/// Get all chat module permissions
pub fn all_permissions() -> Vec<PermissionInfo> {
    vec![
        ConversationsCreate::to_info(),
        ConversationsRead::to_info(),
        ConversationsEdit::to_info(),
        ConversationsDelete::to_info(),
        MessagesCreate::to_info(),
        MessagesRead::to_info(),
        MessagesDelete::to_info(),
        BranchesCreate::to_info(),
        BranchesSwitch::to_info(),
    ]
}
