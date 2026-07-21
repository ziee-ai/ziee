//! Self-paced next-fire proposal registry (Group E / ITEM-21 / DEC-42).
//!
//! The model-facing `schedule_next` core tool lives in `agent-core`; it records
//! a [`agent_core::ScheduleProposal`] through the crate's
//! [`SchedulePort`](agent_core::SchedulePort). This module is the SERVER side of
//! that seam — the "read the proposal off the turn" wiring DEC-42 left as the
//! last remaining step.
//!
//! It is a **process-wide, in-memory keyed registry**: the chat agent-host
//! dispatcher wires [`AgentSchedulePort`] into `AgentCore` for an unattended
//! (scheduled) prompt run, so when the self-paced agent calls `schedule_next`
//! the proposal is stored here keyed by the turn's `run_id` (chat's
//! `assistant_message_id`). The scheduler's [`dispatch::dispatch_prompt`] then
//! DRAINS it by that same id after the turn and feeds the model's proposal to
//! the EXISTING clamp + write-back (`schedule::next_self_paced_fire` →
//! `repository::arm_self_paced`) instead of the default cadence.
//!
//! In-memory (not DB) is deliberate: a proposal is ephemeral — valid only for the
//! single firing that produced it, drained in the same process before the write-
//! back — and if the process dies mid-turn the whole firing is lost anyway.
//! `dispatch_prompt` drains on every prompt run (self-paced or not), so a
//! proposal recorded on a non-self-paced run is drained + ignored rather than
//! leaked (the write-back only consults it for `self_paced` tasks).
//!
//! [`dispatch::dispatch_prompt`]: super::dispatch
//! [`AgentCore`]: agent_core::AgentCore

use std::sync::Arc;

use agent_core::{ScheduleProposal, SchedulePort};
use async_trait::async_trait;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use uuid::Uuid;
use ziee_core::AppError;

use super::schedule::SelfPacedProposal;

/// Process-wide store: `run_id` (assistant_message_id) → the last proposal the
/// self-paced agent recorded this turn. Last-write-wins within a turn.
static PROPOSALS: Lazy<DashMap<Uuid, ScheduleProposal>> = Lazy::new(DashMap::new);

/// A sane upper bound so a pathological leak (a proposal recorded on a run whose
/// `dispatch_prompt` errored before draining) can't grow the map without limit.
/// In normal operation the map holds at most a handful of in-flight turns.
const MAX_PENDING: usize = 4096;

/// The [`SchedulePort`] impl the chat agent-host wires for an unattended run.
/// Records into the process-wide registry; the scheduler drains it post-turn.
pub struct AgentSchedulePort;

#[async_trait]
impl SchedulePort for AgentSchedulePort {
    async fn propose_next(
        &self,
        run_id: Uuid,
        proposal: ScheduleProposal,
    ) -> Result<(), AppError> {
        if PROPOSALS.len() >= MAX_PENDING && !PROPOSALS.contains_key(&run_id) {
            // Defensive: drop the map rather than grow unbounded. The only path
            // here is a run that recorded but never drained (a rare mid-turn
            // error), so evicting stale entries is safe — a live turn re-records.
            PROPOSALS.clear();
        }
        PROPOSALS.insert(run_id, proposal);
        Ok(())
    }
}

/// A convenient `Arc<dyn SchedulePort>` for the dispatcher's `AgentCore.schedule`.
pub fn schedule_port() -> Arc<dyn SchedulePort> {
    Arc::new(AgentSchedulePort)
}

/// Drain (take + remove) the proposal a self-paced turn recorded for `run_id`,
/// converting it to the scheduler's [`SelfPacedProposal`] for the write-back.
/// `None` when the model never called `schedule_next` (⇒ the caller keeps the
/// existing default-cadence behavior). The free-text `reason` is not part of the
/// clamp, so it's dropped here (DEC-43 surfacing is a separate concern).
pub fn take_proposal(run_id: Uuid) -> Option<SelfPacedProposal> {
    PROPOSALS.remove(&run_id).map(|(_, p)| SelfPacedProposal {
        // `None` ⇒ 0 ("as soon as allowed"); the write-back floors it to the
        // admin `min_interval_seconds`. A u64 that overflows i64 is saturated —
        // the clamp caps it at the max-horizon ceiling regardless.
        delay_seconds: p
            .delay_seconds
            .map(|s| i64::try_from(s).unwrap_or(i64::MAX))
            .unwrap_or(0),
        stop: p.stop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_then_drains_once_clamped_by_writeback_caller() {
        let run = Uuid::new_v4();
        // Absent → None (default-cadence path unchanged).
        assert!(take_proposal(run).is_none());

        // Record a delay proposal, then drain it → SelfPacedProposal.
        AgentSchedulePort
            .propose_next(
                run,
                ScheduleProposal { delay_seconds: Some(3600), reason: Some("wait".into()), stop: false },
            )
            .await
            .unwrap();
        let got = take_proposal(run).expect("proposal drained");
        assert_eq!(got, SelfPacedProposal { delay_seconds: 3600, stop: false });
        // Drained exactly once — a second read is None (no leak / no double-arm).
        assert!(take_proposal(run).is_none());
    }

    #[tokio::test]
    async fn stop_and_bare_proposals_convert() {
        let run = Uuid::new_v4();
        AgentSchedulePort
            .propose_next(run, ScheduleProposal { delay_seconds: None, reason: None, stop: true })
            .await
            .unwrap();
        let got = take_proposal(run).unwrap();
        assert!(got.stop, "stop carried through");
        assert_eq!(got.delay_seconds, 0, "no delay ⇒ 0 (floored to min-interval by the clamp)");

        // Bare proposal (run again as soon as allowed).
        let run2 = Uuid::new_v4();
        AgentSchedulePort
            .propose_next(run2, ScheduleProposal { delay_seconds: None, reason: None, stop: false })
            .await
            .unwrap();
        assert_eq!(
            take_proposal(run2).unwrap(),
            SelfPacedProposal { delay_seconds: 0, stop: false }
        );
    }

    #[tokio::test]
    async fn last_write_wins_within_a_turn() {
        let run = Uuid::new_v4();
        AgentSchedulePort
            .propose_next(run, ScheduleProposal { delay_seconds: Some(60), reason: None, stop: false })
            .await
            .unwrap();
        AgentSchedulePort
            .propose_next(run, ScheduleProposal { delay_seconds: Some(120), reason: None, stop: false })
            .await
            .unwrap();
        assert_eq!(
            take_proposal(run).unwrap(),
            SelfPacedProposal { delay_seconds: 120, stop: false }
        );
    }
}
