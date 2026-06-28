//! Instance-level health state machine.
//!
//! Today's reality before this module: each engine impl returns a
//! [`HealthStatus`] enum, the supervisor matches on it and restarts
//! crashed instances immediately with no backoff (`supervisor.rs:99-118`),
//! and the server's `LocalDeployment::health_check` collapses every
//! state to a `bool`. That works but loses information and flaps on
//! a stuck engine.
//!
//! This module owns the *transitions*. Engines keep returning their
//! leaf [`HealthStatus`] (no trait churn — see
//! [`HealthStatus::to_signal`]); the supervisor feeds a
//! [`HealthSignal`] into [`HealthStateMachine::on_event`] and honors
//! the resulting [`Transition`].
//!
//! Backoff is exponential: `1s, 2s, 4s, 8s, 16s, 32s, 60s, 60s, …`
//! (doubling, capped at 60s).
//!
//! Flap detection: 5 [`HealthEvent::Crashed`] events within a 60s
//! sliding window flip the state machine to [`InstanceState::Failed`]
//! and stop further auto-restart attempts. The flap window is
//! independent of `restart_attempts` — the latter is a lifetime
//! counter, the former a rate counter, and either trip stops restarts.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// What an [`HealthStateMachine`] is currently in.
///
/// Persisted by the server into `llm_runtime_instances.state` as the
/// string name (`InstanceState::name`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceState {
    /// Engine spawned but `/health` hasn't reported `Ok` yet.
    Starting,

    /// Engine is responding cleanly.
    Healthy,

    /// Engine is responding but reports degraded health (e.g. model
    /// still loading after the initial start, or repeated 5xx).
    /// The instance still has a process — just not happy.
    Unhealthy { reason: String, since: Instant },

    /// Engine process has exited (caught by the periodic supervisor
    /// loop) or refused to bind. Distinct from `Unhealthy` because
    /// nothing's listening on the port anymore.
    Crashed {
        exit_signal: Option<i32>,
        at: Instant,
    },

    /// Backoff before the next restart attempt. The supervisor reads
    /// `next_at` to schedule its next try.
    Restarting { attempt: u32, next_at: Instant },

    /// Restart cap or flap window exceeded — the supervisor stops
    /// trying. Admin must clear via explicit `clear_failed()` (e.g.
    /// from a "retry" button) before further auto-restart happens.
    Failed { reason: String },

    /// Explicitly stopped by an admin or by the idle reaper. The
    /// supervisor leaves this alone.
    Stopped,
}

impl InstanceState {
    /// String name used for persistence and UI badges. Keep this
    /// stable — the server stores these strings in a VARCHAR column.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Healthy => "healthy",
            Self::Unhealthy { .. } => "unhealthy",
            Self::Crashed { .. } => "crashed",
            Self::Restarting { .. } => "restarting",
            Self::Failed { .. } => "failed",
            Self::Stopped => "stopped",
        }
    }

    // is_terminal / is_running removed — dead code; use on_event-based
    // state machine transitions instead.
}

/// Events that drive the state machine. The supervisor produces
/// these from health-check probes; the server module emits
/// [`HealthEvent::AdminStop`] / [`HealthEvent::AdminStart`] /
/// [`HealthEvent::ClearFailed`] from REST handlers.
///
/// # Dead-code note
/// `Ok`, `Unhealthy`, and `ClearFailed` variants are part of the state
/// machine's designed input set but currently never constructed: the
/// health-check → event path (Ok/Unhealthy) and the admin-rest →
/// event path (ClearFailed) were not wired up. AdminStop IS constructed
/// in tests. Keeping these variants so the state machine's match is
/// total — if a future patch wires the REST handler or the health
/// reporter, the enum contract is complete.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum HealthEvent {
    /// Engine reports `Ok` — equivalent to `HealthSignal::Ok`.
    Ok,
    /// Engine reports degraded health.
    Unhealthy(String),
    /// Engine process is gone.
    Crashed(Option<i32>),
    /// Backoff timer elapsed; supervisor is about to call
    /// `engine.start()` again.
    RestartAttempt,
    /// Engine started successfully (post-restart or first start).
    StartedOk,
    /// Admin explicitly stopped the instance.
    AdminStop,
    /// Admin explicitly cleared a `Failed` state.
    ClearFailed,
}

/// The outcome of `on_event` — what the supervisor should DO next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// State changed but no action required.
    StateChanged { from: String, to: String },
    /// Supervisor should kill + restart at `next_at`.
    Restart { attempt: u32, next_at: Instant },
    /// Supervisor should give up.
    GiveUp { reason: String },
    /// Nothing changed.
    NoOp,
}

/// Exponential backoff with a cap. Pure value-type — owns no clock.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    pub initial: Duration,
    pub max: Duration,
    pub factor: u32,
    /// 0 = first attempt; advance by `next_after` before each retry.
    pub attempt: u32,
}

impl ExponentialBackoff {
    pub fn new(initial: Duration, max: Duration) -> Self {
        Self {
            initial,
            max,
            factor: 2,
            attempt: 0,
        }
    }

    /// Returns the delay to apply BEFORE the next attempt and
    /// advances the internal counter.
    pub fn next_delay(&mut self) -> Duration {
        let base = self.initial.as_secs().saturating_mul(
            (self.factor.saturating_pow(self.attempt)).max(1) as u64,
        );
        let capped = base.min(self.max.as_secs());
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_secs(capped)
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

/// Sliding window of crash timestamps for flap detection.
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    pub window: Duration,
    pub max_events: usize,
    events: VecDeque<Instant>,
}

impl SlidingWindow {
    pub fn new(window: Duration, max_events: usize) -> Self {
        Self {
            window,
            max_events,
            events: VecDeque::new(),
        }
    }

    /// Record an event and return true if the window is now over
    /// the threshold (caller should treat this as a flap trip).
    pub fn record(&mut self, now: Instant) -> bool {
        // Drop expired events first.
        let horizon = now - self.window;
        while let Some(front) = self.events.front() {
            if *front < horizon {
                self.events.pop_front();
            } else {
                break;
            }
        }
        self.events.push_back(now);
        self.events.len() > self.max_events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Per-instance state machine. The supervisor owns one of these per
/// running model and feeds it events; the server module persists
/// `state` + `restart_attempts` + `last_failure_reason` to the DB.
#[derive(Debug, Clone)]
pub struct HealthStateMachine {
    pub state: InstanceState,
    pub restart_attempts: u32,
    pub max_restart_attempts: u32,
    pub backoff: ExponentialBackoff,
    pub flap_window: SlidingWindow,
}

impl HealthStateMachine {
    pub fn new(max_restart_attempts: u32) -> Self {
        Self {
            state: InstanceState::Starting,
            restart_attempts: 0,
            max_restart_attempts,
            backoff: ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(60)),
            flap_window: SlidingWindow::new(Duration::from_secs(60), 5),
        }
    }

    /// Feed an event; mutate state; return what the supervisor
    /// should do.
    pub fn on_event(&mut self, event: HealthEvent) -> Transition {
        self.on_event_at(event, Instant::now())
    }

    /// Same as [`on_event`] but lets tests inject a fake clock for
    /// the backoff and sliding-window calculations.
    pub fn on_event_at(&mut self, event: HealthEvent, now: Instant) -> Transition {
        let from = self.state.name().to_string();
        match (&self.state.clone(), event) {
            // --- HEALTHY-PATH TRANSITIONS ---
            (InstanceState::Starting, HealthEvent::Ok)
            | (InstanceState::Starting, HealthEvent::StartedOk)
            | (InstanceState::Unhealthy { .. }, HealthEvent::Ok)
            | (InstanceState::Restarting { .. }, HealthEvent::StartedOk) => {
                self.state = InstanceState::Healthy;
                self.backoff.reset();
                Transition::StateChanged {
                    from,
                    to: "healthy".into(),
                }
            }
            (InstanceState::Healthy, HealthEvent::Ok) => Transition::NoOp,

            // --- UNHEALTHY-PATH TRANSITIONS ---
            (InstanceState::Healthy | InstanceState::Starting, HealthEvent::Unhealthy(r)) => {
                self.state = InstanceState::Unhealthy {
                    reason: r,
                    since: now,
                };
                Transition::StateChanged {
                    from,
                    to: "unhealthy".into(),
                }
            }
            (InstanceState::Unhealthy { .. }, HealthEvent::Unhealthy(_)) => Transition::NoOp,

            // --- CRASH-PATH TRANSITIONS ---
            (
                InstanceState::Healthy
                | InstanceState::Starting
                | InstanceState::Unhealthy { .. }
                | InstanceState::Restarting { .. },
                HealthEvent::Crashed(signal),
            ) => {
                self.state = InstanceState::Crashed {
                    exit_signal: signal,
                    at: now,
                };

                let flapping = self.flap_window.record(now);
                if flapping {
                    let reason = format!(
                        "engine crashed {} times within {}s (flap protection)",
                        self.flap_window.events.len(),
                        self.flap_window.window.as_secs()
                    );
                    self.state = InstanceState::Failed {
                        reason: reason.clone(),
                    };
                    return Transition::GiveUp { reason };
                }

                if self.restart_attempts >= self.max_restart_attempts {
                    let reason = format!(
                        "exceeded max restart attempts ({}/{})",
                        self.restart_attempts, self.max_restart_attempts
                    );
                    self.state = InstanceState::Failed {
                        reason: reason.clone(),
                    };
                    return Transition::GiveUp { reason };
                }

                // Schedule a restart attempt
                let delay = self.backoff.next_delay();
                let next_at = now + delay;
                let attempt = self.restart_attempts + 1;
                self.state = InstanceState::Restarting { attempt, next_at };
                Transition::Restart { attempt, next_at }
            }

            (InstanceState::Restarting { .. }, HealthEvent::RestartAttempt) => {
                self.restart_attempts = self.restart_attempts.saturating_add(1);
                self.state = InstanceState::Starting;
                Transition::StateChanged {
                    from,
                    to: "starting".into(),
                }
            }

            // --- ADMIN / EXTERNAL ---
            (s, HealthEvent::AdminStop) if !matches!(s, InstanceState::Stopped) => {
                self.state = InstanceState::Stopped;
                self.backoff.reset();
                Transition::StateChanged {
                    from,
                    to: "stopped".into(),
                }
            }
            (InstanceState::Failed { .. }, HealthEvent::ClearFailed) => {
                self.state = InstanceState::Stopped;
                self.restart_attempts = 0;
                self.backoff.reset();
                self.flap_window.clear();
                Transition::StateChanged {
                    from,
                    to: "stopped".into(),
                }
            }

            // --- TERMINAL STATES ABSORB ---
            (InstanceState::Failed { .. } | InstanceState::Stopped, _) => Transition::NoOp,

            // --- UNHANDLED COMBINATIONS ---
            (_, _) => Transition::NoOp,
        }
    }

    /// Manual reset for a Failed instance.
    #[allow(dead_code)]
    pub fn clear_failed(&mut self) {
        if matches!(self.state, InstanceState::Failed { .. }) {
            self.state = InstanceState::Stopped;
            self.restart_attempts = 0;
            self.backoff.reset();
            self.flap_window.clear();
        }
    }

    // last_failure_reason removed — dead code.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn exponential_backoff_doubles_and_caps() {
        let mut b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(60));
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        assert_eq!(b.next_delay(), Duration::from_secs(32));
        assert_eq!(b.next_delay(), Duration::from_secs(60)); // capped
        assert_eq!(b.next_delay(), Duration::from_secs(60)); // stays capped
    }

    #[test]
    fn backoff_resets_on_healthy() {
        let mut sm = HealthStateMachine::new(10);
        // Crash to enter Restarting.
        sm.on_event(HealthEvent::Crashed(None));
        assert!(matches!(sm.state, InstanceState::Restarting { .. }));
        sm.on_event(HealthEvent::RestartAttempt);
        // Becoming healthy should reset backoff.
        sm.on_event(HealthEvent::StartedOk);
        assert!(matches!(sm.state, InstanceState::Healthy));
        assert_eq!(sm.backoff.attempt, 0);
    }

    #[test]
    fn sliding_window_trips_at_threshold() {
        let mut w = SlidingWindow::new(Duration::from_secs(60), 5);
        let t0 = Instant::now();
        assert!(!w.record(t0));
        assert!(!w.record(t0 + Duration::from_secs(1)));
        assert!(!w.record(t0 + Duration::from_secs(2)));
        assert!(!w.record(t0 + Duration::from_secs(3)));
        assert!(!w.record(t0 + Duration::from_secs(4)));
        // The 6th event in <60s window trips.
        assert!(w.record(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn sliding_window_drops_expired() {
        let mut w = SlidingWindow::new(Duration::from_secs(60), 5);
        let t0 = Instant::now();
        for i in 0..5 {
            w.record(t0 + Duration::from_secs(i));
        }
        // Long after the window has passed — should not trip.
        assert!(!w.record(t0 + Duration::from_secs(120)));
    }

    #[test]
    fn flap_protection_lands_failed() {
        let mut sm = HealthStateMachine::new(100); // huge restart cap
        let t0 = Instant::now();
        for i in 0..5 {
            sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(i));
            // Honor the Restart transition: schedule attempt
            sm.on_event_at(HealthEvent::RestartAttempt, t0 + Duration::from_secs(i));
        }
        // 6th crash within 60s should trip flap protection.
        let last = sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(6));
        assert!(matches!(last, Transition::GiveUp { .. }));
        assert!(matches!(sm.state, InstanceState::Failed { .. }));
    }

    #[test]
    fn restart_cap_lands_failed() {
        let mut sm = HealthStateMachine::new(2);
        // Crash 3 times with enough spacing that flap doesn't kick in.
        let t0 = Instant::now();
        sm.on_event_at(HealthEvent::Crashed(None), t0);
        sm.on_event_at(HealthEvent::RestartAttempt, t0);
        sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(120));
        sm.on_event_at(HealthEvent::RestartAttempt, t0 + Duration::from_secs(120));
        let last = sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(240));
        assert!(matches!(last, Transition::GiveUp { .. }));
        assert!(matches!(sm.state, InstanceState::Failed { .. }));
    }

    #[test]
    fn admin_stop_from_any_running_state() {
        let mut sm = HealthStateMachine::new(5);
        sm.on_event(HealthEvent::AdminStop);
        assert_eq!(sm.state.name(), "stopped");
    }

    #[test]
    fn clear_failed_returns_to_stopped() {
        let mut sm = HealthStateMachine::new(1);
        let t0 = Instant::now();
        sm.on_event_at(HealthEvent::Crashed(None), t0);
        sm.on_event_at(HealthEvent::RestartAttempt, t0);
        sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(120));
        assert!(matches!(sm.state, InstanceState::Failed { .. }));
        sm.clear_failed();
        assert_eq!(sm.state.name(), "stopped");
        assert_eq!(sm.restart_attempts, 0);
    }

    #[test]
    fn instance_state_name_stable() {
        // The server stores these strings in a VARCHAR column. Keep
        // them stable; bumping the schema requires a migration.
        let expectations = [
            (
                InstanceState::Starting,
                "starting",
            ),
            (InstanceState::Healthy, "healthy"),
            (
                InstanceState::Unhealthy {
                    reason: "".into(),
                    since: Instant::now(),
                },
                "unhealthy",
            ),
            (
                InstanceState::Crashed {
                    exit_signal: None,
                    at: Instant::now(),
                },
                "crashed",
            ),
            (
                InstanceState::Restarting {
                    attempt: 1,
                    next_at: Instant::now(),
                },
                "restarting",
            ),
            (
                InstanceState::Failed {
                    reason: "".into(),
                },
                "failed",
            ),
            (InstanceState::Stopped, "stopped"),
        ];
        for (s, expected) in expectations {
            assert_eq!(s.name(), expected);
        }
    }
}
