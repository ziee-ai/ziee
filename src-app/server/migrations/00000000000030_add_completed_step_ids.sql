ALTER TABLE users ADD COLUMN completed_step_ids TEXT[] NOT NULL DEFAULT '{}';
