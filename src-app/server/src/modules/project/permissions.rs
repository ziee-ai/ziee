// Project permissions
//
// Single namespace `projects::*` — personal-only v1, no template/sharing
// distinction. File attach/detach is gated by `projects::edit` plus a
// server-side ownership check on `file_id` (no separate
// `project_files::manage` needed — would be deadweight).

use crate::modules::permissions::PermissionCheck;

pub struct ProjectsCreate;
impl PermissionCheck for ProjectsCreate {
    const NAME: &'static str = "ProjectsCreate";
    const PERMISSION: &'static str = "projects::create";
    const DESCRIPTION: &'static str = "Create chat projects";
    const MODULE: &'static str = "project";
}

pub struct ProjectsRead;
impl PermissionCheck for ProjectsRead {
    const NAME: &'static str = "ProjectsRead";
    const PERMISSION: &'static str = "projects::read";
    const DESCRIPTION: &'static str = "Read chat projects";
    const MODULE: &'static str = "project";
}

pub struct ProjectsEdit;
impl PermissionCheck for ProjectsEdit {
    const NAME: &'static str = "ProjectsEdit";
    const PERMISSION: &'static str = "projects::edit";
    const DESCRIPTION: &'static str = "Edit chat projects (incl. attach/detach files)";
    const MODULE: &'static str = "project";
}

pub struct ProjectsDelete;
impl PermissionCheck for ProjectsDelete {
    const NAME: &'static str = "ProjectsDelete";
    const PERMISSION: &'static str = "projects::delete";
    const DESCRIPTION: &'static str = "Delete chat projects";
    const MODULE: &'static str = "project";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four permission constants must match the strings used in
    /// migration 54 (the Administrators grant). Drift here would
    /// leave the migration granting strings nobody checks for.
    #[test]
    fn permission_strings_match_migration() {
        assert_eq!(ProjectsCreate::PERMISSION, "projects::create");
        assert_eq!(ProjectsRead::PERMISSION, "projects::read");
        assert_eq!(ProjectsEdit::PERMISSION, "projects::edit");
        assert_eq!(ProjectsDelete::PERMISSION, "projects::delete");
    }

    #[test]
    fn permission_modules_are_consistent() {
        for module in [
            ProjectsCreate::MODULE,
            ProjectsRead::MODULE,
            ProjectsEdit::MODULE,
            ProjectsDelete::MODULE,
        ] {
            assert_eq!(module, "project");
        }
    }

    #[test]
    fn permission_names_are_distinct() {
        let names = [
            ProjectsCreate::NAME,
            ProjectsRead::NAME,
            ProjectsEdit::NAME,
            ProjectsDelete::NAME,
        ];
        let mut sorted = names.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "permission NAME constants must be distinct");
    }

    #[test]
    fn permission_descriptions_are_non_empty() {
        for desc in [
            ProjectsCreate::DESCRIPTION,
            ProjectsRead::DESCRIPTION,
            ProjectsEdit::DESCRIPTION,
            ProjectsDelete::DESCRIPTION,
        ] {
            assert!(!desc.is_empty(), "permission DESCRIPTION must be non-empty");
        }
    }
}

