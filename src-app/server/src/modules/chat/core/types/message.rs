// Chat type infrastructure

// Message API request/response types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{Branch, Message, MessageContent};

/// Message with its content blocks
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MessageWithContent {
    #[serde(flatten)]
    pub message: Message,
    pub contents: Vec<MessageContent>,
}

// =====================================================
// Paginated message history (lazy-load / keyset window)
// =====================================================

/// Default page size for a message-history window.
pub const DEFAULT_MESSAGE_PAGE_SIZE: i64 = 30;
/// Hard cap on a single message-history window.
pub const MAX_MESSAGE_PAGE_SIZE: i64 = 100;

/// Query params for `GET /conversations/{id}/messages`.
///
/// The cursor is a **message_id** (resolved server-side to its
/// `branch_messages.created_at` on the conversation's ACTIVE branch). At most
/// ONE of `before` / `after` / `around` may be set:
/// - none    → the newest `limit` messages (tail),
/// - `before`→ the `limit` messages immediately OLDER than the cursor,
/// - `after` → the `limit` messages immediately NEWER than the cursor,
/// - `around`→ a window CENTERED on the cursor (≈limit/2 older + it + ≈limit/2 newer).
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct MessageHistoryQuery {
    #[serde(default)]
    pub before: Option<Uuid>,
    #[serde(default)]
    pub after: Option<Uuid>,
    #[serde(default)]
    pub around: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// The resolved window mode (exactly one variant) for the repository layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageWindowMode {
    Tail,
    Before(Uuid),
    After(Uuid),
    Around(Uuid),
}

impl MessageHistoryQuery {
    /// Clamp the requested limit into `[1, MAX]`, defaulting to `DEFAULT`.
    pub fn clamped_limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_MESSAGE_PAGE_SIZE)
            .clamp(1, MAX_MESSAGE_PAGE_SIZE)
    }

    /// Resolve to a single [`MessageWindowMode`], rejecting a request that sets
    /// more than one cursor (→ 400).
    pub fn mode(&self) -> Result<MessageWindowMode, AppError> {
        let count = [
            self.before.is_some(),
            self.after.is_some(),
            self.around.is_some(),
        ]
        .into_iter()
        .filter(|b| *b)
        .count();
        if count > 1 {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "At most one of `before`, `after`, or `around` may be set",
            ));
        }
        Ok(if let Some(id) = self.before {
            MessageWindowMode::Before(id)
        } else if let Some(id) = self.after {
            MessageWindowMode::After(id)
        } else if let Some(id) = self.around {
            MessageWindowMode::Around(id)
        } else {
            MessageWindowMode::Tail
        })
    }
}

/// A page of messages from the active branch, chronological ascending. Cursors
/// are the window endpoints — the client sends `messages[0].id` as the next
/// `before` (scroll-up) and `messages[last].id` as the next `after` (scroll-down).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PaginatedMessages {
    pub messages: Vec<MessageWithContent>,
    /// Older messages exist beyond `messages[0]`.
    pub has_more_before: bool,
    /// Newer messages exist beyond `messages[last]`.
    pub has_more_after: bool,
}

// =====================================================
// In-conversation message search (find over unloaded messages)
// =====================================================

/// Default page size for in-conversation search results.
pub const DEFAULT_SEARCH_PER_PAGE: i64 = 25;
/// Hard cap on a single search-results page.
pub const MAX_SEARCH_PER_PAGE: i64 = 100;
/// Max characters retained in a match snippet.
pub const SEARCH_SNIPPET_MAX_CHARS: usize = 160;

/// Query params for `GET /conversations/{id}/messages/search`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct MessageSearchQuery {
    /// Case-insensitive substring term. Blank/whitespace → empty result.
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
}

impl MessageSearchQuery {
    /// The trimmed term, or `None` when blank (caller returns an empty result
    /// without touching the DB).
    pub fn trimmed_term(&self) -> Option<&str> {
        let t = self.q.trim();
        if t.is_empty() { None } else { Some(t) }
    }

    /// 1-based page floored at 1.
    pub fn clamped_page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    /// Page size clamped into `[1, MAX]`, defaulting to `DEFAULT`.
    pub fn clamped_per_page(&self) -> i64 {
        self.per_page
            .unwrap_or(DEFAULT_SEARCH_PER_PAGE)
            .clamp(1, MAX_SEARCH_PER_PAGE)
    }
}

/// A single in-conversation search hit.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MessageSearchMatch {
    pub message_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
    /// A bounded, ellipsized excerpt of the matching text around the hit.
    pub snippet: String,
    /// 1-based GLOBAL position within the full match set (stable across pages).
    pub ordinal: i64,
}

/// A page of in-conversation search results.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MessageSearchResults {
    pub matches: Vec<MessageSearchMatch>,
    /// Full match count across all pages (drives the "X of Y" readout).
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Build a bounded snippet: the first window of `text` containing `term`
/// (case-insensitive), ellipsized to [`SEARCH_SNIPPET_MAX_CHARS`]. Pure — unit
/// tested (TEST-14).
pub fn build_snippet(text: &str, term: &str) -> String {
    let collapsed: String = {
        // Collapse runs of whitespace so a snippet doesn't carry raw newlines.
        let mut out = String::with_capacity(text.len());
        let mut prev_ws = false;
        for ch in text.chars() {
            if ch.is_whitespace() {
                if !prev_ws {
                    out.push(' ');
                }
                prev_ws = true;
            } else {
                out.push(ch);
                prev_ws = false;
            }
        }
        out.trim().to_string()
    };

    let chars: Vec<char> = collapsed.chars().collect();
    if chars.len() <= SEARCH_SNIPPET_MAX_CHARS {
        return collapsed;
    }

    // Locate the term (case-insensitive) to center the window on it.
    let hay = collapsed.to_lowercase();
    let needle = term.to_lowercase();
    let match_char_idx = hay.find(&needle).map(|byte_idx| {
        // Convert the byte offset into a char offset over `collapsed`.
        collapsed[..byte_idx].chars().count()
    });

    let (start, prefix_ellipsis) = match match_char_idx {
        Some(mi) => {
            let ctx = SEARCH_SNIPPET_MAX_CHARS / 3;
            let s = mi.saturating_sub(ctx);
            (s, s > 0)
        }
        None => (0, false),
    };

    let end = (start + SEARCH_SNIPPET_MAX_CHARS).min(chars.len());
    let suffix_ellipsis = end < chars.len();
    let core: String = chars[start..end].iter().collect();
    format!(
        "{}{}{}",
        if prefix_ellipsis { "…" } else { "" },
        core.trim(),
        if suffix_ellipsis { "…" } else { "" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-1: limit clamp + cursor mutual-exclusion.
    #[test]
    fn history_query_limit_clamps_and_defaults() {
        let q = MessageHistoryQuery::default();
        assert_eq!(q.clamped_limit(), DEFAULT_MESSAGE_PAGE_SIZE);

        let q = MessageHistoryQuery { limit: Some(0), ..Default::default() };
        assert_eq!(q.clamped_limit(), 1);

        let q = MessageHistoryQuery { limit: Some(9999), ..Default::default() };
        assert_eq!(q.clamped_limit(), MAX_MESSAGE_PAGE_SIZE);

        let q = MessageHistoryQuery { limit: Some(-5), ..Default::default() };
        assert_eq!(q.clamped_limit(), 1);
    }

    #[test]
    fn history_query_mode_resolves_single_cursor() {
        let id = Uuid::new_v4();
        assert_eq!(MessageHistoryQuery::default().mode().unwrap(), MessageWindowMode::Tail);
        assert_eq!(
            MessageHistoryQuery { before: Some(id), ..Default::default() }.mode().unwrap(),
            MessageWindowMode::Before(id)
        );
        assert_eq!(
            MessageHistoryQuery { after: Some(id), ..Default::default() }.mode().unwrap(),
            MessageWindowMode::After(id)
        );
        assert_eq!(
            MessageHistoryQuery { around: Some(id), ..Default::default() }.mode().unwrap(),
            MessageWindowMode::Around(id)
        );
    }

    #[test]
    fn history_query_mode_rejects_multiple_cursors() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let err = MessageHistoryQuery { before: Some(a), after: Some(b), ..Default::default() }
            .mode()
            .unwrap_err();
        // A bad_request AppError — asserting it errored is sufficient here.
        let _ = err;
        assert!(
            MessageHistoryQuery { before: Some(a), around: Some(b), ..Default::default() }
                .mode()
                .is_err()
        );
        assert!(
            MessageHistoryQuery { after: Some(a), around: Some(b), ..Default::default() }
                .mode()
                .is_err()
        );
    }

    // TEST-14: search query clamps + blank handling + snippet bounds.
    #[test]
    fn search_query_blank_term_is_none() {
        assert_eq!(MessageSearchQuery { q: "   ".into(), ..Default::default() }.trimmed_term(), None);
        assert_eq!(MessageSearchQuery { q: "".into(), ..Default::default() }.trimmed_term(), None);
        assert_eq!(
            MessageSearchQuery { q: "  hi ".into(), ..Default::default() }.trimmed_term(),
            Some("hi")
        );
    }

    #[test]
    fn search_query_page_and_per_page_clamp() {
        let q = MessageSearchQuery::default();
        assert_eq!(q.clamped_page(), 1);
        assert_eq!(q.clamped_per_page(), DEFAULT_SEARCH_PER_PAGE);

        let q = MessageSearchQuery { page: Some(0), per_page: Some(9999), ..Default::default() };
        assert_eq!(q.clamped_page(), 1);
        assert_eq!(q.clamped_per_page(), MAX_SEARCH_PER_PAGE);

        let q = MessageSearchQuery { page: Some(-3), per_page: Some(0), ..Default::default() };
        assert_eq!(q.clamped_page(), 1);
        assert_eq!(q.clamped_per_page(), 1);
    }

    #[test]
    fn snippet_is_bounded_and_centers_on_term() {
        let short = "a short message with refund in it";
        assert_eq!(build_snippet(short, "refund"), short);

        let long = format!("{} refund {}", "x".repeat(400), "y".repeat(400));
        let snip = build_snippet(&long, "refund");
        assert!(snip.chars().count() <= SEARCH_SNIPPET_MAX_CHARS + 2, "len={}", snip.chars().count());
        assert!(snip.to_lowercase().contains("refund"));
        assert!(snip.starts_with('…') && snip.ends_with('…'));

        // Collapses whitespace/newlines.
        assert_eq!(build_snippet("line1\n\n  line2", "line2"), "line1 line2");
    }
}

/// Request to edit an existing message
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditMessageRequest {
    pub content: String,
}

/// Response when editing a message (creates new branch)
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct EditMessageResponse {
    pub message: Message,
    pub branch: Branch,
}
