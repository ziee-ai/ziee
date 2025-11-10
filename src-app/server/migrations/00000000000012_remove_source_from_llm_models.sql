-- Remove source column from llm_models table
ALTER TABLE llm_models DROP COLUMN IF EXISTS source;

-- Remove the check constraint if it exists
ALTER TABLE llm_models DROP CONSTRAINT IF EXISTS check_source_structure;
