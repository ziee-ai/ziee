-- Drop the redundant index on user_mcp_defaults(user_id). The table's
-- `unique_user_mcp_defaults UNIQUE(user_id)` constraint (migration 18) already
-- backs that column with a unique index, so the explicit
-- `idx_user_mcp_defaults_user_id` is duplicate work on every write.
DROP INDEX IF EXISTS idx_user_mcp_defaults_user_id;
