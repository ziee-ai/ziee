-- Prevent duplicate concurrent downloads of the same repo file. The handler's
-- find-existing-then-create check has a TOCTOU window: two concurrent identical
-- requests both find nothing and both insert. This partial unique index makes
-- the second insert fail at the DB level; the handler catches that and returns
-- the in-flight winner instead.
CREATE UNIQUE INDEX IF NOT EXISTS uq_download_instances_in_progress
    ON download_instances (
        repository_id,
        provider_id,
        (request_data ->> 'repository_path'),
        (request_data ->> 'main_filename')
    )
    WHERE status IN ('pending', 'downloading');
