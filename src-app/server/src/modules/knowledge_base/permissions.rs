//! Permission keys for the knowledge_base module.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in `search_knowledge` / `list_knowledge_bases` tools and read
/// your own knowledge bases + attach them to your chats/projects. Granted to the
/// default Users group by migration 134. Knowledge bases are strictly per-user
/// (every query is `WHERE user_id = $1`), so a model searching the user's own
/// KBs is the same trust level as reading them — no cross-tenant exposure.
pub struct KnowledgeBaseUse;
impl PermissionCheck for KnowledgeBaseUse {
    const NAME: &'static str = "KnowledgeBaseUse";
    const PERMISSION: &'static str = "knowledge_base::use";
    const DESCRIPTION: &'static str =
        "Search your knowledge bases and attach them to conversations/projects.";
    const MODULE: &'static str = "knowledge_base";
}

/// Create / rename / delete knowledge bases and add or remove their documents.
/// Per-user data, so normal users hold this for their own KBs (migration 134).
pub struct KnowledgeBaseManage;
impl PermissionCheck for KnowledgeBaseManage {
    const NAME: &'static str = "KnowledgeBaseManage";
    const PERMISSION: &'static str = "knowledge_base::manage";
    const DESCRIPTION: &'static str =
        "Create, rename, delete knowledge bases and manage their documents.";
    const MODULE: &'static str = "knowledge_base";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_strings_match_migration_147() {
        // These MUST equal the strings granted in
        // migrations/00000000000147_grant_knowledge_base_permissions_to_users.sql
        assert_eq!(KnowledgeBaseUse::PERMISSION, "knowledge_base::use");
        assert_eq!(KnowledgeBaseManage::PERMISSION, "knowledge_base::manage");
    }
}
