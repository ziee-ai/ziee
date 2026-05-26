use crate::modules::permissions::types::PermissionCheck;

pub struct CoreMemoryRead;
impl PermissionCheck for CoreMemoryRead {
    const NAME: &'static str = "CoreMemoryRead";
    const PERMISSION: &'static str = "memory::core::read";
    const DESCRIPTION: &'static str = "Read own assistant core memory blocks.";
    const MODULE: &'static str = "assistant_core_memory";
}

pub struct CoreMemoryWrite;
impl PermissionCheck for CoreMemoryWrite {
    const NAME: &'static str = "CoreMemoryWrite";
    const PERMISSION: &'static str = "memory::core::write";
    const DESCRIPTION: &'static str = "Upsert / delete own assistant core memory blocks.";
    const MODULE: &'static str = "assistant_core_memory";
}
