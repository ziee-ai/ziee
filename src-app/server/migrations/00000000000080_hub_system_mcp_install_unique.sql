-- Partial unique index on `hub_entities` to backstop the
-- application-level duplicate-prevention guard in
-- `Hub.createSystemMcpServerFromHub` (the system-wide variant of
-- the hub MCP install endpoint).
--
-- Without this, two admins clicking "Install as System" near-
-- simultaneously can both observe `find_system_mcp_install("foo") =
-- None` outside any transaction, both proceed to
-- `Repos.mcp.create_system_server` + `track_hub_entity`, and end up
-- with two `is_system=true, user_id=NULL` rows for the same `hub_id`.
-- Mirrors migration 79 (`uniq_hub_template_install`) one-for-one;
-- only the predicate's `entity_type` differs.
--
-- Partial because the uniqueness invariant only holds for the
-- system-install slice of the table: user MCP installs legitimately
-- collide on `hub_id` (one per user). Hub-installed user assistants /
-- models don't share this constraint either.
--
-- The handler translates the resulting SQLSTATE 23505 into a 409 to
-- match the fast-path error code clients see when the application
-- guard wins the race (the translation is entity-agnostic and
-- already in place in `track_hub_entity` from migration 79's
-- companion fix).

CREATE UNIQUE INDEX IF NOT EXISTS uniq_hub_system_mcp_install
    ON hub_entities (hub_id)
    WHERE entity_type = 'mcp_server' AND created_by IS NULL;
