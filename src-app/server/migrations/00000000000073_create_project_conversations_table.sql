-- project_conversations: M:1 join between projects and the existing
-- conversations table. Replaces the inline `conversations.project_id`
-- column so chat's schema no longer mentions projects (chat module
-- knows nothing about project — project membership is queried via
-- this table by the projects module).
--
-- A conversation can be in at most ONE project at a time (PRIMARY KEY
-- on conversation_id alone enforces this); a project has many
-- conversations (project_id is just a regular column with an index).
--
-- CASCADE on both sides:
--   * conversation deletion drops the membership row.
--   * project deletion drops the membership row, which leaves the
--     underlying conversation unfiled. Matches the prior behavior of
--     `ON DELETE SET NULL` on the dropped column (deleting a project
--     leaves its conversations intact but unfiled).
--
-- Migrated data: any rows in `conversations` that had a non-NULL
-- `project_id` get a corresponding row inserted here before the
-- column is dropped.

CREATE TABLE project_conversations (
    conversation_id UUID NOT NULL PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    attached_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- "List all conversations in this project" reads by project_id.
CREATE INDEX idx_project_conversations_project_id
    ON project_conversations(project_id);

-- Copy existing membership data.
INSERT INTO project_conversations (conversation_id, project_id, attached_at)
SELECT id, project_id, COALESCE(updated_at, NOW())
FROM conversations
WHERE project_id IS NOT NULL;

-- Drop the legacy column + its index.
DROP INDEX IF EXISTS idx_conversations_project_id;
ALTER TABLE conversations DROP COLUMN project_id;
