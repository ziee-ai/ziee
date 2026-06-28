-- Partial index for the default-assistant lookup
-- (`get_default_assistant`: WHERE created_by = $1 AND is_default = true
--  AND enabled = true). The migration-6 single-column indexes cannot serve
-- the 3-predicate filter efficiently; this partial index is tiny (only each
-- user's enabled default row) and answers the lookup directly.
CREATE INDEX IF NOT EXISTS idx_assistants_default_lookup
    ON assistants (created_by)
    WHERE is_default = true AND enabled = true;
