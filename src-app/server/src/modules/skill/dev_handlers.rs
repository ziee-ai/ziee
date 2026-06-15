//! Skill dev / local import + validate handlers (Phase B6):
//!   POST /api/skills/import    (multipart tarball, is_dev install)
//!   POST /api/skills/validate  (no-side-effect frontmatter check)
//!
//! Mirrors `workflow::handlers::dev` but skills don't execute, so there's
//! no cost estimation / dry-run / test surface — validate is just
//! frontmatter parsing + the 1536-char cap (plan §2: "Skills are EXEMPT
//! ... Skill validation stays at frontmatter parsing").

#![allow(dead_code)]

use aide::transform::TransformOperation;
use axum::extract::{Multipart, Query};
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::with_permission;
use crate::modules::sync::{SyncAction, SyncOrigin};

use super::frontmatter::parse_skill_md_frontmatter;
use super::models::{CreateSkill, Skill};
use super::permissions::SkillsInstall;

// ============================================================
// POST /api/skills/validate
// ============================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ValidateSkillRequest {
    /// The SKILL.md text (frontmatter + body).
    pub skill_md: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidateErrorEntry {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidateSkillResponse {
    pub valid: bool,
    pub errors: Vec<ValidateErrorEntry>,
    pub warnings: Vec<ValidateErrorEntry>,
}

pub async fn validate_skill(
    _auth: RequirePermissions<(SkillsInstall,)>,
    Json(req): Json<ValidateSkillRequest>,
) -> ApiResult<Json<ValidateSkillResponse>> {
    match parse_skill_md_frontmatter(&req.skill_md) {
        Ok((frontmatter, _body)) => {
            // Require `description` — the model needs it to know when to
            // invoke (plan §2).
            let mut errors = Vec::new();
            if frontmatter
                .get("description")
                .and_then(|v| v.as_str())
                .map(str::is_empty)
                .unwrap_or(true)
            {
                errors.push(ValidateErrorEntry {
                    code: "SKILL_FRONTMATTER_NO_DESCRIPTION".into(),
                    path: Some("description".into()),
                    message: "SKILL.md frontmatter must include a non-empty `description`".into(),
                });
            }
            Ok((
                StatusCode::OK,
                Json(ValidateSkillResponse {
                    valid: errors.is_empty(),
                    errors,
                    warnings: vec![],
                }),
            ))
        }
        Err(e) => Ok((
            StatusCode::OK,
            Json(ValidateSkillResponse {
                valid: false,
                errors: vec![ValidateErrorEntry {
                    code: "SKILL_FRONTMATTER_INVALID".into(),
                    path: None,
                    message: e.to_string(),
                }],
                warnings: vec![],
            }),
        )),
    }
}

pub fn validate_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsInstall,)>(op)
        .id("Skill.validate")
        .tag("Skills")
        .summary("Validate a SKILL.md without installing")
        .description("Parses SKILL.md frontmatter (require description, 1536-char cap). No DB row created.")
        .response::<200, Json<ValidateSkillResponse>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

// ============================================================
// POST /api/skills/import  (multipart, dev install)
// ============================================================

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ImportQuery {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

pub async fn import_skill(
    auth: RequirePermissions<(SkillsInstall,)>,
    Query(q): Query<ImportQuery>,
    origin: SyncOrigin,
    multipart: Multipart,
) -> ApiResult<Json<Skill>> {
    let bytes = read_bundle_field(multipart).await?;

    // Scope (`system` requires skills::manage_system) — reuse the
    // workflow helper (it's module-agnostic).
    let scope = crate::modules::workflow::handlers::dev::resolve_import_scope(
        &auth.user,
        &auth.groups,
        q.scope.as_deref(),
        "skills",
    )?;

    let slug = q
        .name
        .clone()
        .map(|s| sanitize_slug(&s))
        .unwrap_or_else(|| "imported-skill".to_string());
    let name = format!("local.dev/{slug}");
    let version = "0.0.0-dev".to_string();

    let app_data_dir = crate::core::get_app_data_dir();
    let target_dir = app_data_dir.join("skills").join(&name).join(&version);

    // Bomb-guarded extract. Skill bundles drop execute bits (Phase 1).
    let extraction = crate::modules::hub::bundle::extract_tarball_bytes(
        &bytes,
        &target_dir,
        crate::modules::hub::bundle::BundleKind::Skill,
    )
    .await?;

    let entry_point = "SKILL.md".to_string();
    let skill_md_path = extraction.extracted_path.join(&entry_point);
    let content = match tokio::fs::read_to_string(&skill_md_path).await {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&extraction.extracted_path);
            return Err(AppError::bad_request(
                "SKILL_NO_ENTRY_POINT",
                format!("bundle is missing SKILL.md: {e}"),
            )
            .into());
        }
    };
    let (frontmatter_json, _body) = match parse_skill_md_frontmatter(&content) {
        Ok(parsed) => parsed,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&extraction.extracted_path);
            return Err(e.into());
        }
    };

    let display_name = frontmatter_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| Some(slug.clone()));
    let description = frontmatter_json
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let when_to_use = frontmatter_json
        .get("when_to_use")
        .or_else(|| frontmatter_json.get("when-to-use"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // Re-import overwrites: delete any prior row (the dir was already
    // overwritten by extract_tarball_bytes).
    if let Some(prior) = Repos
        .skill
        .find_by_name_version(&name, Some(&version))
        .await?
    {
        Repos.skill.delete(prior.id).await?;
    }

    let owner_user_id = if scope == "system" {
        None
    } else {
        Some(auth.user.id)
    };

    let create = CreateSkill {
        name: name.clone(),
        version: Some(version),
        display_name,
        description,
        when_to_use,
        extracted_path: extraction.extracted_path.display().to_string(),
        bundle_sha256: extraction.sha256_hex.clone(),
        bundle_size_bytes: extraction.total_bytes as i64,
        file_count: extraction.file_count as i32,
        entry_point,
        frontmatter_json,
        tags: serde_json::Value::Array(vec![]),
        scope: scope.clone(),
        owner_user_id,
        created_by: Some(auth.user.id),
        enabled: true,
        is_dev: true,
    };

    let skill = match Repos.skill.insert(create).await {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&extraction.extracted_path);
            return Err(e.into());
        }
    };

    if scope == "system" {
        super::events::emit_system_skill(SyncAction::Create, skill.id, origin.0);
    } else {
        super::events::emit_user_skill(SyncAction::Create, skill.id, auth.user.id, origin.0);
    }

    Ok((StatusCode::CREATED, Json(skill)))
}

pub fn import_skill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsInstall,)>(op)
        .id("Skill.import")
        .tag("Skills")
        .summary("Dev-import a skill bundle (multipart tarball)")
        .description("Extract a tar.gz of the skill source dir, parse SKILL.md frontmatter, install as local.dev/<slug> with is_dev=true. Re-import overwrites.")
        .response::<201, Json<Skill>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<403, (), _>(|r| r.description("Forbidden (system scope without admin)"))
}

// ============================================================
// Helpers
// ============================================================

async fn read_bundle_field(mut multipart: Multipart) -> Result<Vec<u8>, AppError> {
    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "bundle" {
            let data = field.bytes().await.map_err(|e| {
                AppError::bad_request("IMPORT_READ_FAILED", format!("read bundle field: {e}"))
            })?;
            return Ok(data.to_vec());
        }
    }
    Err(AppError::bad_request(
        "IMPORT_NO_BUNDLE",
        "multipart upload missing a `bundle` form field carrying the tar.gz",
    ))
}

fn sanitize_slug(raw: &str) -> String {
    let s: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "imported".to_string()
    } else {
        s
    }
}
