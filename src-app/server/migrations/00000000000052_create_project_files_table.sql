-- project_files: M:N join between projects and the existing files table.
-- A project's "knowledge" attachments — read by the chat/project extension
-- and prepended to every conversation in the project.
--
-- CASCADE on both sides:
--   * file deletion (user-initiated, global) silently removes membership
--     from every project that referenced it. The user already opted in
--     to global removal by deleting the file.
--   * project deletion removes membership rows but leaves the underlying
--     files (they may be attached to other projects or used per-message
--     in conversations the project owned).
--
-- Duplicate attach is idempotent — composite PK turns repeat INSERTs
-- into a no-op (server returns 200 with the existing row).

CREATE TABLE project_files (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    file_id    UUID NOT NULL REFERENCES files(id)    ON DELETE CASCADE,
    added_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, file_id)
);

-- project_id is the leading column of the PK so no separate index needed
-- there. file_id needs its own index for "delete file → cascade everywhere".
CREATE INDEX idx_project_files_file_id ON project_files(file_id);
