-- Partial unique index on `hub_entities` to backstop the application-level
-- duplicate-prevention guard in `Hub.createAssistantTemplateFromHub`.
--
-- Without this, two admins clicking "Use as Template" near-simultaneously
-- can both observe `find_template_install("foo") = None` outside any
-- transaction, both proceed to `Repos.assistant.create` + `track_hub_entity`,
-- and end up with two `is_template=true, created_by=NULL` rows for the
-- same `hub_id`. Both would then fan out to every subsequent signup via
-- `CloneTemplateAssistantsHandler`, doubling the per-user assistant cost.
--
-- Partial because the uniqueness invariant only holds for the
-- template-install slice of the table: user assistants legitimately can
-- collide on `hub_id` (one per user), and hub MCP servers / models
-- don't share this constraint.
--
-- The handler translates the resulting SQLSTATE 23505 into a 409 to match
-- the fast-path error code clients see when the guard wins the race.

CREATE UNIQUE INDEX IF NOT EXISTS uniq_hub_template_install
    ON hub_entities (hub_id)
    WHERE entity_type = 'assistant' AND created_by IS NULL;
