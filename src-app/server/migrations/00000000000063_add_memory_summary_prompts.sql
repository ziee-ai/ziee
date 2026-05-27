-- Promote the summarizer prompts from compiled-in constants to
-- nullable admin-overridable columns. NULL = "use the binary's
-- compiled-in default"; a non-NULL value overrides the default
-- without a redeploy.
--
-- Why nullable instead of NOT NULL with the prompt text in the
-- DEFAULT: prompt strings are long and likely to be tuned across
-- releases. Baking them into a migration means re-migrating when
-- defaults change. NULL-as-fallback keeps the defaults in Rust
-- (versioned with the rest of the prompt-engineering code) while
-- still letting operators override per-deployment.
--
-- Validation happens at the handler level (must contain the
-- {transcript} or {previous_summary}/{new_transcript} placeholders)
-- so the admin gets a clear 400 instead of silently broken
-- summaries.

ALTER TABLE memory_admin_settings
    ADD COLUMN full_summary_prompt TEXT,
    ADD COLUMN incremental_summary_prompt TEXT;
