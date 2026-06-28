-- Drop the redundant index on conversation_summaries(branch_id).
--
-- `conversation_summaries.branch_id` is the table's PRIMARY KEY, which already
-- creates a unique B-tree index on `branch_id`. The separately-declared
-- `idx_conversation_summaries_branch` (migration 59) duplicates that index for
-- no benefit — every branch_id lookup is already served by the PK index — so
-- it just costs extra write amplification and storage. Drop it.
DROP INDEX IF EXISTS idx_conversation_summaries_branch;
