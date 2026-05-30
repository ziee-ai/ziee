-- P4 (health state machine): persist the per-instance state machine
-- view onto llm_runtime_instances so the UI can render badges and
-- recover state across server restarts.
--
-- The state names mirror llm_runtime::health::InstanceState::name():
-- starting | healthy | unhealthy | crashed | restarting | failed | stopped

ALTER TABLE llm_runtime_instances
    ADD COLUMN state VARCHAR(50) NOT NULL DEFAULT 'starting'
        CHECK (state IN ('starting', 'healthy', 'unhealthy',
                         'crashed', 'restarting', 'failed', 'stopped')),
    ADD COLUMN state_changed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    ADD COLUMN restart_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN last_failure_reason TEXT;

CREATE INDEX idx_llm_runtime_instances_state ON llm_runtime_instances(state);
