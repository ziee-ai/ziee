//! Budget + stop-condition accounting for one agent turn (ITEM-5).
//!
//! Caps mirror the workflow runner (`PER_RUN_TOKEN_CAP` / `PER_STEP_TOKEN_CAP`)
//! and the chat loop's `SAFETY_MAX_ITERATIONS`. The host supplies the caps
//! (from `agent_admin_settings`, DEC-6); this struct only tallies + decides.

use crate::types::StopReason;

/// Hard failsafe on loop iterations (Goose's `DEFAULT_MAX_TURNS` analog); the
/// admin `default_max_steps` (DEC-7 = 30) is the practical cap layered on top.
pub const SAFETY_MAX_ITERATIONS: u32 = 1000;

/// Token/iteration budget for a single agent turn.
#[derive(Debug, Clone)]
pub struct Budget {
    pub max_steps: u32,
    pub per_run_token_cap: u64,
    pub per_step_token_cap: u64,
    run_tokens: u64,
}

impl Budget {
    pub fn new(max_steps: u32, per_run_token_cap: u64, per_step_token_cap: u64) -> Self {
        Self {
            max_steps: max_steps.min(SAFETY_MAX_ITERATIONS),
            per_run_token_cap,
            per_step_token_cap,
            run_tokens: 0,
        }
    }

    /// Fold a model call's usage into the running total.
    pub fn add_tokens(&mut self, n: u64) {
        self.run_tokens = self.run_tokens.saturating_add(n);
    }

    pub fn run_tokens(&self) -> u64 {
        self.run_tokens
    }

    /// Whether the loop must stop *before* issuing another model call at
    /// `iteration` (1-based). Returns the `StopReason`, or `None` to continue.
    pub fn stop_before(&self, iteration: u32) -> Option<StopReason> {
        if iteration > self.max_steps {
            Some(StopReason::IterationCap)
        } else if self.run_tokens > self.per_run_token_cap {
            Some(StopReason::TokenCap)
        } else {
            None
        }
    }

    /// Whether a single step's tokens breached the per-step cap.
    pub fn step_over_cap(&self, step_tokens: u64) -> bool {
        step_tokens > self.per_step_token_cap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_cap_trips() {
        let b = Budget::new(3, 1_000_000, 500_000);
        assert_eq!(b.stop_before(3), None);
        assert_eq!(b.stop_before(4), Some(StopReason::IterationCap));
    }

    #[test]
    fn token_cap_trips() {
        let mut b = Budget::new(30, 100, 100);
        b.add_tokens(101);
        assert_eq!(b.stop_before(1), Some(StopReason::TokenCap));
    }

    #[test]
    fn safety_cap_clamps_max_steps() {
        let b = Budget::new(10_000, 1, 1);
        assert_eq!(b.max_steps, SAFETY_MAX_ITERATIONS);
    }

    #[test]
    fn per_step_cap() {
        let b = Budget::new(30, 1_000, 200);
        assert!(b.step_over_cap(201));
        assert!(!b.step_over_cap(200));
    }
}
