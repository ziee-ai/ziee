-- Covering index for the per-user daily extraction-quota COUNT in
-- memory::engine::extractor:
--   SELECT COUNT(*) FROM user_memories
--   WHERE user_id = $1 AND source = 'extraction'
--     AND created_at > NOW() - INTERVAL '24 hours'
-- A partial index on the 'extraction' source keeps it small; the
-- (user_id, created_at) key serves the equality + trailing-window range so
-- the quota check (run on every extraction, now also under an advisory lock)
-- is an index scan instead of a per-user heap scan.
CREATE INDEX IF NOT EXISTS idx_user_memories_extraction_quota
    ON user_memories (user_id, created_at)
    WHERE source = 'extraction';
