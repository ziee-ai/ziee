//! Unit tests for the MCP client's HTTP 429 wait-hint parser + backoff math.
//!
//! These are the pure helpers behind Fix B (the client honors a 429 wait hint
//! and retries instead of failing the tool call). The end-to-end retry behavior
//! lives in `rate_limit_retry_test.rs`; this covers the parsing/clamping edge
//! cases the public path can't easily reach. Exercised via the doc-hidden
//! `ziee::test_internals` re-exports.

use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
use ziee::test_internals::{
    rate_limit_delay_ms, rate_limit_wait_ms, RL_RETRY_INITIAL_MS, RL_RETRY_MAX_MS,
};

fn no_headers() -> HeaderMap {
    HeaderMap::new()
}

#[test]
fn wait_ms_prefers_retry_after_header_seconds() {
    let mut h = HeaderMap::new();
    h.insert(RETRY_AFTER, HeaderValue::from_static("2"));
    assert_eq!(rate_limit_wait_ms(&h, ""), Some(2000));
}

#[test]
fn wait_ms_header_takes_precedence_over_body() {
    let mut h = HeaderMap::new();
    h.insert(RETRY_AFTER, HeaderValue::from_static("1"));
    assert_eq!(rate_limit_wait_ms(&h, "Wait for 9s"), Some(1000));
}

#[test]
fn wait_ms_parses_wait_for_body_suffix() {
    assert_eq!(
        rate_limit_wait_ms(&no_headers(), "Too Many Requests! Wait for 3s"),
        Some(3000)
    );
}

#[test]
fn wait_ms_zero_hint_is_none_so_caller_uses_backoff() {
    // Our loopback governor emits "Wait for 0s" when the bucket will refill
    // imminently — treat as "no useful hint" and fall back to exponential.
    assert_eq!(rate_limit_wait_ms(&no_headers(), "Wait for 0s"), None);
    let mut h = HeaderMap::new();
    h.insert(RETRY_AFTER, HeaderValue::from_static("0"));
    assert_eq!(rate_limit_wait_ms(&h, ""), None);
}

#[test]
fn wait_ms_none_when_no_hint_present() {
    assert_eq!(rate_limit_wait_ms(&no_headers(), "Too Many Requests"), None);
    assert_eq!(rate_limit_wait_ms(&no_headers(), ""), None);
}

#[test]
fn delay_ms_honors_hint_within_cap() {
    assert_eq!(rate_limit_delay_ms(0, Some(1500)), 1500);
}

#[test]
fn delay_ms_clamps_absurd_hint_to_cap() {
    assert_eq!(rate_limit_delay_ms(0, Some(10_000_000)), RL_RETRY_MAX_MS);
}

#[test]
fn delay_ms_exponential_backoff_without_hint() {
    assert_eq!(rate_limit_delay_ms(0, None), RL_RETRY_INITIAL_MS);
    assert_eq!(rate_limit_delay_ms(1, None), RL_RETRY_INITIAL_MS * 2);
    assert_eq!(rate_limit_delay_ms(2, None), RL_RETRY_INITIAL_MS * 4);
    // Exponential growth is capped.
    assert_eq!(rate_limit_delay_ms(20, None), RL_RETRY_MAX_MS);
}

#[test]
fn delay_ms_takes_max_of_hint_and_exponential() {
    // Plan B intent: a tiny hint at a deep retry attempt MUST NOT short-circuit
    // the exponential floor — we still back off appropriately. attempt=5 →
    // exp = INITIAL * 32, which is well above any plausible "short" hint.
    let big_exp = RL_RETRY_INITIAL_MS.saturating_mul(32);
    let capped_exp = big_exp.min(RL_RETRY_MAX_MS);
    assert_eq!(rate_limit_delay_ms(5, Some(50)), capped_exp);
    assert_eq!(rate_limit_delay_ms(5, Some(100)), capped_exp);
    // Once the hint exceeds the exponential floor, the hint wins (still clamped).
    assert_eq!(
        rate_limit_delay_ms(0, Some(RL_RETRY_INITIAL_MS * 2)),
        RL_RETRY_INITIAL_MS * 2
    );
}
