//! Crate-local token estimation (relocated from the ziee `common/tokens.rs`, so
//! the crate needs no server dep). ~chars/4 heuristic — same as the summarizer +
//! chat streaming use. Not tokenizer-exact; used for compaction budgeting.

/// Estimate token count for a string (~chars/4, min 1 for non-empty).
pub fn estimate_tokens(s: &str) -> usize {
    tokens_from_chars(s.chars().count())
}

/// Token count from a Unicode-scalar count.
pub fn tokens_from_chars(chars: usize) -> usize {
    if chars == 0 {
        0
    } else {
        chars.div_ceil(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(tokens_from_chars(0), 0);
    }

    #[test]
    fn rounds_up() {
        assert_eq!(tokens_from_chars(1), 1);
        assert_eq!(tokens_from_chars(4), 1);
        assert_eq!(tokens_from_chars(5), 2);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }
}
