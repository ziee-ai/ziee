-- Move per-user onboarding progress off the `users` table into a dedicated
-- table so the auth/user module no longer carries onboarding concerns.
-- The onboarding module owns this table (read via GET /onboarding/progress,
-- written by complete_guide / complete_guide_step). Step ids keep the
-- "{guide_id}/{step_id}" composite-key format.

CREATE TABLE user_onboarding (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    completed_guide_ids TEXT[] NOT NULL DEFAULT '{}',
    completed_step_ids TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Backfill BEFORE dropping the columns (same migration runs in one tx).
-- Only users with existing progress get a row; everyone else is lazily
-- created on their first completion.
INSERT INTO user_onboarding (user_id, completed_guide_ids, completed_step_ids)
SELECT id, completed_onboarding_ids, completed_onboarding_step_ids
FROM users
WHERE cardinality(completed_onboarding_ids) > 0
   OR cardinality(completed_onboarding_step_ids) > 0;

ALTER TABLE users
    DROP COLUMN completed_onboarding_ids,
    DROP COLUMN completed_onboarding_step_ids;
