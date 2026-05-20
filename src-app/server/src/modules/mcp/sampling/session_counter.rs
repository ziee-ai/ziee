// Session counter for limiting concurrent MCP sampling sessions per server
//
// Uses a global Mutex<HashMap> with RAII guards to ensure counters are always released,
// even if the session panics or is dropped early.
//
// NOTE: SESSION_COUNTS is process-global. For multi-process deployments (load balancing),
// move this counter to a shared store (Redis) or accept that limits are per-process.

use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;
use once_cell::sync::Lazy;

use crate::common::AppError;

/// Global session counter map: server_id → active session count
static SESSION_COUNTS: Lazy<Mutex<HashMap<Uuid, u32>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Acquire the global lock, recovering from poisoning rather than propagating it.
fn lock_counts() -> std::sync::MutexGuard<'static, HashMap<Uuid, u32>> {
    SESSION_COUNTS.lock().unwrap_or_else(|poisoned| {
        tracing::error!("[sampling] Session counter mutex was poisoned — recovering");
        poisoned.into_inner()
    })
}

/// RAII guard that decrements the counter when dropped
pub struct SessionGuard {
    server_id: Uuid,
    /// True when this guard is counting against the server's limit.
    /// False for unlimited sessions (None or 0 max) — no decrement needed on drop.
    holds_slot: bool,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if !self.holds_slot {
            return;
        }
        let mut counts = lock_counts();
        if let Some(count) = counts.get_mut(&self.server_id) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                counts.remove(&self.server_id);
            }
        }
    }
}

/// Attempt to acquire a session slot for the given server.
///
/// - `max`: maximum concurrent sessions (`None` = unlimited)
/// - Returns `Ok(SessionGuard)` if a slot is available
/// - Returns `Err` if the limit is reached
pub fn acquire_session(server_id: Uuid, max: Option<i32>) -> Result<SessionGuard, AppError> {
    let max = match max {
        None => return Ok(SessionGuard { server_id, holds_slot: false }),
        Some(m) if m <= 0 => return Ok(SessionGuard { server_id, holds_slot: false }),
        Some(m) => m as u32,
    };

    let mut counts = lock_counts();

    let count = counts.entry(server_id).or_insert(0);
    if *count >= max {
        return Err(AppError::bad_request(
            "SAMPLING_CAPACITY_EXCEEDED",
            "This MCP server is at capacity. Please try again shortly.",
        ));
    }
    *count += 1;

    Ok(SessionGuard { server_id, holds_slot: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_session_no_limit() {
        let id = Uuid::new_v4();
        let guard = acquire_session(id, None).expect("Should succeed with None limit");
        assert!(!guard.holds_slot, "Unlimited guard should not hold a slot");
        drop(guard);

        // Some(0) also means unlimited
        let guard2 = acquire_session(id, Some(0)).expect("Should succeed with 0 limit");
        assert!(!guard2.holds_slot, "Zero limit guard should not hold a slot");
    }

    #[test]
    fn test_acquire_session_within_limit() {
        let id = Uuid::new_v4();
        let guard1 = acquire_session(id, Some(2)).expect("First acquire should succeed");
        assert!(guard1.holds_slot);
        let guard2 = acquire_session(id, Some(2)).expect("Second acquire should succeed");
        assert!(guard2.holds_slot);

        // Drop first, count goes to 1
        drop(guard1);

        // Can acquire a third (count was 2, now 1, limit is 2)
        let guard3 = acquire_session(id, Some(2)).expect("After drop, should succeed");
        assert!(guard3.holds_slot);
        drop(guard2);
        drop(guard3);
    }

    #[test]
    fn test_acquire_session_at_capacity() {
        let id = Uuid::new_v4();
        let guard1 = acquire_session(id, Some(2)).expect("First acquire should succeed");
        let guard2 = acquire_session(id, Some(2)).expect("Second acquire should succeed");

        // 3rd should fail - at capacity
        let result = acquire_session(id, Some(2));
        assert!(result.is_err(), "Should fail when at capacity");

        // Drop one slot and verify we can acquire again
        drop(guard1);
        let guard3 = acquire_session(id, Some(2)).expect("After dropping one, should succeed");
        drop(guard2);
        drop(guard3);
    }

    #[test]
    fn test_raii_guard_releases_on_drop() {
        let id = Uuid::new_v4();
        {
            let _guard = acquire_session(id, Some(1)).expect("Should succeed");
            // At capacity: second acquire should fail
            let result = acquire_session(id, Some(1));
            assert!(result.is_err(), "Should be at capacity");
        }
        // Guard dropped — slot released
        let guard = acquire_session(id, Some(1)).expect("After drop, should succeed");
        drop(guard);
    }

    #[test]
    fn test_acquire_session_second_concurrent_fails() {
        // Use a fresh UUID so this test doesn't affect the global SESSION_COUNTS state
        // shared across other tests in the same process
        let server_id = Uuid::new_v4();

        // First acquisition succeeds: slot count goes from 0 → 1
        let guard1 = acquire_session(server_id, Some(1))
            .expect("First session should succeed when server has no active sessions");
        assert!(guard1.holds_slot);

        // Second acquisition must fail: slot count is already at the limit (1/1)
        let result2 = acquire_session(server_id, Some(1));
        assert!(
            result2.is_err(),
            "Second concurrent session must be rejected when max_concurrent_sessions=1 is held"
        );

        // Drop the first guard: RAII decrements count back to 0
        drop(guard1);

        // Now the slot is free: a new acquisition must succeed
        let guard3 = acquire_session(server_id, Some(1));
        assert!(
            guard3.is_ok(),
            "Session acquisition should succeed again after the previous guard is dropped"
        );
    }
}
