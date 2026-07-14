-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- Framework bootstrap: required extensions + the shared updated_at trigger
-- function (used by triggers across many modules). Sorts FIRST.

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;
CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;

CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;
