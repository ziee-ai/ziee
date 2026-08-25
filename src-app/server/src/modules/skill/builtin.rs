//! Built-in capability skills — ziee's own self-documentation, embedded
//! in the binary and synced into the `skills` table as `scope='built_in'`
//! rows on boot.
//!
//! These are NOT hub-distributed (they're intrinsic to the app + version-
//! locked to the binary), NOT uninstallable, and always available to every
//! user. They still load on-demand via `skill_mcp` (progressive disclosure)
//! — only the frontmatter listing sits in the prompt; bodies load when the
//! model calls `load_skill`.
//!
//! Source lives at `resources/builtin-skills/<leaf>/SKILL.md` and is baked
//! in via `include_dir!`. On boot the sync parses each frontmatter, writes
//! the files to `<data_dir>/skills/builtin/<leaf>/`, and UPSERTs a
//! `scope='built_in'` row keyed on `name` (so a binary upgrade replaces the
//! row in place — version-locked).

use include_dir::{Dir, include_dir};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::common::AppError;
use crate::modules::skill::frontmatter::parse_skill_md_frontmatter;
use crate::modules::skill::models::CreateSkill;

/// Embedded built-in skill source (SKILL.md + optional references/).
static BUILTIN_SKILLS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/resources/builtin-skills");

/// Reverse-DNS namespace for built-in capability skills. Distinct from any
/// likely user/system skill name; the per-scope unique index keys on `name`.
const BUILTIN_NAMESPACE: &str = "io.github.ziee";

/// Sync every embedded built-in skill into the `skills` table as a
/// `scope='built_in'` row (idempotent upsert keyed on `name`). Extracts the
/// SKILL.md (+ references) to `<data_dir>/skills/builtin/<leaf>/` so
/// `skill_mcp` can read the body on demand.
pub async fn sync_builtin_skills(pool: &PgPool) -> Result<usize, AppError> {
    let base = crate::core::get_app_data_dir()
        .join("skills")
        .join("builtin");

    let mut synced = 0usize;
    for entry in BUILTIN_SKILLS.dirs() {
        let leaf = entry
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        if leaf.is_empty() {
            continue;
        }
        match sync_one(pool, &base, &leaf, entry).await {
            Ok(()) => synced += 1,
            Err(e) => {
                // One bad built-in must not abort the rest (or boot).
                tracing::warn!(skill = %leaf, error = %e, "builtin skill sync failed");
            }
        }
    }
    Ok(synced)
}

async fn sync_one(
    pool: &PgPool,
    base: &std::path::Path,
    leaf: &str,
    dir: &Dir<'_>,
) -> Result<(), AppError> {
    // SKILL.md is the entry point.
    let skill_md = dir
        .get_file(format!("{leaf}/SKILL.md"))
        .ok_or_else(|| AppError::internal_error(format!("builtin {leaf}: no SKILL.md")))?;
    let raw = skill_md
        .contents_utf8()
        .ok_or_else(|| AppError::internal_error(format!("builtin {leaf}: SKILL.md not UTF-8")))?;

    let (frontmatter_json, _body) = parse_skill_md_frontmatter(raw)?;
    let display_name = frontmatter_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let description = frontmatter_json
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let when_to_use = frontmatter_json
        .get("when_to_use")
        .or_else(|| frontmatter_json.get("when-to-use"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // Extract files to disk (overwrite — version-locked to the binary).
    let dest = base.join(leaf);
    tokio::fs::create_dir_all(&dest)
        .await
        .map_err(|e| AppError::internal_error(format!("builtin {leaf}: mkdir: {e}")))?;
    let mut file_count = 0i32;
    let mut total_bytes = 0i64;
    for f in dir.files() {
        let rel = f
            .path()
            .strip_prefix(leaf)
            .unwrap_or_else(|_| f.path());
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::internal_error(format!("builtin {leaf}: mkdir: {e}")))?;
        }
        tokio::fs::write(&target, f.contents())
            .await
            .map_err(|e| AppError::internal_error(format!("builtin {leaf}: write: {e}")))?;
        file_count += 1;
        total_bytes += f.contents().len() as i64;
    }
    // Also handle nested files under references/ that `dir.files()` may not
    // surface at the top level (include_dir flattens via recursion below).
    for sub in dir.dirs() {
        for f in sub.files() {
            let rel = f.path().strip_prefix(leaf).unwrap_or_else(|_| f.path());
            let target = dest.join(rel);
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    AppError::internal_error(format!("builtin {leaf}: mkdir: {e}"))
                })?;
            }
            tokio::fs::write(&target, f.contents())
                .await
                .map_err(|e| AppError::internal_error(format!("builtin {leaf}: write: {e}")))?;
            file_count += 1;
            total_bytes += f.contents().len() as i64;
        }
    }

    let sha = hex::encode(Sha256::digest(raw.as_bytes()));
    let name = format!("{BUILTIN_NAMESPACE}/{leaf}");

    let create = CreateSkill {
        name,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        display_name,
        description,
        when_to_use,
        extracted_path: dest.to_string_lossy().to_string(),
        bundle_sha256: sha,
        bundle_size_bytes: total_bytes,
        file_count,
        entry_point: "SKILL.md".to_string(),
        frontmatter_json,
        tags: serde_json::json!([]),
        scope: "built_in".to_string(),
        owner_user_id: None,
        created_by: None,
        enabled: true,
        is_dev: false,
    };
    super::repository::upsert_builtin(pool, create).await?;
    Ok(())
}
