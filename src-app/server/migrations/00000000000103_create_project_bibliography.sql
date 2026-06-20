-- project_bibliography: M:N join between projects and bibliography_entries.
-- A project's reference list is just the entries linked to it — the citation
-- data lives once in bibliography_entries (one library; project lists are
-- subsets, link-not-copy). Mirrors project_files exactly.
--
-- CASCADE on both sides:
--   * deleting a bibliography entry (global, user-initiated) removes its
--     membership from every project that referenced it.
--   * deleting a project removes membership rows but leaves the entries
--     (they may belong to other projects / the global library).
--
-- "Remove from project" deletes only the link (unlink); "Delete" removes the
-- bibliography_entries row (and cascades these links). Composite PK makes
-- repeat attach idempotent.

CREATE TABLE project_bibliography (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entry_id   UUID NOT NULL REFERENCES bibliography_entries(id) ON DELETE CASCADE,
    added_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, entry_id)
);

-- project_id leads the PK; entry_id needs its own index for
-- "delete entry → cascade everywhere".
CREATE INDEX idx_project_bibliography_entry_id ON project_bibliography(entry_id);
