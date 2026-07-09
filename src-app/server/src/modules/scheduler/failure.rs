//! Failure taxonomy (DEC-18) — pure, unit-testable.
//!
//! A firing either succeeds or fails. Failures are classified so the tick can
//! decide retry-vs-terminal: auth/permission/validation problems are **terminal**
//! (retrying won't help — disable + notify), while transient problems
//! (timeout / provider blip / 5xx) are retryable. A task that fails
//! `max_consecutive_failures` times in a row is **auto-paused** (flap cap).

use axum::http::StatusCode;

/// The failure bucket recorded on a `scheduled_task_runs` row (`error_class`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// Timeout / provider blip / 5xx — worth retrying with backoff.
    Transient,
    /// 401 — bad/expired credentials. Terminal.
    Auth,
    /// 403 — the owner lacks access to the target/model. Terminal.
    Permission,
    /// 400 / 422 — malformed task (e.g. bad inputs). Terminal.
    Validation,
    /// The workflow / model the task references was deleted. Terminal.
    TargetMissing,
    /// Anything else. Treated as terminal (don't hammer on an unknown fault).
    Internal,
}

impl FailureClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureClass::Transient => "transient",
            FailureClass::Auth => "auth",
            FailureClass::Permission => "permission",
            FailureClass::Validation => "validation",
            FailureClass::TargetMissing => "target_missing",
            FailureClass::Internal => "internal",
        }
    }

    /// Whether a firing that failed this way should be retried within the run
    /// (with backoff) rather than counted as a hard failure.
    pub fn is_retryable(&self) -> bool {
        matches!(self, FailureClass::Transient)
    }
}

/// Classify an HTTP status (from the dispatch's `AppError`) into a failure
/// bucket. `is_timeout` covers non-HTTP transient faults (wall-clock timeout,
/// connection reset) the caller detects out of band.
pub fn classify(status: StatusCode, is_timeout: bool) -> FailureClass {
    if is_timeout {
        return FailureClass::Transient;
    }
    match status {
        StatusCode::UNAUTHORIZED => FailureClass::Auth,
        StatusCode::FORBIDDEN => FailureClass::Permission,
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => FailureClass::Validation,
        StatusCode::NOT_FOUND => FailureClass::TargetMissing,
        s if s.is_server_error() => FailureClass::Transient,
        StatusCode::TOO_MANY_REQUESTS | StatusCode::REQUEST_TIMEOUT => FailureClass::Transient,
        _ => FailureClass::Internal,
    }
}

/// Should the task auto-pause given the failure count that INCLUDES the firing
/// just recorded? (`consecutive_failures` is the post-increment value.)
pub fn should_autopause(consecutive_failures: i32, max_consecutive_failures: i32) -> bool {
    consecutive_failures >= max_consecutive_failures.max(1)
}

/// Exponential backoff (with a cap) for the Nth in-run retry of a transient
/// failure. `attempt` is 0-based.
pub fn retry_backoff_ms(attempt: u32) -> u64 {
    // 500ms, 1s, 2s, 4s … capped at 30s. Jitter is applied by the caller.
    let base = 500u64.saturating_mul(1u64 << attempt.min(6));
    base.min(30_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-30: auth/perm/validation are terminal; timeout/5xx transient.
    #[test]
    fn classifies_terminal_vs_transient() {
        assert_eq!(classify(StatusCode::UNAUTHORIZED, false), FailureClass::Auth);
        assert_eq!(classify(StatusCode::FORBIDDEN, false), FailureClass::Permission);
        assert_eq!(classify(StatusCode::BAD_REQUEST, false), FailureClass::Validation);
        assert_eq!(
            classify(StatusCode::UNPROCESSABLE_ENTITY, false),
            FailureClass::Validation
        );
        assert_eq!(
            classify(StatusCode::NOT_FOUND, false),
            FailureClass::TargetMissing
        );

        assert!(!FailureClass::Auth.is_retryable());
        assert!(!FailureClass::Permission.is_retryable());
        assert!(!FailureClass::Validation.is_retryable());

        assert_eq!(
            classify(StatusCode::INTERNAL_SERVER_ERROR, false),
            FailureClass::Transient
        );
        assert_eq!(
            classify(StatusCode::BAD_GATEWAY, false),
            FailureClass::Transient
        );
        assert_eq!(classify(StatusCode::OK, true), FailureClass::Transient);
        assert!(FailureClass::Transient.is_retryable());
    }

    // TEST-30: auto-pause once the consecutive-failure count crosses the cap.
    #[test]
    fn autopause_at_threshold() {
        assert!(!should_autopause(4, 5));
        assert!(should_autopause(5, 5));
        assert!(should_autopause(6, 5));
        // A zero/negative cap is floored to 1 so a task can't spin forever.
        assert!(should_autopause(1, 0));
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(retry_backoff_ms(0), 500);
        assert_eq!(retry_backoff_ms(1), 1000);
        assert_eq!(retry_backoff_ms(2), 2000);
        assert_eq!(retry_backoff_ms(20), 30_000); // capped
    }
}
