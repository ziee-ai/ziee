-- Memory audit log — plan §11 PII mitigation.
--
-- Every ADD / UPDATE / DELETE of `user_memories` writes a row here so
-- users (and admins on demand) can audit what was captured, when, and
-- by which source. Append-only; no UPDATE / DELETE on this table.
CREATE TABLE memory_audit_log (
    id         BIGSERIAL PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    memory_id  UUID,
    op         TEXT NOT NULL CHECK (op IN ('ADD', 'UPDATE', 'DELETE', 'BULK_DELETE')),
    source     TEXT NOT NULL CHECK (source IN ('extraction', 'mcp_tool', 'manual', 'admin')),
    content_snapshot TEXT,
    actor_kind TEXT NOT NULL DEFAULT 'user' CHECK (actor_kind IN ('user', 'assistant', 'admin', 'system')),
    metadata   JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_memory_audit_log_user_created
    ON memory_audit_log(user_id, created_at DESC);
CREATE INDEX idx_memory_audit_log_memory
    ON memory_audit_log(memory_id) WHERE memory_id IS NOT NULL;
