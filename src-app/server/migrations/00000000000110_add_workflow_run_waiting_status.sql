-- Durable workflow resume.
--
-- 1) Add the non-terminal `waiting` run status. A run parked on an `elicit`
--    human gate now sits in `waiting` (instead of `running`) so the boot sweep
--    can tell "parked, safe to resume" from "was actively computing": only
--    `pending`/`running` orphans are failed at boot; `waiting` runs are spared
--    and resumed lazily when the human submits.
--
-- 2) Add `elicit_response_json`: when a submit arrives for a `waiting` run that
--    has no resident runner (post-restart), the response is persisted here first,
--    then a resume runner is spawned; the resumed elicit step consumes it instead
--    of re-parking. Makes cold-resume submit race-free. Nullable; only ever set
--    on dev/normal runs transiently and cleared once consumed.
--
-- Both changes are additive: existing rows and the original five statuses are
-- unchanged.

ALTER TABLE workflow_runs DROP CONSTRAINT workflow_runs_status_check;
ALTER TABLE workflow_runs ADD CONSTRAINT workflow_runs_status_check
    CHECK (status IN ('pending', 'running', 'waiting', 'completed', 'failed', 'cancelled'));

ALTER TABLE workflow_runs ADD COLUMN elicit_response_json JSONB;
