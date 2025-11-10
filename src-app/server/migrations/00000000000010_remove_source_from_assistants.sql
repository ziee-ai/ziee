-- Remove source column from assistants table
ALTER TABLE assistants DROP COLUMN IF EXISTS source;

-- Remove the check constraint if it exists
ALTER TABLE assistants DROP CONSTRAINT IF EXISTS check_source_structure;
