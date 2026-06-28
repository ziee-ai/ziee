-- Covering index for the daily extraction-quota count
-- (extractor.rs: WHERE user_id = $1 AND source = 'extraction'
--  AND created_at > NOW() - INTERVAL '24 hours'). Partial on extraction rows,
-- ordered by created_at so the per-user 24h window is a cheap range scan.
CREATE INDEX IF NOT EXISTS idx_user_memories_extraction_recent
    ON user_memories (user_id, created_at)
    WHERE source = 'extraction';
