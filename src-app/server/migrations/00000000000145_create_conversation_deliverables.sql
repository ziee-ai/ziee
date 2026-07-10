-- conversation_deliverables: user curation of a conversation's "deliverables".
--
-- The deliverables list is DERIVED by default (files the model authored in the
-- conversation: file_versions.source_message_id in the conversation AND
-- files.created_by IN ('mcp','llm')). This table lets the user CURATE that list:
--   * pinned = true  → promote a file into the list (e.g. a plain upload the
--                       user wants treated as a deliverable),
--   * pinned = false → hide a derived file from the list.
--
-- Mirrors project_files. CASCADE on both sides: deleting the conversation or the
-- underlying file drops the curation row (the file itself is owned globally and
-- survives conversation deletion, exactly like project_files).

CREATE TABLE conversation_deliverables (
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    file_id         UUID NOT NULL REFERENCES files(id)         ON DELETE CASCADE,
    pinned          BOOLEAN NOT NULL DEFAULT true,
    title           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (conversation_id, file_id)
);

-- conversation_id leads the PK (no separate index needed). file_id needs its own
-- index so "delete file → cascade everywhere" doesn't table-scan.
CREATE INDEX idx_conversation_deliverables_file_id ON conversation_deliverables(file_id);
