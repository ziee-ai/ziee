//! Skill lifecycle events.
//!
//! Notify-and-refetch only — the per-conversation chat extension + the
//! `skill_mcp` `list_tools` response refetch on the corresponding
//! `sync:<entity>` event. Rich row data lives in the REST endpoints.


use uuid::Uuid;

use crate::modules::skill::permissions::{SkillsManageSystem, SkillsRead};
use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};

/// User-scope skill create/update/delete — notify only the owner's
/// connections.
pub fn emit_user_skill(action: SyncAction, skill_id: Uuid, owner_user_id: Uuid, origin: Option<Uuid>) {
    sync_publish(SyncEntity::Skill, action, skill_id, Audience::owner(owner_user_id), origin);
}

/// System-scope skill create/update/delete. Dual-fan-out per plan §3:
/// `SkillSystem` to admins (for the admin list), `Skill` to all users
/// who can read skills (for their available-skills listing).
pub fn emit_system_skill(action: SyncAction, skill_id: Uuid, origin: Option<Uuid>) {
    sync_publish(
        SyncEntity::SkillSystem,
        action,
        skill_id,
        Audience::perm::<SkillsManageSystem>(),
        origin,
    );
    sync_publish(
        SyncEntity::Skill,
        action,
        skill_id,
        Audience::perm::<SkillsRead>(),
        origin,
    );
}
