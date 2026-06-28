//! Cheap, provider-agnostic token estimation.
//!
//! Uses the `ceil(chars / 4)` heuristic — no tokenizer dependency, stable
//! across providers and local engines. Genuinely shared (via
//! [`tokens_from_chars`]) by the chat context-trimming transform
//! (`clear_old_tool_results`) and the token-aware conversation summarizer. It is
//! intentionally an ESTIMATE: never used for billing, only for "is this getting
//! large enough to act on it".

/// Convert a precomputed Unicode-scalar count into estimated tokens as
/// `ceil(chars / 4)`. The single source of truth for the chars→tokens rounding,
/// so callers that already have a char total (e.g. summed over many content
/// blocks) don't re-implement (and drift on) the heuristic.
pub fn tokens_from_chars(chars: usize) -> usize {
    chars.div_ceil(4)
}

/// Estimate the number of tokens in `s` as `ceil(chars / 4)`.
///
/// Counts Unicode scalar values (not bytes) so multibyte text isn't
/// over-counted. Empty string → 0.
pub fn estimate_tokens(s: &str) -> usize {
    tokens_from_chars(s.chars().count())
}

/// Estimate tokens across many strings (e.g. all text blocks of a message).
#[allow(dead_code)]
pub fn estimate_tokens_iter<'a, I: IntoIterator<Item = &'a str>>(parts: I) -> usize {
    parts.into_iter().map(estimate_tokens).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn rounds_up() {
        // 4 chars -> 1, 5 chars -> 2, 8 chars -> 2
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn counts_chars_not_bytes() {
        // 4 multibyte chars -> 1 token (not 12 from byte length)
        assert_eq!(estimate_tokens("héllo".chars().take(4).collect::<String>().as_str()), 1);
    }

    #[test]
    fn iter_sums() {
        assert_eq!(estimate_tokens_iter(["abcd", "abcd"]), 2);
    }
}
