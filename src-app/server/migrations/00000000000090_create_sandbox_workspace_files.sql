-- Provenance map linking a per-conversation sandbox workspace path to the
-- `files` row it represents, so that when an editable file is modified inside
-- the sandbox the change is committed as a NEW VERSION of that file (not an
-- orphan file). `base_version_id` is the version the workspace copy was last
-- seeded from / committed as; end-of-turn version-back checksum-diffs against
-- it and appends a new version when the workspace bytes differ.

CREATE TABLE sandbox_workspace_files (
    conversation_id   UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    workspace_relpath TEXT NOT NULL,
    file_id           UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    -- CASCADE so deleting a file (which cascade-deletes its file_versions) can't
    -- be blocked by a dangling base_version_id ref. The file_id CASCADE above
    -- already removes this row in the same statement, but the explicit CASCADE
    -- here makes the intent unambiguous and order-independent.
    base_version_id   UUID NOT NULL REFERENCES file_versions(id) ON DELETE CASCADE,
    PRIMARY KEY (conversation_id, workspace_relpath)
);

CREATE INDEX idx_sandbox_workspace_files_file ON sandbox_workspace_files(file_id);
