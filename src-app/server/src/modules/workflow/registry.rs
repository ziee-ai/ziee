//! In-memory per-run handle registry (§4.3 + §4.4 + §4.6).
//!
//! Each in-flight `workflow_runs` row gets a `RunHandle` with:
//! - a cancellation `Notify` (the runner's `tokio::select!` arms wait
//!   on `cancel.notified()` so any branch that's awaiting can be
//!   preempted instantly),
//! - a per-client `mpsc::UnboundedSender` map for the per-run SSE
//!   endpoint (matches the existing `code_sandbox/version_install_tasks.rs`
//!   + `llm_model/handlers/downloads.rs` patterns),
//! - an optional pending-elicitation slot (oneshot reply channel +
//!   id, set/cleared by `ElicitDispatcher`).
//!
//! The registry survives only in-memory. On server restart any orphan
//! `running` / `pending` rows are flipped to `failed` by
//! `startup_sweep.rs`; clients reconnecting after a restart get the
//! terminal status on subscribe.
//!
//! NOTE on the cancellation primitive: the codebase ships a
//! `utils/cancellation::CancellationToken` for downloads. That one is
//! poll-only (`is_cancelled().await` consumes its inner receiver) and
//! doesn't compose with `tokio::select!` across multiple await points
//! the way the runner needs. The runner uses a `Notify` instead — it's
//! the natural primitive for "wake every waiter when this fires", which
//! is exactly the runner's per-step `select!` shape.


use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::response::sse::Event;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::sync::{Notify, mpsc, oneshot};
use uuid::Uuid;

use crate::modules::workflow::events::SSEWorkflowRunEvent;

pub type ClientId = Uuid;
pub type ClientSender = mpsc::UnboundedSender<Result<Event, axum::Error>>;

/// Pending elicitation reply slot. Set by `ElicitDispatcher::dispatch`
/// while it's awaiting; consumed by `POST /elicit/{id}` (or cleared by
/// cancel / timeout).
pub struct PendingElicit {
    pub id: Uuid,
    pub tx: oneshot::Sender<serde_json::Value>,
}

/// Default effective run timeout (seconds) — mirrors `runner::RUN_WALL_CLOCK`.
/// The runner overrides this per run from the workflow's `max_runtime_secs`
/// (or the engine default) right after `register`; `0` means UNBOUNDED.
pub const DEFAULT_RUN_TIMEOUT_SECS: u64 = 30 * 60;

pub struct RunHandle {
    /// Signalled by `cancel()` — any `tokio::select!` waiting on
    /// `cancel.notified()` returns immediately.
    pub cancel: Arc<Notify>,
    /// Live-adjustable wall-clock cap (seconds; `0` = unbounded). Read by the
    /// runner's deadline watcher each tick, so `PUT .../timeout` takes effect
    /// mid-run. In-memory only — runs don't survive a restart.
    pub timeout_secs: Arc<std::sync::atomic::AtomicU64>,
    /// Once `cancel` has fired, this stays true so a future entrant
    /// into a step's select! arm exits without waiting forever (Notify
    /// is "edge"-shaped — waiters added after the notify_waiters() call
    /// would otherwise block).
    pub cancelled: Arc<std::sync::atomic::AtomicBool>,
    /// Durable resume: true while a runner task is resident on this handle;
    /// false for a handle that exists ONLY to hold SSE clients (a run that
    /// SUSPENDED on a `timeout_ms: 0` gate keeps its handle so subscribers stay
    /// attached, but has no runner until `resume_run`). `resume_run`'s
    /// idempotency guard + `submit_elicit`'s hot/cold branch key on this rather
    /// than mere handle presence (a subscriber alone creates a handle).
    pub has_runner: Arc<std::sync::atomic::AtomicBool>,
    pub clients: Arc<Mutex<HashMap<ClientId, ClientSender>>>,
    pub pending_elicitation: Arc<Mutex<Option<PendingElicit>>>,
    pub created_at: Instant,
}

impl RunHandle {
    fn new() -> Self {
        Self {
            cancel: Arc::new(Notify::new()),
            timeout_secs: Arc::new(std::sync::atomic::AtomicU64::new(DEFAULT_RUN_TIMEOUT_SECS)),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            has_runner: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            clients: Arc::new(Mutex::new(HashMap::new())),
            pending_elicitation: Arc::new(Mutex::new(None)),
            created_at: Instant::now(),
        }
    }

    /// True when a runner task is resident (vs a clients-only handle).
    pub fn has_runner(&self) -> bool {
        self.has_runner.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Await cancellation. Returns immediately if cancel has already
    /// been signalled — defensive against the `Notify` edge-trigger
    /// race (waiter added after `notify_waiters()`).
    pub async fn await_cancel(&self) {
        if self.is_cancelled() {
            return;
        }
        self.cancel.notified().await;
    }
}

/// Global DashMap keyed by run_id. Mirrors
/// `code_sandbox/version_install_tasks.rs::INSTALL_TASKS`.
pub static RUN_HANDLES: Lazy<DashMap<Uuid, Arc<RunHandle>>> = Lazy::new(DashMap::new);

/// Cap on concurrent SSE subscribers per run. Above this, subscribe
/// is refused with 429 (the FE retries; in practice users have at most
/// one or two tabs open per run).
pub const MAX_CLIENTS_PER_RUN: usize = 32;

pub fn register(run_id: Uuid) -> Arc<RunHandle> {
    // Get-or-create: a cold subscriber may have already created a clients-only
    // handle for this run (durable resume); reuse it so its SSE clients stay
    // attached across the suspend → resume boundary. Mark a runner resident.
    let handle = RUN_HANDLES
        .entry(run_id)
        .or_insert_with(|| Arc::new(RunHandle::new()))
        .value()
        .clone();
    handle
        .has_runner
        .store(true, std::sync::atomic::Ordering::Relaxed);
    handle
}

pub fn unregister(run_id: Uuid) {
    RUN_HANDLES.remove(&run_id);
}

/// Durable resume: the runner suspended on a `timeout_ms: 0` gate. Clear the
/// runner-resident flag but KEEP the handle so subscribers' SSE streams stay
/// attached (and `resume_run` reuses the same handle + clients). The handle is
/// reaped by TTL if the run never resumes / is never reopened.
pub fn set_no_runner(run_id: Uuid) {
    if let Some(h) = get(run_id) {
        h.has_runner
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// True when a runner task is currently resident for this run (vs a
/// clients-only handle or no handle at all).
pub fn runner_resident(run_id: Uuid) -> bool {
    get(run_id).map(|h| h.has_runner()).unwrap_or(false)
}

pub fn get(run_id: Uuid) -> Option<Arc<RunHandle>> {
    RUN_HANDLES.get(&run_id).map(|r| r.value().clone())
}

/// Set the live wall-clock cap (seconds; `0` = unbounded) for an in-flight run.
/// The runner's deadline watcher honors the new value within its recheck
/// interval. Returns `false` if the handle has already exited.
pub fn set_timeout(run_id: Uuid, secs: u64) -> bool {
    if let Some(h) = get(run_id) {
        h.timeout_secs
            .store(secs, std::sync::atomic::Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Fire the cancel signal for `run_id`. Idempotent — repeat calls
/// after the first are no-ops. Returns `false` if the handle has
/// already exited (runner removed itself from the registry).
pub fn cancel(run_id: Uuid) -> bool {
    if let Some(h) = get(run_id) {
        h.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        h.cancel.notify_waiters();
        true
    } else {
        false
    }
}

// ============================================================
// SSE client lifecycle
// ============================================================

pub fn register_client(run_id: Uuid, tx: ClientSender) -> Result<ClientId, &'static str> {
    // Create-if-absent: a client may subscribe to a SUSPENDED (cold, `waiting`)
    // run that has no resident runner — it still needs its snapshot + a live
    // stream that `resume_run` later emits to. The created handle is
    // clients-only (`has_runner` stays false) until a runner adopts it.
    let handle = RUN_HANDLES
        .entry(run_id)
        .or_insert_with(|| Arc::new(RunHandle::new()))
        .value()
        .clone();
    let mut clients = handle.clients.lock().map_err(|_| "client map poisoned")?;
    if clients.len() >= MAX_CLIENTS_PER_RUN {
        return Err("too many subscribers");
    }
    let id = Uuid::new_v4();
    clients.insert(id, tx);
    Ok(id)
}

pub fn remove_client(run_id: Uuid, client_id: ClientId) {
    if let Some(handle) = get(run_id)
        && let Ok(mut clients) = handle.clients.lock()
    {
        clients.remove(&client_id);
    }
}

/// Broadcast `ev` to every client of `run_id`. Dead senders (client
/// disconnected) are pruned. Silent on unknown run_id (the runner may
/// have already removed the handle; reconnect path reads from DB).
pub fn broadcast(run_id: Uuid, ev: SSEWorkflowRunEvent) {
    let handle = match get(run_id) {
        Some(h) => h,
        None => return,
    };
    let snapshot: Vec<(ClientId, ClientSender)> = match handle.clients.lock() {
        Ok(g) => g.iter().map(|(k, v)| (*k, v.clone())).collect(),
        Err(_) => return,
    };
    let axum_event: Event = ev.into();
    let mut dead: Vec<ClientId> = Vec::new();
    for (id, tx) in &snapshot {
        if tx.send(Ok(axum_event.clone())).is_err() {
            dead.push(*id);
        }
    }
    if !dead.is_empty()
        && let Ok(mut g) = handle.clients.lock()
    {
        for id in dead {
            g.remove(&id);
        }
    }
}

// ============================================================
// Elicitation
// ============================================================

/// Park a oneshot reply slot for `elicitation_id`. The
/// `ElicitDispatcher` calls this before broadcasting the
/// `ElicitationRequired` event and then awaits `rx.recv()` (composed
/// in a `tokio::select!` with cancel + timeout). Returns the receiver
/// the dispatcher awaits on.
pub fn set_pending_elicitation(
    run_id: Uuid,
    elicitation_id: Uuid,
) -> Result<oneshot::Receiver<serde_json::Value>, &'static str> {
    let handle = get(run_id).ok_or("run not active")?;
    let (tx, rx) = oneshot::channel();
    let mut slot = handle
        .pending_elicitation
        .lock()
        .map_err(|_| "elicit slot poisoned")?;
    *slot = Some(PendingElicit { id: elicitation_id, tx });
    Ok(rx)
}

pub fn clear_pending_elicitation(run_id: Uuid) {
    if let Some(handle) = get(run_id)
        && let Ok(mut slot) = handle.pending_elicitation.lock()
    {
        *slot = None;
    }
}

/// Deliver a response to a pending elicitation. Returns one of:
/// - `Ok(())` — delivered to the waiting dispatcher
/// - `Err("stale")` — the elicitation_id doesn't match the pending one
///   (replay / late submission)
/// - `Err("none")` — there's no pending elicitation for this run
/// - `Err("run not active")` — the runner has already exited
pub fn submit_elicitation_response(
    run_id: Uuid,
    elicitation_id: Uuid,
    value: serde_json::Value,
) -> Result<(), &'static str> {
    let handle = get(run_id).ok_or("run not active")?;
    let mut slot = handle
        .pending_elicitation
        .lock()
        .map_err(|_| "elicit slot poisoned")?;
    let Some(pending) = slot.take() else {
        return Err("none");
    };
    if pending.id != elicitation_id {
        // restore the slot (the in-flight one is still pending)
        *slot = Some(pending);
        return Err("stale");
    }
    pending
        .tx
        .send(value)
        .map_err(|_| "dispatcher receiver dropped")
}

// ============================================================
// Maintenance — periodic reap of orphaned handles
// ============================================================

/// Grace buffer added to a run's own effective timeout before its handle is
/// considered orphaned (the runner panicked without removing itself). Replaces
/// the old fixed 45-min TTL, which would wrongly reap a long / unbounded run's
/// still-live handle (breaking `set_timeout`/`cancel`/SSE for it).
pub const HANDLE_TTL_BUFFER: Duration = Duration::from_secs(15 * 60);

/// Walk the registry, drop any handle whose age exceeds its OWN effective
/// timeout + a grace buffer (the runner forgot to remove itself — likely
/// panicked). An UNBOUNDED run (`timeout_secs == 0`) is never reaped this way —
/// only the startup sweep / explicit unregister clears it. Cheap; called from
/// the per-run SSE subscribe path opportunistically.
pub fn reap_stale() {
    let now = Instant::now();
    let stale: Vec<Uuid> = RUN_HANDLES
        .iter()
        .filter(|e| {
            let h = e.value();
            // Clamp to the engine ceiling (belt-and-suspenders, matching
            // `await_terminal`) so a writer that skipped the clamp can't park a
            // handle here for an absurd duration. 0 = unbounded → never reaped.
            let secs = h
                .timeout_secs
                .load(std::sync::atomic::Ordering::Relaxed)
                .min(crate::modules::workflow::runner::MAX_RUN_TIMEOUT_SECS);
            secs != 0
                && now.duration_since(h.created_at)
                    > Duration::from_secs(secs) + HANDLE_TTL_BUFFER
        })
        .map(|e| *e.key())
        .collect();
    for id in stale {
        RUN_HANDLES.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_signal_wakes_waiter() {
        let run_id = Uuid::new_v4();
        let handle = register(run_id);
        let waiter = tokio::spawn({
            let h = handle.clone();
            async move {
                h.await_cancel().await;
                42
            }
        });
        // The waiter is parked; cancel should wake it.
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(cancel(run_id));
        let v = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter never woke")
            .unwrap();
        assert_eq!(v, 42);
        unregister(run_id);
    }

    #[tokio::test]
    async fn cancel_is_idempotent_after_runner_exits() {
        let run_id = Uuid::new_v4();
        register(run_id);
        assert!(cancel(run_id));
        // Runner removes itself.
        unregister(run_id);
        // Subsequent cancel is a no-op (handle gone).
        assert!(!cancel(run_id));
    }

    #[tokio::test]
    async fn cancel_after_signal_does_not_block_new_waiters() {
        let run_id = Uuid::new_v4();
        let handle = register(run_id);
        assert!(cancel(run_id));
        // New waiter must NOT block; cancelled flag is sticky.
        tokio::time::timeout(Duration::from_millis(100), handle.await_cancel())
            .await
            .expect("await_cancel hung after signal");
        unregister(run_id);
    }

    #[tokio::test]
    async fn elicitation_response_round_trips() {
        let run_id = Uuid::new_v4();
        register(run_id);
        let eid = Uuid::new_v4();
        let rx = set_pending_elicitation(run_id, eid).unwrap();
        let val = serde_json::json!({"answer": 42});
        let send_val = val.clone();
        let waiter = tokio::spawn(async move { rx.await.unwrap() });
        tokio::time::sleep(Duration::from_millis(10)).await;
        submit_elicitation_response(run_id, eid, send_val).unwrap();
        let got = waiter.await.unwrap();
        assert_eq!(got, val);
        unregister(run_id);
    }

    #[tokio::test]
    async fn elicitation_stale_id_rejected() {
        let run_id = Uuid::new_v4();
        register(run_id);
        let eid = Uuid::new_v4();
        let _rx = set_pending_elicitation(run_id, eid).unwrap();
        let wrong = Uuid::new_v4();
        let err = submit_elicitation_response(run_id, wrong, serde_json::json!(1)).unwrap_err();
        assert_eq!(err, "stale");
        // Original is still pending.
        submit_elicitation_response(run_id, eid, serde_json::json!(2)).unwrap();
        unregister(run_id);
    }

    #[test]
    fn set_timeout_updates_the_live_value_and_reports_missing() {
        let run_id = Uuid::new_v4();
        let h = register(run_id);
        assert_eq!(
            h.timeout_secs.load(std::sync::atomic::Ordering::Relaxed),
            DEFAULT_RUN_TIMEOUT_SECS
        );
        assert!(set_timeout(run_id, 0)); // unbounded
        assert_eq!(h.timeout_secs.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert!(set_timeout(run_id, 7200));
        assert_eq!(h.timeout_secs.load(std::sync::atomic::Ordering::Relaxed), 7200);
        unregister(run_id);
        // Gone handle → false (terminal run).
        assert!(!set_timeout(run_id, 60));
    }

    #[test]
    fn reap_stale_never_reaps_an_unbounded_handle() {
        let run_id = Uuid::new_v4();
        let h = register(run_id);
        // Mark unbounded; even an "old" handle must survive the reaper.
        h.timeout_secs.store(0, std::sync::atomic::Ordering::Relaxed);
        reap_stale();
        assert!(get(run_id).is_some(), "unbounded handle must not be reaped");
        unregister(run_id);
    }

    // TEST-49 (ITEM-14/15, LOCK-2): registering a `JobKind` is ADDITIVE — a kind
    // is discovered by walking the decentralized policy registry, never a central
    // `match` — so the backbone extends without editing a dispatch site. (The
    // full policy-shape assertions live alongside the registry in `job_kind.rs`;
    // here we pin the additive-lookup property from the RunHandle-registry side.)
    #[test]
    fn job_kind_registry_is_additive_no_central_match() {
        use crate::modules::workflow::job_kind::{JOB_KIND_POLICIES, policy_for};
        // Every shipped kind resolves via the slice lookup (no hardcoded arm).
        assert!(JOB_KIND_POLICIES.len() >= 3);
        for k in ["workflow", "sandbox_exec", "subagent"] {
            assert!(policy_for(k).is_some(), "kind '{k}' resolves via the registry");
        }
        // An unregistered kind is None (a new kind plugs in by REGISTERING, not
        // by editing this lookup).
        assert!(policy_for("not_a_registered_kind").is_none());
    }

    // TEST-49 (LOCK-2 invariant): there is exactly ONE durable background-run
    // substrate — `workflow_runs`. NO separate `background_jobs` table exists.
    // DB-gated: soft-skips without `DATABASE_URL` (mirrors the suite's env-gated
    // real-stack tests); runs for real against the migrated DB.
    #[tokio::test]
    async fn no_background_jobs_table_exists() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — cannot check the schema");
                return;
            }
        };
        let pool = match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_name = 'background_jobs'",
        )
        .fetch_one(&pool)
        .await
        .expect("query information_schema");
        assert_eq!(
            count, 0,
            "LOCK-2: no separate background_jobs table — workflow_runs is the sole substrate"
        );
    }
}
