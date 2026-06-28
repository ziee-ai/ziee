//! Static tool descriptors emitted by `tools/list`, plus the
//! implementations behind `tools/call`.
//!
//! Both tools read from the bundle's `<extracted_path>` on disk and
//! go through the shared `file_cache` LRU. Per-tool authorization:
//! the JSON-RPC handler in `handlers.rs` validates the JWT (gated on
//! `skills::read`) before calling these; the tools themselves
//! re-verify per-skill access and per-conversation visibility.

#![allow(dead_code)]

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::skill::frontmatter;

use super::file_cache::{self, CacheKey};

/// Static descriptors returned by `tools/list`. Names are intentionally
/// short so the composed `<server_uuid>__<tool_name>` stays well under
/// Anthropic's 128-char cap.
pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "load_skill",
                "description": "Read the SKILL.md body of an installed skill by its reverse-DNS name. Use this after seeing a matching entry in the system-prompt skill listing. The frontmatter is stripped; what you get back is the procedural markdown body.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The skill's reverse-DNS name (e.g. `io.github.ziee/configure-llm-providers`)."
                        }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "read_skill_file",
                "description": "Read a supporting text file from a skill's bundle (e.g. `references/foo.md`, `examples/sample.md`). Path is relative to the skill's bundle root.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The skill's reverse-DNS name."
                        },
                        "path": {
                            "type": "string",
                            "description": "Relative path under the bundle root. Must NOT contain `..`, must NOT be absolute, must NOT be a symlink."
                        }
                    },
                    "required": ["name", "path"]
                }
            }
        ]
    })
}

#[derive(Debug, Deserialize)]
pub struct LoadSkillArgs {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadSkillFileArgs {
    pub name: String,
    pub path: String,
}

/// `load_skill(name)` — returns the SKILL.md body (frontmatter stripped).
pub async fn load_skill(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    let args: LoadSkillArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let name = args.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "name must not be empty"));
    }

    let skill = lookup_accessible(user_id, conversation_id, name).await?;
    let body = read_skill_md_cached(&skill).await?;
    Ok(json!({ "name": skill.name, "content": body }))
}

/// `read_skill_file(name, path)` — returns the content of a supporting
/// file from the bundle. Path-safety re-checked at read time on top of
/// extract-time guarantees.
pub async fn read_skill_file(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    let args: ReadSkillFileArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let name = args.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "name must not be empty"));
    }
    if args.path.trim().is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "path must not be empty"));
    }

    let skill = lookup_accessible(user_id, conversation_id, name).await?;
    let rel = sanitize_relative_path(&args.path)?;

    let bundle_root = PathBuf::from(&skill.extracted_path);
    let full = bundle_root.join(&rel);

    // Re-resolve via canonicalize and verify it stays under the bundle
    // root (defense in depth: extract-time guards already reject `..`,
    // symlinks, and absolute paths; this catches any post-install
    // tampering OR a symlink someone planted out-of-band).
    let canon_root = tokio::fs::canonicalize(&bundle_root)
        .await
        .map_err(|e| AppError::not_found(&format!("skill bundle missing: {e}")))?;
    let canon_full = tokio::fs::canonicalize(&full)
        .await
        .map_err(|_| AppError::not_found("file not found in skill bundle"))?;
    if !canon_full.starts_with(&canon_root) {
        return Err(AppError::forbidden(
            "PATH_ESCAPE",
            "resolved path escapes the bundle root",
        ));
    }

    let meta = tokio::fs::symlink_metadata(&canon_full)
        .await
        .map_err(|e| AppError::not_found(&format!("file metadata unavailable: {e}")))?;
    if meta.file_type().is_symlink() {
        return Err(AppError::forbidden(
            "SYMLINK_REJECTED",
            "symlinks are not readable through skill_mcp",
        ));
    }
    if !meta.file_type().is_file() {
        return Err(AppError::bad_request(
            "NOT_A_FILE",
            "path is not a regular file",
        ));
    }

    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0);
    let key = CacheKey {
        skill_id: skill.id,
        rel_path: rel.to_string_lossy().to_string(),
        mtime_nanos,
        stripped: false, // raw file content (M-5)
    };
    if let Some(cached) = file_cache::get(&key) {
        return Ok(json!({ "name": skill.name, "path": args.path, "content": cached }));
    }

    let bytes = tokio::fs::read(&canon_full)
        .await
        .map_err(|e| AppError::internal_error(&format!("read failed: {e}")))?;
    if is_binary(&bytes) {
        return Err(AppError::bad_request(
            "BINARY_REJECTED",
            "file appears to be binary; skill_mcp returns text only",
        ));
    }
    let content = String::from_utf8(bytes)
        .map_err(|_| AppError::bad_request("INVALID_UTF8", "file is not valid UTF-8"))?;
    file_cache::put(key, content.clone());
    Ok(json!({ "name": skill.name, "path": args.path, "content": content }))
}

// =====================================================
// Internal helpers
// =====================================================

/// Look up the skill by name + enforce access:
/// - User can read it (owner of user-scope OR system-scope reachable).
/// - Not hidden in the requesting conversation (when a conversation
///   context is present).
async fn lookup_accessible(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    name: &str,
) -> Result<crate::modules::skill::models::Skill, AppError> {
    // M5: resolve to the row THIS user can read (preferring their own copy),
    // not the global highest-version row — otherwise another user's same-named
    // install could shadow and make the caller's own skill uncallable.
    // `find_accessible_by_name` already enforces the per-user access predicate
    // (built_in / owned-user / group-granted system) AND `enabled = TRUE`, so a
    // row returned here is necessarily readable by this user — a follow-up
    // `user_can_read` would re-run the identical scope query for no behavioral
    // change. A non-accessible skill simply resolves to None → not_found, which
    // also avoids leaking the existence of another user's install.
    let skill = Repos
        .skill
        .find_accessible_by_name(user_id, name)
        .await?
        .ok_or_else(|| AppError::not_found("skill not installed"))?;

    if let Some(cid) = conversation_id {
        if Repos.skill.is_hidden_in_conversation(skill.id, cid).await? {
            return Err(AppError::forbidden(
                "SKILL_HIDDEN",
                "skill hidden in this conversation",
            ));
        }
    }

    Ok(skill)
}

/// Read SKILL.md + strip frontmatter. Uses the file cache keyed on
/// mtime so an in-place re-extract is reflected on the next read.
async fn read_skill_md_cached(
    skill: &crate::modules::skill::models::Skill,
) -> Result<String, AppError> {
    let path = PathBuf::from(&skill.extracted_path).join(&skill.entry_point);
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| AppError::not_found(&format!("skill bundle missing: {e}")))?;
    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0);
    let key = CacheKey {
        skill_id: skill.id,
        rel_path: skill.entry_point.clone(),
        mtime_nanos,
        stripped: true, // frontmatter-stripped body (M-5)
    };
    if let Some(cached) = file_cache::get(&key) {
        return Ok(cached);
    }
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::internal_error(format!("read SKILL.md failed: {e}")))?;
    let (_frontmatter, body) = frontmatter::parse_skill_md_frontmatter(&raw)?;
    file_cache::put(key, body.clone());
    Ok(body)
}

/// Reject `..`, absolute paths, root-prefixed, weird components. Returns
/// the cleaned `PathBuf` for joining onto the bundle root.
pub fn sanitize_relative_path(input: &str) -> Result<PathBuf, AppError> {
    let p = Path::new(input);
    if p.is_absolute() {
        return Err(AppError::bad_request(
            "PATH_NOT_RELATIVE",
            "path must be relative to the bundle root",
        ));
    }
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(seg) => out.push(seg),
            // Reject anything that isn't a plain name component.
            Component::ParentDir => {
                return Err(AppError::bad_request(
                    "PATH_TRAVERSAL",
                    "path must not contain `..`",
                ));
            }
            Component::CurDir => {
                // Skip silent `./` segments — they're harmless.
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::bad_request(
                    "PATH_NOT_RELATIVE",
                    "path must be relative to the bundle root",
                ));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(AppError::bad_request(
            "PATH_EMPTY",
            "path must reference a file",
        ));
    }
    Ok(out)
}

/// Cheap binary-content heuristic: presence of a NUL byte in the first
/// 8 KiB. Good enough to reject .png / .bin / etc. without dragging in
/// `infer`.
fn is_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(8192)];
    sample.contains(&0u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_dotdot() {
        assert!(sanitize_relative_path("../etc/passwd").is_err());
        assert!(sanitize_relative_path("a/../b").is_err());
    }

    #[test]
    fn sanitize_rejects_absolute() {
        assert!(sanitize_relative_path("/etc/passwd").is_err());
    }

    #[test]
    fn sanitize_rejects_empty() {
        assert!(sanitize_relative_path("").is_err());
        assert!(sanitize_relative_path(".").is_err()); // `./` alone resolves to empty
    }

    #[test]
    fn sanitize_accepts_normal_subpaths() {
        let p = sanitize_relative_path("references/foo.md").expect("ok");
        assert_eq!(p, PathBuf::from("references/foo.md"));

        // Silent `./` prefix stripped.
        let p = sanitize_relative_path("./examples/x.md").expect("ok");
        assert_eq!(p, PathBuf::from("examples/x.md"));
    }

    #[test]
    fn binary_detector_catches_nulls() {
        assert!(is_binary(b"hello\0world"));
        assert!(!is_binary(b"hello world"));
        assert!(!is_binary(b"## SKILL\n\nProcedural body."));
    }

    #[test]
    fn tool_list_advertises_two_tools_with_required_args() {
        let v = tool_list();
        let tools = v["tools"].as_array().expect("array");
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["load_skill", "read_skill_file"]);
        // Required args propagate per Anthropic's input-schema shape.
        assert_eq!(tools[0]["inputSchema"]["required"], json!(["name"]));
        assert_eq!(tools[1]["inputSchema"]["required"], json!(["name", "path"]));
    }
}
