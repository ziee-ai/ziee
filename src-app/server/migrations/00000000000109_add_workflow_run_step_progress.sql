-- Workflow live sandbox-step progress (plan P2.6).
--
-- Holds the CURRENTLY-running sandbox step's coalesced live-progress track map
-- (a `progress.v1` `{ id -> ProgressTrack }` object), upserted on the
-- dispatcher's throttle flush and CLEARED when the step completes / fails /
-- cancels. Kept on the run row (not a separate table) so the per-run SSE
-- `Snapshot` — which is projected entirely from this row — rehydrates in-flight
-- progress bars after a page refresh / reconnect. Only ever one step's tracks at
-- a time, so it stays small. NULL = no in-flight progress.
--
-- NOTE: numbered 109 — origin/main merged the standalone-runs work (PR #110)
-- which took 106/107/108, so this (the second PR) renumbered to the next free
-- slot per the standard rebase flow.
ALTER TABLE workflow_runs
    ADD COLUMN step_progress_json JSONB;
