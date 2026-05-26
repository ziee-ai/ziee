-- Add optional project_id FK to conversations so a conversation can belong
-- to a project. Nullable: existing conversations + future "no-project"
-- conversations stay at NULL.
--
-- ON DELETE SET NULL = "deleting a project leaves its conversations intact
-- but they will no longer receive project knowledge or instructions"
-- (the rule documented in the project delete-confirmation UX copy).

ALTER TABLE conversations
    ADD COLUMN project_id UUID REFERENCES projects(id) ON DELETE SET NULL;

CREATE INDEX idx_conversations_project_id ON conversations(project_id);
