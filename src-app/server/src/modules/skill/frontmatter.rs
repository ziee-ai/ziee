//! SKILL.md frontmatter parser (Agent Skills spec).
//!
//! The bundle's `SKILL.md` ships with YAML frontmatter between `---`
//! markers at the top, followed by a free-form markdown body. The
//! frontmatter carries the model-visible identity + listing line
//! (`name`, `description`, `when_to_use`) plus optional capability /
//! invocation fields (`allowed-tools`, `disable-model-invocation`,
//! `paths`, `metadata`).
//!
//! Per the Agent Skills spec, the combined length of
//! `description + when_to_use` is capped at 1536 chars (matches the
//! spec's truncation rule). The full frontmatter is persisted opaquely
//! in `skills.frontmatter_json` so unknown fields round-trip on
//! re-export.

use crate::common::AppError;

/// Max combined chars across `description` + `when_to_use`. Per Agent
/// Skills spec — the listing line each gets in the model's "available
/// skills" prompt is bounded.
pub const MAX_DESCRIPTION_PLUS_WHEN_TO_USE: usize = 1536;

/// Parse SKILL.md content into `(frontmatter_json, body)`.
///
/// Requirements:
/// - Content MUST start with `---\n` (an opening fence).
/// - A second `---` line closes the frontmatter block.
/// - The block between fences MUST be valid YAML deserializing to a
///   JSON object (`serde_json::Value::Object`).
/// - The combined `description + when_to_use` field lengths MUST NOT
///   exceed 1536 chars.
///
/// On the happy path, returns `(frontmatter_object, body_string)`.
pub fn parse_skill_md_frontmatter(
    content: &str,
) -> Result<(serde_json::Value, String), AppError> {
    // Strip an optional UTF-8 BOM so an editor-saved SKILL.md doesn't
    // fail the opening-fence check.
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    // Tolerate LF / CRLF / CR line endings.
    let (head, rest) = content
        .split_once('\n')
        .ok_or_else(|| {
            AppError::bad_request(
                "SKILL_FRONTMATTER_MISSING",
                "SKILL.md must start with a YAML frontmatter block fenced by '---'",
            )
        })?;
    if head.trim_end_matches('\r') != "---" {
        return Err(AppError::bad_request(
            "SKILL_FRONTMATTER_MISSING",
            "SKILL.md must start with a YAML frontmatter block fenced by '---'",
        ));
    }

    // Find the closing fence — a line that, ignoring trailing CR, is
    // exactly `---`. Track:
    //   - `yaml_end`: byte offset (within `rest`) where the YAML body
    //     ENDS (exclusive — NOT including the fence line's leading
    //     newline).
    //   - `body_start`: byte offset where the body BEGINS (just past
    //     the closing fence's terminating newline).
    let mut yaml_end: Option<usize> = None;
    let mut body_start: Option<usize> = None;
    let mut cursor = 0usize;
    for line in rest.split_inclusive('\n') {
        let stripped = line.trim_end_matches('\n').trim_end_matches('\r');
        if stripped == "---" {
            yaml_end = Some(cursor);
            body_start = Some(cursor + line.len());
            break;
        }
        cursor += line.len();
    }
    // If the file ends without a trailing newline, the last "line"
    // produced by `split_inclusive` won't end with `\n`. Handle the
    // trailing closing-fence case.
    let (yaml_end, body_start) = match (yaml_end, body_start) {
        (Some(y), Some(b)) => (y, b),
        _ => {
            let last = &rest[cursor..];
            if last.trim_end_matches('\r') == "---" {
                (cursor, rest.len())
            } else {
                return Err(AppError::bad_request(
                    "SKILL_FRONTMATTER_UNCLOSED",
                    "SKILL.md frontmatter missing the closing '---' fence",
                ));
            }
        }
    };

    let yaml_block = &rest[..yaml_end];
    let body = rest[body_start..].to_string();

    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_block).map_err(|e| {
        AppError::bad_request(
            "SKILL_FRONTMATTER_INVALID_YAML",
            format!("SKILL.md frontmatter is not valid YAML: {e}"),
        )
    })?;
    let frontmatter_json =
        serde_json::to_value(&parsed).map_err(|e| {
            AppError::internal_error(format!(
                "skill: convert YAML to JSON: {e}"
            ))
        })?;
    if !frontmatter_json.is_object() {
        return Err(AppError::bad_request(
            "SKILL_FRONTMATTER_NOT_OBJECT",
            "SKILL.md frontmatter must be a YAML mapping (object)",
        ));
    }

    // Cap description + when_to_use per the spec. The cap is in CHARACTERS
    // (the spec's truncation rule + the publisher's validate.py count chars),
    // so count chars, not bytes — otherwise a multibyte (CJK / emoji)
    // description would be capped early.
    let desc_len = frontmatter_json
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().count())
        .unwrap_or(0);
    let when_len = frontmatter_json
        .get("when_to_use")
        .or_else(|| frontmatter_json.get("when-to-use"))
        .and_then(|v| v.as_str())
        .map(|s| s.chars().count())
        .unwrap_or(0);
    if desc_len + when_len > MAX_DESCRIPTION_PLUS_WHEN_TO_USE {
        return Err(AppError::bad_request(
            "SKILL_FRONTMATTER_TOO_LONG",
            format!(
                "SKILL.md frontmatter description + when_to_use exceeds {} chars (got {})",
                MAX_DESCRIPTION_PLUS_WHEN_TO_USE,
                desc_len + when_len
            ),
        ));
    }

    Ok((frontmatter_json, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_frontmatter() {
        let md = "---\nname: configure-llm-providers\ndescription: How to set up providers.\n---\n# Body\ncontent here";
        let (fm, body) = parse_skill_md_frontmatter(md).expect("parse");
        assert_eq!(
            fm.get("name").and_then(|v| v.as_str()),
            Some("configure-llm-providers")
        );
        assert!(body.starts_with("# Body"));
    }

    #[test]
    fn rejects_missing_opening_fence() {
        let md = "name: foo\ndescription: bar\n";
        let err = parse_skill_md_frontmatter(md).unwrap_err();
        assert!(err.to_string().contains("frontmatter"));
    }

    #[test]
    fn rejects_unclosed_frontmatter() {
        let md = "---\nname: foo\ndescription: bar\n";
        let err = parse_skill_md_frontmatter(md).unwrap_err();
        assert!(err.to_string().contains("closing"));
    }

    #[test]
    fn rejects_oversize_description_plus_when_to_use() {
        let desc = "a".repeat(1500);
        let when = "b".repeat(100);
        let md = format!(
            "---\ndescription: \"{desc}\"\nwhen_to_use: \"{when}\"\n---\nbody"
        );
        let err = parse_skill_md_frontmatter(&md).unwrap_err();
        assert!(err.to_string().contains("exceeds"));
    }

    #[test]
    fn preserves_unknown_fields_opaquely() {
        let md = "---\nname: foo\ndescription: bar\nallowed-tools: Read Bash(npm:*)\nmetadata:\n  author: phibya\n---\nbody";
        let (fm, _) = parse_skill_md_frontmatter(md).expect("parse");
        assert_eq!(
            fm.get("allowed-tools").and_then(|v| v.as_str()),
            Some("Read Bash(npm:*)")
        );
        assert_eq!(
            fm.get("metadata")
                .and_then(|m| m.get("author"))
                .and_then(|v| v.as_str()),
            Some("phibya")
        );
    }

    #[test]
    fn accepts_crlf_line_endings() {
        let md = "---\r\nname: foo\r\ndescription: bar\r\n---\r\nbody\r\n";
        let (fm, body) = parse_skill_md_frontmatter(md).expect("parse");
        assert_eq!(fm.get("name").and_then(|v| v.as_str()), Some("foo"));
        assert!(body.starts_with("body"));
    }
}
