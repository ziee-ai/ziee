//! Permission keys for the memory module.

use crate::modules::permissions::types::PermissionCheck;

/// List + read own memories.
pub struct MemoryRead;
impl PermissionCheck for MemoryRead {
    const NAME: &'static str = "MemoryRead";
    const PERMISSION: &'static str = "memory::read";
    const DESCRIPTION: &'static str = "List and read own memories.";
    const MODULE: &'static str = "memory";
}

/// Create / update / delete own memories.
pub struct MemoryWrite;
impl PermissionCheck for MemoryWrite {
    const NAME: &'static str = "MemoryWrite";
    const PERMISSION: &'static str = "memory::write";
    const DESCRIPTION: &'static str = "Create, edit, and delete own memories.";
    const MODULE: &'static str = "memory";
}

/// Read deployment-wide memory admin settings.
pub struct MemoryAdminRead;
impl PermissionCheck for MemoryAdminRead {
    const NAME: &'static str = "MemoryAdminRead";
    const PERMISSION: &'static str = "memory::admin::read";
    const DESCRIPTION: &'static str = "Read memory admin settings (embedding model, defaults).";
    const MODULE: &'static str = "memory";
}

/// Mutate deployment-wide memory admin settings.
pub struct MemoryAdminManage;
impl PermissionCheck for MemoryAdminManage {
    const NAME: &'static str = "MemoryAdminManage";
    const PERMISSION: &'static str = "memory::admin::manage";
    const DESCRIPTION: &'static str = "Update memory admin settings (embedding model, enable/disable).";
    const MODULE: &'static str = "memory";
}
