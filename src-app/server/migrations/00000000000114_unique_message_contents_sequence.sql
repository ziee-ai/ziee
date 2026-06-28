-- Enforce uniqueness of (message_id, sequence_order) on message_contents.
--
-- Ordering within a message is assigned by the application; without a DB
-- constraint a bug (or a concurrent insert) could write two rows with the same
-- sequence_order, silently corrupting content order. This adds defense in depth.
--
-- Existing data may already contain collisions (the app never guaranteed
-- uniqueness), so we first repair any colliding message by re-sequencing its
-- rows contiguously from 0 while preserving their relative order. Messages with
-- no collision are left untouched.
WITH ranked AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY message_id
            ORDER BY sequence_order, created_at, id
        ) - 1 AS new_order
    FROM message_contents
    WHERE message_id IN (
        SELECT message_id
        FROM message_contents
        GROUP BY message_id, sequence_order
        HAVING COUNT(*) > 1
    )
)
UPDATE message_contents mc
SET sequence_order = ranked.new_order
FROM ranked
WHERE mc.id = ranked.id
  AND mc.sequence_order <> ranked.new_order;

-- The old non-unique index covers the same columns; replace it with a UNIQUE
-- index (which still serves ordered lookups by message_id).
DROP INDEX IF EXISTS idx_message_contents_sequence;

CREATE UNIQUE INDEX idx_message_contents_message_seq_unique
    ON message_contents(message_id, sequence_order);
