//! Instance-level health state machine for the managed whisper-server.
//!
//! Ported from `llm_local_runtime::engine::health`, scoped to the SINGLE
//! whisper-server instance (whisper transcribes one model at a time, so there is
//! exactly one of these in the process). It owns the *transitions* only — a pure
//! value type with no clock of its own: callers feed a [`HealthEvent`] into
//! [`HealthStateMachine::on_event`] and honor the resulting [`Transition`].
//!
//! Backoff is exponential: `1s, 2s, 4s, 8s, 16s, 32s, 60s, 60s, …` (doubling,
//! capped at 60s).
//!
//! Flap detection: 5 [`HealthEvent::Crashed`] events within a 60s sliding window
//! flip the state machine to [`InstanceState::Failed`] and stop further
//! auto-restart attempts. The flap window is independent of `restart_attempts` —
//! the latter is a lifetime counter, the former a rate counter, and either trip
//! stops restarts.
//!
//! The `state` string ([`InstanceState::name`]) + `restart_attempts` +
//! `last_failure_reason` are persisted by `auto_start` into the singleton
//! `voice_runtime_instance` row so the flap / give-up history survives a server
//! restart ([`HealthStateMachine::from_persisted`]).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// What an [`HealthStateMachine`] is currently in.
///
/// Persisted by the server into `voice_runtime_instance.state` as the string
/// name ([`InstanceState::name`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceState {
    /// whisper-server spawned but `/` hasn't reported `200` yet.
    Starting,

    /// whisper-server is responding cleanly.
    Healthy,

    /// whisper-server is responding but degraded (e.g. model still loading, or
    /// repeated 5xx). The instance still has a process — just not happy.
    Unhealthy { reason: String, since: Instant },

    /// whisper-server process has exited (caught by the reaper's health pass)
    /// or refused to bind. Distinct from `Unhealthy` because nothing's
    /// listening on the port anymore.
    Crashed {
        exit_signal: Option<i32>,
        at: Instant,
    },

    /// Backoff before the next restart attempt. The auto-start path reads
    /// `next_at` to decide whether to defer a restart.
    Restarting { attempt: u32, next_at: Instant },

    /// Restart cap or flap window exceeded — auto-start stops trying. Admin
    /// must clear via explicit [`HealthStateMachine::clear_failed`] (e.g. the
    /// `/voice/instance/restart` retry) before further auto-restart happens.
    Failed { reason: String },

    /// Explicitly stopped by an admin or by the idle reaper. Left alone.
    Stopped,
}

impl InstanceState {
    /// String name used for persistence and UI badges. Keep this stable — the
    /// server stores these strings in a VARCHAR column.
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
}

/// Events that drive the state machine. The reaper's periodic health pass
/// produces `Ok` / `Unhealthy`; the auto-start crash path produces `Crashed` /
/// `StartedOk`; the admin REST handlers produce `AdminStop` / `ClearFailed`.
#[derive(Debug, Clone)]
pub enum HealthEvent {
    /// whisper-server reports `200` on `/`.
    Ok,
    /// whisper-server reports degraded health.
    Unhealthy(String),
    /// whisper-server process is gone.
    Crashed(Option<i32>),
    /// Backoff timer elapsed; the caller is about to respawn. Part of the
    /// designed input set + exercised by tests; the lazy auto-start path drives
    /// restarts via the `Restart` transition rather than emitting this
    /// explicitly.
    #[allow(dead_code)]
    RestartAttempt,
    /// whisper-server started successfully (post-restart or first start).
    StartedOk,
    /// Admin explicitly stopped the instance.
    AdminStop,
    /// Admin explicitly cleared a `Failed` state.
    ClearFailed,
}

/// The outcome of `on_event` — what the caller should DO next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// State changed but no action required.
    StateChanged { from: String, to: String },
    /// Caller should kill + restart at `next_at`.
    Restart { attempt: u32, next_at: Instant },
    /// Caller should give up.
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
    /// 0 = first attempt; advance by `next_delay` before each retry.
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

    /// Returns the delay to apply BEFORE the next attempt and advances the
    /// internal counter.
    pub fn next_delay(&mut self) -> Duration {
        let base = self
            .initial
            .as_secs()
            .saturating_mul((self.factor.saturating_pow(self.attempt)).max(1) as u64);
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

    /// Record an event and return true if the window is now over the threshold
    /// (caller should treat this as a flap trip).
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

/// The single whisper-server instance's state machine. `auto_start` owns one of
/// these (behind a mutex) and feeds it events; it persists
/// `state` + `restart_attempts` + `last_failure_reason` to the singleton row.
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

    /// Rebuild a state machine from the persisted DB columns
    /// (`voice_runtime_instance.state` / `restart_attempts` /
    /// `last_failure_reason`) so the flap / give-up history survives a server
    /// restart. Without this the in-memory machine starts empty on boot and an
    /// instance the flap cap had already marked `failed` would be auto-respawned.
    ///
    /// Only the two terminal states (`Failed` / `Stopped`) are reconstructed
    /// verbatim — they carry no `Instant`. Every other persisted state is
    /// transient (the engine is not running just after a restart anyway), so we
    /// fall back to a fresh `Starting` state while still preserving the
    /// `restart_attempts` counter as the backstop against immediate re-flap.
    pub fn from_persisted(
        max_restart_attempts: u32,
        state_name: &str,
        restart_attempts: i32,
        last_failure_reason: Option<String>,
    ) -> Self {
        let mut sm = Self::new(max_restart_attempts);
        sm.restart_attempts = restart_attempts.max(0) as u32;
        sm.state = match state_name {
            "failed" => InstanceState::Failed {
                reason: last_failure_reason
                    .unwrap_or_else(|| "failed (restored from persisted state)".to_string()),
            },
            "stopped" => InstanceState::Stopped,
            _ => InstanceState::Starting,
        };
        sm
    }

    /// Feed an event; mutate state; return what the caller should do.
    pub fn on_event(&mut self, event: HealthEvent) -> Transition {
        self.on_event_at(event, Instant::now())
    }

    /// Same as [`on_event`](Self::on_event) but lets tests inject a fake clock
    /// for the backoff and sliding-window calculations.
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
                        "whisper-server crashed {} times within {}s (flap protection)",
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

                // Schedule a restart attempt.
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
                // Reuse the manual reset so the REST-driven ClearFailed path and
                // the direct `clear_failed()` helper share one implementation.
                self.clear_failed();
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

    /// Manual reset for a Failed instance. Driven in production by the admin
    /// `/voice/instance/restart` retry path via the `HealthEvent::ClearFailed`
    /// arm.
    pub fn clear_failed(&mut self) {
        if matches!(self.state, InstanceState::Failed { .. }) {
            self.state = InstanceState::Stopped;
            self.restart_attempts = 0;
            self.backoff.reset();
            self.flap_window.clear();
        }
    }
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
        sm.on_event(HealthEvent::Crashed(None));
        assert!(matches!(sm.state, InstanceState::Restarting { .. }));
        sm.on_event(HealthEvent::RestartAttempt);
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
        assert!(w.record(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn sliding_window_drops_expired() {
        let mut w = SlidingWindow::new(Duration::from_secs(60), 5);
        let t0 = Instant::now();
        for i in 0..5 {
            w.record(t0 + Duration::from_secs(i));
        }
        assert!(!w.record(t0 + Duration::from_secs(120)));
    }

    #[test]
    fn flap_protection_lands_failed() {
        let mut sm = HealthStateMachine::new(100); // huge restart cap
        let t0 = Instant::now();
        for i in 0..5 {
            sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(i));
            sm.on_event_at(HealthEvent::RestartAttempt, t0 + Duration::from_secs(i));
        }
        let last = sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(6));
        assert!(matches!(last, Transition::GiveUp { .. }));
        assert!(matches!(sm.state, InstanceState::Failed { .. }));
    }

    #[test]
    fn restart_cap_lands_failed() {
        let mut sm = HealthStateMachine::new(2);
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
    fn clear_failed_event_resets_to_stopped() {
        let mut sm = HealthStateMachine::new(1);
        let t0 = Instant::now();
        sm.on_event_at(HealthEvent::Crashed(None), t0);
        sm.on_event_at(HealthEvent::RestartAttempt, t0);
        sm.on_event_at(HealthEvent::Crashed(None), t0 + Duration::from_secs(120));
        assert!(matches!(sm.state, InstanceState::Failed { .. }));

        let t = sm.on_event(HealthEvent::ClearFailed);
        assert!(matches!(
            t,
            Transition::StateChanged { ref to, .. } if to == "stopped"
        ));
        assert_eq!(sm.state.name(), "stopped");
        assert_eq!(sm.restart_attempts, 0);
    }

    #[test]
    fn unhealthy_then_ok_round_trips() {
        let mut sm = HealthStateMachine::new(5);
        sm.on_event(HealthEvent::StartedOk);
        assert_eq!(sm.state.name(), "healthy");

        let t = sm.on_event(HealthEvent::Unhealthy("5xx".into()));
        assert!(matches!(t, Transition::StateChanged { ref to, .. } if to == "unhealthy"));
        assert_eq!(sm.state.name(), "unhealthy");

        let t = sm.on_event(HealthEvent::Ok);
        assert!(matches!(t, Transition::StateChanged { ref to, .. } if to == "healthy"));
        assert_eq!(sm.state.name(), "healthy");

        assert!(matches!(sm.on_event(HealthEvent::Ok), Transition::NoOp));
    }

    #[test]
    fn from_persisted_restores_failed_and_counter() {
        let sm = HealthStateMachine::from_persisted(5, "failed", 3, Some("boom".into()));
        assert_eq!(sm.state.name(), "failed");
        assert_eq!(sm.restart_attempts, 3);
        let sm2 = HealthStateMachine::from_persisted(5, "running", 1, None);
        // Non-terminal persisted state falls back to Starting, counter preserved.
        assert_eq!(sm2.state.name(), "starting");
        assert_eq!(sm2.restart_attempts, 1);
    }

    #[test]
    fn instance_state_name_stable() {
        let expectations = [
            (InstanceState::Starting, "starting"),
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
            (InstanceState::Failed { reason: "".into() }, "failed"),
            (InstanceState::Stopped, "stopped"),
        ];
        for (s, expected) in expectations {
            assert_eq!(s.name(), expected);
        }
    }
}
