-- Track which catalog version each hub-installed entity came from.
-- Lets /api/hub/updates compute "X updates available" by comparing
-- against the current catalog's hub_version. NULL for rows that
-- predate this migration; treated as "unknown, assume out of date".
ALTER TABLE hub_entities ADD COLUMN hub_version VARCHAR(32);

COMMENT ON COLUMN hub_entities.hub_version IS
    'Hub catalog version (semver) the entity was installed from. NULL for legacy rows.';
