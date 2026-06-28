-- Enforce a unique URL per LLM repository.
--
-- `llm_repositories` already has UNIQUE(name) but not UNIQUE(url). Two
-- repositories pointing at the same URL is a configuration mistake (the URL is
-- the identity of the upstream source: Hugging Face, GitHub, a custom mirror),
-- and an unconstrained url lets concurrent creates / careless edits introduce
-- duplicates that downstream resolution then has to disambiguate arbitrarily.
-- Add the missing UNIQUE constraint so duplicates are rejected at the DB level.
--
-- The two seeded built-in repos (huggingface.co, github.com) have distinct
-- URLs, so this is safe to add on a fresh database.
ALTER TABLE llm_repositories
    ADD CONSTRAINT llm_repositories_url_key UNIQUE (url);
