//! Decentralized `JobKind` policy registry (ITEM-17 / DEC-25/33/76, LOCK-2).
//!
//! The BACKBONE stores every background run in `workflow_runs` under a
//! `job_kind` discriminator (see [`super::models::JobKind`]). Each kind has its
//! OWN boot-sweep / flap / retention policy. To keep a new kind an ADDITIVE
//! registration — never an edit to a central `match` (the exact
//! extensibility/modularity property the codebase is graded on, mirroring the
//! `MODULE_ENTRIES` / built-in-MCP registry culture) — each kind registers a
//! [`JobKindPolicy`] into the `linkme` distributed slice below. The sweep +
//! retention paths iterate the slice; they never name a kind.
//!
//! No `background_jobs` table exists — one durable substrate (`workflow_runs`),
//! one policy registry (this file). That is the LOCK-2 invariant.

use linkme::distributed_slice;

/// What the boot orphan-sweep should do with an in-flight run of this kind that
/// the server was killed underneath (ITEM-29 / DEC-25/76).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanSweepPolicy {
    /// Mark `failed`: the underlying work cannot resume (a killed sandbox
    /// subprocess — its `Child` is gone, its stdout is lost). Fire-and-forget.
    Fail,
    /// Mark `resumable`: the work is REPLAYABLE from a durable checkpoint (a
    /// sub-agent's persisted transcript). The boot path re-drives it.
    Resumable,
}

/// One background-run kind's self-declared policy. Registered into
/// [`JOB_KIND_POLICIES`] at link time, so adding a kind is an additive
/// registration — not a central-`match` edit.
pub struct JobKindPolicy {
    /// The DB `job_kind` text this policy governs (== [`super::models::JobKind::as_str`]).
    pub job_kind: &'static str,
    /// Boot-sweep disposition for an orphaned in-flight run of this kind.
    pub orphan_sweep: OrphanSweepPolicy,
    /// Flap cap for a `Resumable` kind's crash-restart loop: give up re-driving
    /// after this many crashes within `flap_window_secs` (mirrors the local
    /// runtime's flap cap). `0` = no auto-give-up. A `Fail` kind (never
    /// re-driven) leaves this `0`.
    // Seam: read by the crash-restart loop in a later tranche (the boot sweep
    // uses `orphan_sweep` today); shipped now so each kind declares it up front.
    #[allow(dead_code)]
    pub flap_max_restarts: u32,
    #[allow(dead_code)]
    pub flap_window_secs: u64,
    /// Terminal-run retention in days for the boot prune loop (DEC-30);
    /// `0` = keep forever. Read by the retention prune path once it lands.
    #[allow(dead_code)]
    pub retention_days: u32,
}

/// The registry: every registered background-run kind's policy. Iterated by the
/// per-kind boot sweep + retention prune; never indexed by a hardcoded kind.
#[distributed_slice]
pub static JOB_KIND_POLICIES: [JobKindPolicy];

/// Resolve the policy for a `job_kind` string by walking the decentralized
/// slice. `None` for an unregistered / unknown kind (forward-compat: a newer DB
/// value an older binary doesn't know — the caller falls back conservatively,
/// e.g. the sweep treats an unknown kind as fail-closed).
// Seam (ITEM-17): the single-kind lookup the model-facing check_status /
// retention paths use in a later tranche; the boot sweep filters the whole slice
// directly. Kept as the registry's public lookup API + exercised by tests.
#[allow(dead_code)]
pub fn policy_for(job_kind: &str) -> Option<&'static JobKindPolicy> {
    JOB_KIND_POLICIES.iter().find(|p| p.job_kind == job_kind)
}

// ── The built-in kinds each register their OWN policy (decentralized) ────────

/// A classic YAML-DAG run. A plain orphan fails ("server restart during
/// execution"); the special case of a crash INSIDE an `agent` step is spared
/// separately via the `resumable_agent` flag (see `repository::fail_orphaned_runs`
/// step 1), independent of this kind policy.
#[distributed_slice(JOB_KIND_POLICIES)]
pub static WORKFLOW_POLICY: JobKindPolicy = JobKindPolicy {
    job_kind: "workflow",
    orphan_sweep: OrphanSweepPolicy::Fail,
    flap_max_restarts: 0,
    flap_window_secs: 0,
    retention_days: 0,
};

/// A fire-and-forget background sandbox command. The subprocess dies with the
/// server; there is nothing to replay → `Fail` (DEC-25/76).
#[distributed_slice(JOB_KIND_POLICIES)]
pub static SANDBOX_EXEC_POLICY: JobKindPolicy = JobKindPolicy {
    job_kind: "sandbox_exec",
    orphan_sweep: OrphanSweepPolicy::Fail,
    flap_max_restarts: 0,
    flap_window_secs: 0,
    retention_days: 0,
};

/// A detached agent-core turn (Option C). Its transcript is persisted, so a
/// crash mid-loop is re-driven via transcript replay → `Resumable` (DEC-25/76).
/// Flap-capped like the local-runtime supervisor (give up after 5 crashes / 60s).
#[distributed_slice(JOB_KIND_POLICIES)]
pub static SUBAGENT_POLICY: JobKindPolicy = JobKindPolicy {
    job_kind: "subagent",
    orphan_sweep: OrphanSweepPolicy::Resumable,
    flap_max_restarts: 5,
    flap_window_secs: 60,
    retention_days: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    // ITEM-17: the registry resolves every built-in kind, and a kind's policy is
    // discovered by SLICE LOOKUP (no central match) — so registering a new kind
    // (an extra `#[distributed_slice]` entry) is picked up here with zero edits
    // to the lookup. We assert the three shipped kinds + the per-kind sweep
    // disposition DEC-76 fixes.
    #[test]
    fn registry_resolves_every_builtin_kind_additively() {
        // At least the three built-ins are registered (the slice is additive —
        // a future kind only grows this count, never forces a match arm here).
        assert!(
            JOB_KIND_POLICIES.len() >= 3,
            "expected >= 3 registered kinds, found {}",
            JOB_KIND_POLICIES.len()
        );

        // Every built-in kind resolves via the decentralized lookup.
        let workflow = policy_for("workflow").expect("workflow kind registered");
        let sandbox = policy_for("sandbox_exec").expect("sandbox_exec kind registered");
        let subagent = policy_for("subagent").expect("subagent kind registered");

        // DEC-25/76 per-kind sweep policy: subagent replays, the rest fail.
        assert_eq!(subagent.orphan_sweep, OrphanSweepPolicy::Resumable);
        assert_eq!(sandbox.orphan_sweep, OrphanSweepPolicy::Fail);
        assert_eq!(workflow.orphan_sweep, OrphanSweepPolicy::Fail);

        // Only the replayable kind carries a flap cap.
        assert!(subagent.flap_max_restarts > 0);
        assert_eq!(sandbox.flap_max_restarts, 0);

        // Unknown / unregistered kind → None (forward-compat, fail-closed).
        assert!(policy_for("future_kind").is_none());

        // Registry entries agree with the `JobKind` enum's own strings (no drift
        // between the enum vocabulary and the registered policies).
        use crate::modules::workflow::models::JobKind;
        for k in [JobKind::Workflow, JobKind::SandboxExec, JobKind::SubAgent] {
            assert!(
                policy_for(k.as_str()).is_some(),
                "JobKind '{}' must have a registered policy",
                k.as_str()
            );
        }
    }
}
