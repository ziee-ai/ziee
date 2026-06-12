//! Pure text-edit primitives for the read-write file tools. Kept free of I/O so
//! the matching/validation rules are unit-testable in isolation (Tier 1).

use crate::common::AppError;

/// Outcome of an edit attempt: either new content, or a no-op (content was
/// already what the edit would produce — the caller skips appending a version).
#[derive(Debug)]
pub enum EditOutcome {
    Changed(String),
    Unchanged,
}

/// Replace the single, unique occurrence of `old` with `new`.
///
/// - 0 matches  → `NO_MATCH` (400)
/// - >1 matches → `MULTIPLE_MATCHES` (400) — the model must disambiguate
/// - exactly 1  → replaced (or `Unchanged` if `old == new`)
/// - empty `old` → invalid
pub fn str_replace(content: &str, old: &str, new: &str) -> Result<EditOutcome, AppError> {
    if old.is_empty() {
        return Err(AppError::bad_request(
            "INVALID_ARGS",
            "old_str must not be empty",
        ));
    }
    if old == new {
        return Ok(EditOutcome::Unchanged);
    }
    let count = content.matches(old).count();
    match count {
        0 => Err(AppError::bad_request(
            "NO_MATCH",
            "old_str was not found in the file",
        )),
        1 => Ok(EditOutcome::Changed(content.replacen(old, new, 1))),
        n => Err(AppError::bad_request(
            "MULTIPLE_MATCHES",
            format!("old_str matches {n} times — include more surrounding text to make it unique"),
        )),
    }
}

/// Replace the 1-indexed inclusive line range `[start_line, end_line]` with
/// `new_content`. `start_line == line_count + 1` appends. Mirrors the sandbox
/// `edit_file` contract (1-indexed, append-by-one-past-end).
pub fn apply_line_range(
    content: &str,
    start_line: usize,
    end_line: usize,
    new_content: &str,
) -> Result<EditOutcome, AppError> {
    if start_line == 0 {
        return Err(AppError::bad_request(
            "INVALID_ARGS",
            "start_line is 1-indexed (must be >= 1)",
        ));
    }
    // Preserve whether the file ended with a trailing newline.
    let had_trailing_nl = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let len = lines.len();

    if start_line > len + 1 {
        return Err(AppError::bad_request(
            "INVALID_ARGS",
            format!(
                "start_line {start_line} is past end+1 ({}); use start_line = {} to append",
                len + 1,
                len + 1
            ),
        ));
    }
    if end_line < start_line.saturating_sub(1) {
        return Err(AppError::bad_request(
            "INVALID_ARGS",
            "end_line must be >= start_line - 1",
        ));
    }

    let start_idx = start_line - 1;
    let end_idx = end_line.min(len); // clamp; inclusive end → drain [start_idx, end_idx)
    let drain_end = end_idx.max(start_idx);
    let replacement: Vec<String> = new_content.lines().map(|s| s.to_string()).collect();
    lines.splice(start_idx..drain_end, replacement);

    let mut out = lines.join("\n");
    if had_trailing_nl && !out.is_empty() {
        out.push('\n');
    }
    if out == content {
        Ok(EditOutcome::Unchanged)
    } else {
        Ok(EditOutcome::Changed(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(o: EditOutcome) -> String {
        match o {
            EditOutcome::Changed(s) => s,
            EditOutcome::Unchanged => panic!("expected Changed"),
        }
    }
    fn is_unchanged(o: EditOutcome) -> bool {
        matches!(o, EditOutcome::Unchanged)
    }

    #[test]
    fn str_replace_unique_match() {
        assert_eq!(changed(str_replace("hello world", "world", "rust").unwrap()), "hello rust");
    }

    #[test]
    fn str_replace_no_match_errors() {
        let e = str_replace("abc", "xyz", "q").unwrap_err();
        assert_eq!(e.error_code(), "NO_MATCH");
    }

    #[test]
    fn str_replace_multiple_matches_errors() {
        let e = str_replace("a a a", "a", "b").unwrap_err();
        assert_eq!(e.error_code(), "MULTIPLE_MATCHES");
    }

    #[test]
    fn str_replace_empty_old_invalid() {
        assert_eq!(str_replace("x", "", "y").unwrap_err().error_code(), "INVALID_ARGS");
    }

    #[test]
    fn str_replace_noop_when_old_equals_new() {
        assert!(is_unchanged(str_replace("a a", "a", "a").unwrap()));
    }

    #[test]
    fn str_replace_multibyte_safe() {
        // 'é' is multi-byte; replacen must not split a char boundary.
        assert_eq!(changed(str_replace("café X", "X", "déjà").unwrap()), "café déjà");
    }

    #[test]
    fn str_replace_multiline_old() {
        assert_eq!(
            changed(str_replace("a\nb\nc\n", "a\nb", "z").unwrap()),
            "z\nc\n"
        );
    }

    #[test]
    fn str_replace_deletion() {
        assert_eq!(changed(str_replace("foobar", "foo", "").unwrap()), "bar");
    }

    #[test]
    fn line_range_replace_middle() {
        assert_eq!(changed(apply_line_range("a\nb\nc\n", 2, 2, "B").unwrap()), "a\nB\nc\n");
    }

    #[test]
    fn line_range_append_one_past_end() {
        assert_eq!(changed(apply_line_range("a\nb\n", 3, 3, "c").unwrap()), "a\nb\nc\n");
    }

    #[test]
    fn line_range_start_zero_invalid() {
        assert_eq!(apply_line_range("a\n", 0, 1, "x").unwrap_err().error_code(), "INVALID_ARGS");
    }

    #[test]
    fn line_range_past_end_invalid() {
        assert_eq!(apply_line_range("a\nb\n", 5, 5, "x").unwrap_err().error_code(), "INVALID_ARGS");
    }

    #[test]
    fn line_range_preserves_no_trailing_newline() {
        assert_eq!(changed(apply_line_range("a\nb", 1, 1, "Z").unwrap()), "Z\nb");
    }

    #[test]
    fn line_range_noop_when_identical() {
        assert!(is_unchanged(apply_line_range("a\nb\n", 1, 1, "a").unwrap()));
    }
}
