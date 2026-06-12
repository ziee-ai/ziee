-- File versioning: add a child `file_versions` table holding one immutable row
-- per version. `files` stays the parent (PK = stable file_id, referenced
-- everywhere → zero reference migration).
--
-- Key invariant: v1's version id == file_id, so existing on-disk blobs at
-- originals/{user}/{file_id}.{ext} resolve unchanged (storage is keyed by
-- blob_version_id, and v1.blob_version_id == file_id). Only v2+ introduce new
-- blob paths.
--
-- The per-version columns are LEFT on `files` for now (still populated by the
-- create path) so the codebase keeps compiling; a later migration drops them
-- once every read goes through the head join.

CREATE TABLE file_versions (
    id                  UUID PRIMARY KEY,
    file_id             UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    version             INTEGER NOT NULL,
    is_head             BOOLEAN NOT NULL DEFAULT false,
    -- Which version's blob holds THIS version's bytes. Normally = id; a RESTORE
    -- points it at the restored target so no bytes are duplicated on disk.
    blob_version_id     UUID NOT NULL,
    file_size           BIGINT NOT NULL,
    mime_type           VARCHAR(100),
    checksum            VARCHAR(64),
    has_thumbnail       BOOLEAN NOT NULL DEFAULT false,
    preview_page_count  INTEGER NOT NULL DEFAULT 0,
    text_page_count     INTEGER NOT NULL DEFAULT 0,
    processing_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- The chat turn / tool-call that produced this version (provenance). No FK:
    -- it is opaque provenance metadata and must survive message edits/branch
    -- pruning without cascading.
    source_message_id   UUID,
    created_by          VARCHAR(10) NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (file_id, version)
);

-- Exactly one head per file (the head-pointer invariant, enforced by the DB).
CREATE UNIQUE INDEX uq_file_versions_head ON file_versions(file_id) WHERE is_head;
CREATE INDEX idx_file_versions_file ON file_versions(file_id, version DESC);
CREATE INDEX idx_file_versions_blob ON file_versions(blob_version_id);

-- Head pointer on the parent. The FK is DEFERRABLE INITIALLY DEFERRED so the
-- application's create() can set current_version_id in the SAME INSERT that
-- precedes the v1 file_versions row (the FK is verified at COMMIT, by which
-- point the referenced version exists). This lets us enforce NOT NULL below.
ALTER TABLE files
    ADD COLUMN current_version_id UUID
    REFERENCES file_versions(id) DEFERRABLE INITIALLY DEFERRED;

-- Backfill: one version per existing file, id = file_id, blob_version_id =
-- file_id (preserves the on-disk blob path), copying the per-version columns.
INSERT INTO file_versions (
    id, file_id, version, is_head, blob_version_id,
    file_size, mime_type, checksum, has_thumbnail,
    preview_page_count, text_page_count, processing_metadata,
    source_message_id, created_by, created_at
)
SELECT
    id, id, 1, true, id,
    file_size, mime_type, checksum, has_thumbnail,
    preview_page_count, text_page_count, processing_metadata,
    NULL, created_by, created_at
FROM files;

UPDATE files SET current_version_id = id;

-- Every file now has exactly one head; enforce the invariant at the schema
-- level so a future code path can never silently create a head-less file
-- (which would vanish from the head-join reads).
ALTER TABLE files ALTER COLUMN current_version_id SET NOT NULL;
