-- Per-user onboarding progress tracking: which guides and which guide-steps
-- a user has completed. Consumed by the onboarding_screen module
-- (complete_guide / complete_guide_step) and surfaced to the frontend wizard.
ALTER TABLE users ADD COLUMN completed_onboarding_ids TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE users ADD COLUMN completed_onboarding_step_ids TEXT[] NOT NULL DEFAULT '{}';
