//! Shared "operate on a workflow the model authored in its per-conversation
//! sandbox workspace" helpers.
//!
//! The single source of truth for confining a model-/client-supplied `dir` to
//! the CALLER's conversation workspace (`<workspace_root>/<conversation_id>/…`).
//! Used by the `workflow_mcp` `run_from_workspace` / `validate_from_workspace`
//! / `save_workflow` verbs and by the `workspace-save` / `workspace-export`
//! REST endpoints, so the traversal / absolute / symlink-escape guard can
//! never drift between the two surfaces.

use std::path::{Component, Path, PathBuf};

use uuid::Uuid;

use crate::common::AppError;

use super::runner;

/// Reject unless `conversation_id` is owned by `user_id`. The workspace verbs +
/// REST endpoints take a client-supplied `conversation_id`; without this a
/// caller could name ANOTHER user's conversation and read / pack / run that
/// conversation's sandbox-workspace files (cross-tenant IDOR). `get_conversation`
/// is owner-scoped (returns `None` for a non-owned or missing id) → 404, which
/// also avoids leaking whether the conversation exists.
pub async fn require_conversation_owner(
    conversation_id: Option<Uuid>,
    user_id: Uuid,
) -> Result<Uuid, AppError> {
    let conv_id = conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "WORKFLOW_NO_CONVERSATION",
            "this operation requires an active conversation (x-conversation-id)",
        )
    })?;
    crate::core::Repos
        .chat
        .core
        .get_conversation(conv_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("conversation"))?;
    Ok(conv_id)
}

/// Resolve `dir` to an absolute, existing directory under the caller's
/// per-conversation sandbox workspace. `dir` is always relative to the
/// caller's OWN conversation — there is no way to name another conversation's
/// or user's workspace. Rejects absolute paths, `..`, and (via canonicalize)
/// symlink escapes.
pub fn resolve_conversation_workspace_dir(
    conversation_id: Option<Uuid>,
    dir: &str,
) -> Result<PathBuf, AppError> {
    let conv = conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "WORKFLOW_NO_CONVERSATION",
            "this operation requires an active conversation (x-conversation-id)",
        )
    })?;
    if dir.is_empty() {
        return Err(AppError::bad_request(
            "WORKFLOW_DIR_REQUIRED",
            "'dir' (a workspace subdir) is required",
        ));
    }
    let rel = Path::new(dir);
    if rel.is_absolute() {
        return Err(AppError::bad_request(
            "WORKFLOW_WORKSPACE_BAD_DIR",
            "'dir' must be a relative path inside the conversation workspace",
        ));
    }
    // Only Normal components — no `..`, no root/prefix. `./` is tolerated.
    for c in rel.components() {
        match c {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(AppError::bad_request(
                    "WORKFLOW_WORKSPACE_BAD_DIR",
                    "'dir' must not contain '..' or absolute segments",
                ));
            }
        }
    }
    let base = runner::workflow_workspace_root().join(conv.to_string());
    let candidate = base.join(rel);
    // canonicalize resolves symlinks — the real escape guard. Requires the dir
    // to exist (the model must write the files first).
    let canon = candidate.canonicalize().map_err(|_| {
        AppError::bad_request(
            "WORKFLOW_WORKSPACE_MISSING",
            format!("workspace dir '{dir}' does not exist — write the files first"),
        )
    })?;
    let base_canon = base.canonicalize().unwrap_or(base);
    if !canon.starts_with(&base_canon) {
        return Err(AppError::bad_request(
            "WORKFLOW_WORKSPACE_ESCAPE",
            "'dir' resolves outside the conversation workspace",
        ));
    }
    if !canon.is_dir() {
        return Err(AppError::bad_request(
            "WORKFLOW_WORKSPACE_NOT_DIR",
            format!("workspace path '{dir}' is not a directory"),
        ));
    }
    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // The helper resolves against `runner::workflow_workspace_root()`, which in
    // tests falls back to a temp dir; to exercise confinement deterministically
    // we assert the pure component-level rejections that don't depend on the
    // root (absolute / traversal / empty / no-conversation), and cover the
    // existence + escape paths in the integration tier where the real root is
    // configured.

    #[test]
    fn t1_confine_requires_conversation_id() {
        let err = resolve_conversation_workspace_dir(None, "flow").unwrap_err();
        assert_eq!(err.error_code(), "WORKFLOW_NO_CONVERSATION");
    }

    #[test]
    fn t1_confine_rejects_absolute() {
        let err = resolve_conversation_workspace_dir(Some(Uuid::new_v4()), "/etc").unwrap_err();
        assert_eq!(err.error_code(), "WORKFLOW_WORKSPACE_BAD_DIR");
    }

    #[test]
    fn t1_confine_rejects_parent_traversal() {
        for bad in ["../../etc", "a/../../b", "..", "a/../../.."] {
            let err = resolve_conversation_workspace_dir(Some(Uuid::new_v4()), bad).unwrap_err();
            assert_eq!(
                err.error_code(),
                "WORKFLOW_WORKSPACE_BAD_DIR",
                "expected traversal rejection for {bad:?}"
            );
        }
    }

    #[test]
    fn t1_confine_rejects_empty() {
        let err = resolve_conversation_workspace_dir(Some(Uuid::new_v4()), "").unwrap_err();
        assert_eq!(err.error_code(), "WORKFLOW_DIR_REQUIRED");
    }

    #[test]
    fn t1_confine_rejects_missing_dir() {
        // A well-formed relative dir that doesn't exist under the (temp) root.
        let err =
            resolve_conversation_workspace_dir(Some(Uuid::new_v4()), "nope/here").unwrap_err();
        assert_eq!(err.error_code(), "WORKFLOW_WORKSPACE_MISSING");
    }

    #[test]
    fn t1_confine_accepts_nested_safe_dir() {
        // Build a real dir under the actual workspace root so canonicalize +
        // confinement succeed end-to-end.
        let conv = Uuid::new_v4();
        let base = runner::workflow_workspace_root().join(conv.to_string());
        let nested = base.join("proj/flow");
        fs::create_dir_all(&nested).unwrap();
        let out = resolve_conversation_workspace_dir(Some(conv), "proj/flow").unwrap();
        assert!(out.ends_with("proj/flow"));
        let _ = fs::remove_dir_all(&base);
    }
}
