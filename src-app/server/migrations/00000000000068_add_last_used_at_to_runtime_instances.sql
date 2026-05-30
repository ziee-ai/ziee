-- P1.b: track when each running instance was last forwarded a
-- request by the proxy. The reaper uses this column to find idle
-- engines to unload. Defaults to NOW() so existing rows aren't
-- immediately eligible for eviction at first boot post-migration.

ALTER TABLE llm_runtime_instances
    ADD COLUMN last_used_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW();

-- Partial index — we only ever query last_used_at for status='running'.
CREATE INDEX idx_llm_runtime_instances_last_used
    ON llm_runtime_instances(last_used_at)
    WHERE status = 'running';
