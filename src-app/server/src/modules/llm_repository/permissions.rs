use crate::modules::permissions::PermissionCheck;

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

#[cfg(test)]
mod tests {
    // audit id all-710690387070 — the permission file had no test pinning the
    // PERMISSION strings / MODULE / NAME constants. Mirrors
    // project/permissions.rs::tests so drift between these constants and the
    // migration that grants them (and the FE Permissions enum scraped from the
    // OpenAPI examples) fails the suite.
    use super::*;

    #[test]
    fn permission_strings_are_stable() {
        assert_eq!(LlmRepositoriesRead::PERMISSION, "llm_repositories::read");
        assert_eq!(LlmRepositoriesCreate::PERMISSION, "llm_repositories::create");
        assert_eq!(LlmRepositoriesEdit::PERMISSION, "llm_repositories::edit");
        assert_eq!(LlmRepositoriesDelete::PERMISSION, "llm_repositories::delete");
    }

    #[test]
    fn permission_modules_are_consistent() {
        for module in [
            LlmRepositoriesRead::MODULE,
            LlmRepositoriesCreate::MODULE,
            LlmRepositoriesEdit::MODULE,
            LlmRepositoriesDelete::MODULE,
        ] {
            assert_eq!(module, "llm_repository");
        }
    }

    #[test]
    fn permission_names_are_distinct() {
        let names = [
            LlmRepositoriesRead::NAME,
            LlmRepositoriesCreate::NAME,
            LlmRepositoriesEdit::NAME,
            LlmRepositoriesDelete::NAME,
        ];
        let mut sorted = names.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "permission NAME constants must be distinct");
    }

    #[test]
    fn permission_strings_are_namespaced_under_module() {
        for p in [
            LlmRepositoriesRead::PERMISSION,
            LlmRepositoriesCreate::PERMISSION,
            LlmRepositoriesEdit::PERMISSION,
            LlmRepositoriesDelete::PERMISSION,
        ] {
            assert!(p.starts_with("llm_repositories::"), "{p} must be llm_repositories-namespaced");
        }
    }
}
