-- Remove three unused built-in system MCP servers seeded by migration 7:
--   'filesystem' (Filesystem Access), 'browser' (Browser Automation), 'git'.
--
-- They ship DISABLED, are assigned to no group, and no runtime code path
-- resolves them (they are absent from `auto_attach_builtin_ids` and
-- `is_builtin_server_id` in mcp/chat_extension/mcp.rs, and connection-health
-- skips them). They are dead example rows; this deployment does not want them.
--
-- DELIBERATELY KEPT:
--   'fetch' — enabled and assigned to the default group by migration 7.
--   'files' — a DIFFERENT, load-bearing built-in (files.ziee.internal: http
--             loopback, deterministic id, auto-attached to chats). Do not
--             confuse it with the stdio 'filesystem' row deleted here.
--
-- Migration 7 itself is NOT edited: sqlx stores a checksum per applied
-- migration, so changing its INSERT in place would hard-fail the boot of every
-- existing deployment. Deleting here is equivalent on a fresh DB and safe on an
-- existing one.
--
-- `user_group_mcp_servers` rows cascade via ON DELETE CASCADE.

DELETE FROM mcp_servers
WHERE is_system = true
  AND name IN ('filesystem', 'browser', 'git');
