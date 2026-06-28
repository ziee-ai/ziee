-- Defense-in-depth: enforce one row per (message_id, sequence_order).
--
-- Ordering within a message is assigned by the application (often as
-- `MAX(sequence_order) + 1` computed inside the INSERT). Two concurrent
-- inserts for the same message can therefore compute the same next value and
-- BOTH succeed, silently corrupting the rendered content order. There was no
-- UNIQUE constraint to catch this — the app was the sole guard. This migration
-- adds the missing constraint so a collision fails loudly (the caller can
-- retry) instead of duplicating a slot.
--
-- A pre-existing duplicate would make the ALTER fail, so first re-sequence any
-- colliding rows deterministically. row_number() over (message_id ORDER BY the
-- existing sequence_order, then created_at, then id) compacts each message's
-- contents to a contiguous 0..n-1 range while PRESERVING their relative order,
-- so display output is unchanged; only the stored integers (and only where a
-- collision existed) move. Idempotent: messages with no duplicates are left
-- untouched by the `WHERE sequence_order <> new_order` guard.

WITH ranked AS (
    SELECT
        id,
        (row_number() OVER (
            PARTITION BY message_id
            ORDER BY sequence_order, created_at, id
        ))::int - 1 AS new_order
    FROM message_contents
)
UPDATE message_contents mc
SET sequence_order = ranked.new_order
FROM ranked
WHERE mc.id = ranked.id
  AND mc.sequence_order <> ranked.new_order;

ALTER TABLE message_contents
    ADD CONSTRAINT uq_message_contents_message_sequence
    UNIQUE (message_id, sequence_order);

-- The UNIQUE constraint above creates a backing unique index on exactly
-- (message_id, sequence_order), so the old non-unique index from migration 12
-- is now redundant.
DROP INDEX IF EXISTS idx_message_contents_sequence;

