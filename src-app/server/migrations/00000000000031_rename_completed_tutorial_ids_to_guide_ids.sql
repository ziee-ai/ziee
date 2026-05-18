ALTER TABLE users DROP COLUMN completed_tutorial_ids;
ALTER TABLE users ADD COLUMN completed_guide_ids TEXT[] NOT NULL DEFAULT '{}';
