//! Cheap, provider-agnostic token estimation.
//!
//! Uses the `chars / 4` heuristic — no tokenizer dependency, stable
//! across providers and local engines. Shared by the chat context-trimming
//! transform (clear old tool results past a threshold) and the token-aware
//! conversation summarizer. It is intentionally an ESTIMATE: it is never
//! used for billing, only for "is this getting large enough to act on it".

/// Estimate the number of tokens in `s` as `ceil(chars / 4)`.
///
/// Counts Unicode scalar values (not bytes) so multibyte text isn't
/// over-counted. Empty string → 0.
pub fn estimate_tokens(s: &str) -> usize {
    let chars = s.chars().count();
    chars.div_ceil(4)
}

/// Estimate tokens across many strings (e.g. all text blocks of a message).
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
