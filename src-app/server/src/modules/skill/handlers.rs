//! REST handlers for the skill module.
//!
//! User vs admin split mirrors `src/modules/mcp/handlers/{user,system,groups}.rs`.
//! User-scope endpoints sit under `/api/skills/*`; admin-scope under
//! `/api/skills/system/*`. The hub-install endpoints
//! (`POST /api/skills/install-from-hub` etc.) are thin wrappers around
//! the existing hub handlers — same compiled function bound to the
//! canonical user-facing path so clients don't need to know about the
//! hub URL namespace.

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Path,
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{SyncAction, SyncOrigin},
};

use super::events;
use super::models::{Skill, UpdateSkill};
use super::permissions::{
    SkillsAssignToGroups, SkillsInstall, SkillsManage, SkillsManageSystem, SkillsRead,
};
use super::types::{
    AvailableSkillEntry, AvailableSkillsQuery, AvailableSkillsResponse,
    HideSkillInConversationRequest, SkillBodyResponse, SkillGroupsRequest, SkillListResponse,
};

// =====================================================
// User Handlers
// =====================================================

/// List user-owned + accessible system skills.
#[debug_handler]
pub async fn list_user_skills(
    auth: RequirePermissions<(SkillsRead,)>,
) -> ApiResult<Json<SkillListResponse>> {
    let skills = Repos.skill.list_accessible(auth.user.id).await?;
    Ok((StatusCode::OK, Json(SkillListResponse { skills })))
}

pub fn list_user_skills_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsRead,)>(op)
        .id("Skill.list")
        .tag("Skills")
        .summary("List accessible skills")
        .description("List the caller's own user-scope skills plus any system-scope skills they can access via group assignment.")
        .response::<200, Json<SkillListResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

/// Get one skill by id (caller must have access).
#[debug_handler]
pub async fn get_user_skill(
    auth: RequirePermissions<(SkillsRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Skill>> {
    if !Repos.skill.user_can_read(auth.user.id, id).await? {
        return Err(AppError::not_found("Skill").into());
    }
    let skill = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    Ok((StatusCode::OK, Json(skill)))
}

pub fn get_user_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsRead,)>(op)
        .id("Skill.get")
        .tag("Skills")
        .summary("Get one skill")
        .description("Read a single accessible skill by id.")
        .response::<200, Json<Skill>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

/// Read the SKILL.md markdown body (frontmatter stripped) from the
/// extracted bundle on disk. The frontmatter metadata is already on the
/// `Skill` row; this serves the substantive procedural content for the
/// detail drawer (plan §5). Path is anchored to the row's
/// `extracted_path` + `entry_point` — no user-supplied path component.
#[debug_handler]
pub async fn get_skill_body(
    auth: RequirePermissions<(SkillsRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SkillBodyResponse>> {
    if !Repos.skill.user_can_read(auth.user.id, id).await? {
        return Err(AppError::not_found("Skill").into());
    }
    let skill = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    let path = std::path::Path::new(&skill.extracted_path).join(&skill.entry_point);
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        AppError::internal_error(format!(
            "skill: read {} body at {}: {e}",
            skill.name,
            path.display()
        ))
    })?;
    let (_frontmatter, body) = super::frontmatter::parse_skill_md_frontmatter(&content)
        .unwrap_or_else(|_| (serde_json::Value::Null, content.clone()));
    Ok((StatusCode::OK, Json(SkillBodyResponse { body })))
}

pub fn get_skill_body_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsRead,)>(op)
        .id("Skill.getBody")
        .tag("Skills")
        .summary("Get a skill's SKILL.md body")
        .description("Returns the markdown body (frontmatter stripped) read from the extracted bundle.")
        .response::<200, Json<SkillBodyResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

/// Edit a user-owned skill (limited fields: display_name / description /
/// when_to_use / enabled / tags). Admin-only edits to system-scope
/// items go through the admin endpoint.
#[debug_handler]
pub async fn update_user_skill(
    auth: RequirePermissions<(SkillsManage,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateSkill>,
) -> ApiResult<Json<Skill>> {
    let existing = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    // Only the owner of a user-scope skill may edit via this endpoint.
    if existing.scope != "user" || existing.owner_user_id != Some(auth.user.id) {
        return Err(AppError::forbidden(
            "FORBIDDEN",
            "only the owner may edit a user-scope skill",
        )
        .into());
    }
    // M-2: the edit path must enforce the same description+when_to_use cap as
    // install (frontmatter parse) — otherwise an edit could bloat the
    // always-loaded available-skills system message.
    check_description_cap(&existing, &request)?;
    let updated = Repos.skill.update(id, request).await?;
    // Drop any cached SKILL.md / reference-file content for this id so
    // the next skill_mcp `load_skill` / `read_skill_file` re-reads from
    // disk (mtime invalidation handles content edits; this catches
    // metadata-only changes too).
    crate::modules::skill_mcp::file_cache::invalidate_skill(id);
    events::emit_user_skill(SyncAction::Update, id, auth.user.id, origin.0);
    Ok((StatusCode::OK, Json(updated)))
}

pub fn update_user_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManage,)>(op)
        .id("Skill.update")
        .tag("Skills")
        .summary("Edit a user-owned skill")
        .description("Update the editable metadata of a user-scope skill the caller owns.")
        .response::<200, Json<Skill>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Not the owner"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

/// Delete a user-owned skill. Also rms the extracted bundle dir
/// (best-effort — the DB row is the source of truth).
#[debug_handler]
pub async fn delete_user_skill(
    auth: RequirePermissions<(SkillsManage,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    let existing = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    if existing.scope != "user" || existing.owner_user_id != Some(auth.user.id) {
        return Err(AppError::forbidden(
            "FORBIDDEN",
            "only the owner may delete a user-scope skill",
        )
        .into());
    }
    Repos.skill.delete(id).await?;
    // Best-effort cleanup — the bundle dir is per-install, not per-run.
    let _ = std::fs::remove_dir_all(&existing.extracted_path);
    crate::modules::skill_mcp::file_cache::invalidate_skill(id);
    events::emit_user_skill(SyncAction::Delete, id, auth.user.id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_user_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManage,)>(op)
        .id("Skill.delete")
        .tag("Skills")
        .summary("Delete a user-owned skill")
        .description("Remove a user-scope skill and rm its extracted bundle dir.")
        .response_with::<204, (), _>(|r| r.description("Skill deleted"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Not the owner"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

/// Hide a skill from the model in a specific conversation. The skill
/// stays installed; it just disappears from the available-skills
/// listing for that one conversation.
#[debug_handler]
pub async fn hide_skill_in_conversation(
    auth: RequirePermissions<(SkillsRead,)>,
    Path(id): Path<Uuid>,
    Json(request): Json<HideSkillInConversationRequest>,
) -> ApiResult<StatusCode> {
    if !Repos.skill.user_can_read(auth.user.id, id).await? {
        return Err(AppError::not_found("Skill").into());
    }
    // Verify the caller owns the conversation — never let user A hide a
    // skill in user B's conversation.
    let owner = Repos
        .code_sandbox
        .get_conversation_user_id(request.conversation_id)
        .await
        .ok()
        .flatten();
    if owner != Some(auth.user.id) {
        return Err(AppError::not_found("Conversation").into());
    }
    Repos
        .skill
        .set_hidden_in_conversation(id, request.conversation_id, true)
        .await?;
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn hide_skill_in_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsRead,)>(op)
        .id("Skill.hideInConversation")
        .tag("Skills")
        .summary("Hide a skill from a conversation")
        .description("Insert a per-conversation override so this skill is omitted from the available-skills listing for that conversation.")
        .response_with::<204, (), _>(|r| r.description("Skill hidden"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill or conversation not found"))
}

/// Remove the per-conversation hide override.
#[debug_handler]
pub async fn unhide_skill_in_conversation(
    auth: RequirePermissions<(SkillsRead,)>,
    Path((id, conversation_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let owner = Repos
        .code_sandbox
        .get_conversation_user_id(conversation_id)
        .await
        .ok()
        .flatten();
    if owner != Some(auth.user.id) {
        return Err(AppError::not_found("Conversation").into());
    }
    Repos
        .skill
        .clear_hidden_in_conversation(id, conversation_id)
        .await?;
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn unhide_skill_in_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsRead,)>(op)
        .id("Skill.unhideInConversation")
        .tag("Skills")
        .summary("Remove a per-conversation hide")
        .description("Restore the skill to the available-skills listing in this conversation.")
        .response_with::<204, (), _>(|r| r.description("Hide cleared"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Conversation not found"))
}

/// Effective available-skills listing for a conversation. Same query
/// the chat extension + `skill_mcp::list_tools` use.
#[debug_handler]
pub async fn list_available_skills(
    auth: RequirePermissions<(SkillsRead,)>,
    axum::extract::Query(query): axum::extract::Query<AvailableSkillsQuery>,
) -> ApiResult<Json<AvailableSkillsResponse>> {
    // Verify conversation ownership before exposing per-conversation
    // visibility state (a stale conv id could leak whether OTHER users'
    // conversations exist via the override join).
    let owner = Repos
        .code_sandbox
        .get_conversation_user_id(query.conversation_id)
        .await
        .ok()
        .flatten();
    if owner != Some(auth.user.id) {
        return Err(AppError::not_found("Conversation").into());
    }
    let entries = Repos
        .skill
        .list_available_for_conversation(auth.user.id, query.conversation_id)
        .await?;
    Ok((
        StatusCode::OK,
        Json(AvailableSkillsResponse {
            skills: entries
                .into_iter()
                .map(|e| AvailableSkillEntry {
                    id: e.id,
                    name: e.name,
                    description: e.description,
                    when_to_use: e.when_to_use,
                })
                .collect(),
        }),
    ))
}

pub fn list_available_skills_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsRead,)>(op)
        .id("Skill.listAvailable")
        .tag("Skills")
        .summary("List effective skills for a conversation")
        .description("Return the same listing the chat extension and `skill_mcp::list_tools` produce: user-owned + accessible system, minus per-conversation hides.")
        .response::<200, Json<AvailableSkillsResponse>>()
        .response_with::<400, (), _>(|r| r.description("Missing conversation_id"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Conversation not found"))
}

/// Re-export wrapper: `POST /api/skills/install-from-hub` simply binds
/// the existing hub handler at the canonical user-facing path. Single
/// implementation in `hub::handlers::create_skill_from_hub`.
pub use crate::modules::hub::handlers::{
    create_skill_from_hub as install_from_hub,
    create_skill_from_hub_docs as install_from_hub_docs,
    create_system_skill_from_hub as install_system_from_hub,
    create_system_skill_from_hub_docs as install_system_from_hub_docs,
};

// =====================================================
// Admin Handlers
// =====================================================

/// List all system-scope skills.
#[debug_handler]
pub async fn list_system_skills(
    _auth: RequirePermissions<(SkillsManageSystem,)>,
) -> ApiResult<Json<SkillListResponse>> {
    let skills = Repos.skill.list_system().await?;
    Ok((StatusCode::OK, Json(SkillListResponse { skills })))
}

pub fn list_system_skills_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManageSystem,)>(op)
        .id("SkillSystem.list")
        .tag("Skills - System")
        .summary("List all system-scope skills")
        .response::<200, Json<SkillListResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

#[debug_handler]
pub async fn get_system_skill(
    _auth: RequirePermissions<(SkillsManageSystem,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Skill>> {
    let skill = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    if skill.scope != "system" {
        return Err(AppError::not_found("Skill").into());
    }
    Ok((StatusCode::OK, Json(skill)))
}

pub fn get_system_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManageSystem,)>(op)
        .id("SkillSystem.get")
        .tag("Skills - System")
        .summary("Get one system-scope skill")
        .response::<200, Json<Skill>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

#[debug_handler]
pub async fn update_system_skill(
    _auth: RequirePermissions<(SkillsManageSystem,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateSkill>,
) -> ApiResult<Json<Skill>> {
    let existing = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    if existing.scope != "system" {
        return Err(AppError::not_found("Skill").into());
    }
    // M-2: enforce the description+when_to_use cap on edits (system-scope
    // edits affect EVERY user's always-loaded listing, so this matters most
    // here).
    check_description_cap(&existing, &request)?;
    let updated = Repos.skill.update(id, request).await?;
    crate::modules::skill_mcp::file_cache::invalidate_skill(id);
    events::emit_system_skill(SyncAction::Update, id, origin.0);
    Ok((StatusCode::OK, Json(updated)))
}

/// M-2: enforce the Agent-Skills description+when_to_use char cap on the edit
/// path (install enforces it during frontmatter parse). Partial updates merge
/// the request's new values over the existing row before measuring.
fn check_description_cap(
    existing: &Skill,
    request: &UpdateSkill,
) -> Result<(), (StatusCode, AppError)> {
    let desc = request
        .description
        .as_deref()
        .or(existing.description.as_deref())
        .unwrap_or("");
    let when = request
        .when_to_use
        .as_deref()
        .or(existing.when_to_use.as_deref())
        .unwrap_or("");
    let combined = desc.chars().count() + when.chars().count();
    if combined > crate::modules::skill::frontmatter::MAX_DESCRIPTION_PLUS_WHEN_TO_USE {
        return Err(AppError::unprocessable_entity(
            "SKILL_DESCRIPTION_TOO_LONG",
            format!(
                "description + when_to_use exceeds {} chars (got {})",
                crate::modules::skill::frontmatter::MAX_DESCRIPTION_PLUS_WHEN_TO_USE,
                combined
            ),
        )
        .into());
    }
    Ok(())
}

pub fn update_system_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManageSystem,)>(op)
        .id("SkillSystem.update")
        .tag("Skills - System")
        .summary("Edit a system-scope skill")
        .response::<200, Json<Skill>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

#[debug_handler]
pub async fn delete_system_skill(
    _auth: RequirePermissions<(SkillsManageSystem,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    let existing = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    if existing.scope != "system" {
        return Err(AppError::not_found("Skill").into());
    }
    Repos.skill.delete(id).await?;
    let _ = std::fs::remove_dir_all(&existing.extracted_path);
    crate::modules::skill_mcp::file_cache::invalidate_skill(id);
    events::emit_system_skill(SyncAction::Delete, id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_system_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManageSystem,)>(op)
        .id("SkillSystem.delete")
        .tag("Skills - System")
        .summary("Delete a system-scope skill")
        .response_with::<204, (), _>(|r| r.description("Skill deleted"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

// ---- Group assignment ----

#[debug_handler]
pub async fn get_skill_groups(
    _auth: RequirePermissions<(SkillsAssignToGroups,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<Uuid>>> {
    let groups = Repos.skill.get_skill_groups(id).await?;
    Ok((StatusCode::OK, Json(groups)))
}

pub fn get_skill_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsAssignToGroups,)>(op)
        .id("SkillSystem.getGroups")
        .tag("Skills - System")
        .summary("Get groups assigned to a skill")
        .response::<200, Json<Vec<Uuid>>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

#[debug_handler]
pub async fn set_skill_groups(
    _auth: RequirePermissions<(SkillsAssignToGroups,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<SkillGroupsRequest>,
) -> ApiResult<StatusCode> {
    let existing = Repos
        .skill
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::not_found("Skill"))?;
    if existing.scope != "system" {
        return Err(AppError::bad_request(
            "INVALID_SCOPE",
            "only system-scope skills can be assigned to groups",
        )
        .into());
    }
    // Replace-all: get current, diff, apply.
    let current: std::collections::HashSet<Uuid> = Repos
        .skill
        .get_skill_groups(id)
        .await?
        .into_iter()
        .collect();
    let desired: std::collections::HashSet<Uuid> = request.group_ids.into_iter().collect();
    for gid in current.difference(&desired) {
        Repos.skill.remove_skill_from_group(id, *gid).await?;
    }
    for gid in desired.difference(&current) {
        Repos.skill.assign_skill_to_group(id, *gid).await?;
    }
    events::emit_system_skill(SyncAction::Update, id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn set_skill_groups_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsAssignToGroups,)>(op)
        .id("SkillSystem.setGroups")
        .tag("Skills - System")
        .summary("Replace the set of groups assigned to a skill")
        .response_with::<204, (), _>(|r| r.description("Assignments updated"))
        .response_with::<400, (), _>(|r| r.description("Bad request — non-system scope"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<404, (), _>(|r| r.description("Skill not found"))
}

#[debug_handler]
pub async fn remove_skill_from_group(
    _auth: RequirePermissions<(SkillsAssignToGroups,)>,
    Path((id, group_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    Repos.skill.remove_skill_from_group(id, group_id).await?;
    events::emit_system_skill(SyncAction::Update, id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn remove_skill_from_group_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsAssignToGroups,)>(op)
        .id("SkillSystem.removeFromGroup")
        .tag("Skills - System")
        .summary("Remove a skill from one group")
        .response_with::<204, (), _>(|r| r.description("Removed"))
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}
