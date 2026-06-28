-- Completes the retriever SCOPE_FILTER index set. Migration 87 added partial
-- indexes for the scope='project' and scope='conversation' arms; the
-- scope='user' arm (user_id = $1 AND scope = 'user' AND deleted_at IS NULL),
-- the most common recall predicate, had no targeted index.
CREATE INDEX IF NOT EXISTS idx_user_memories_scope_user
    ON user_memories (user_id)
    WHERE scope = 'user' AND deleted_at IS NULL;
