-- Promote 4 behavioral knobs from compiled-in constants to admin-tunable
-- columns on memory_admin_settings:
--
--   soft_delete_grace_days       - reaper: hard-delete grace period after
--                                   soft delete (was reaper.rs::SOFT_DELETE_GRACE_DAYS = 30.0)
--   daily_extraction_quota       - extractor: per-user/day extraction cap
--                                   (was extractor.rs::DAILY_EXTRACTION_QUOTA = 200)
--   summarize_after_n_messages   - summarizer: trigger threshold
--                                   (was summarizer.rs::SUMMARIZE_AFTER_N_MESSAGES = 50)
--   summarizer_keep_recent       - summarizer: messages kept verbatim
--                                   (was summarizer.rs::KEEP_RECENT = 10)
--
-- All four take their previous compiled-in values as DEFAULTs so an
-- existing deployment that hasn't touched the admin page behaves
-- identically post-migration. CHECK clauses pin sane lower bounds and,
-- where it makes sense, an upper bound to prevent foot-guns (a
-- 100,000-message summarizer-keep would defeat summarization entirely).

ALTER TABLE memory_admin_settings
    ADD COLUMN soft_delete_grace_days INTEGER NOT NULL DEFAULT 30
        CHECK (soft_delete_grace_days >= 1 AND soft_delete_grace_days <= 365),
    ADD COLUMN daily_extraction_quota INTEGER NOT NULL DEFAULT 200
        CHECK (daily_extraction_quota >= 1 AND daily_extraction_quota <= 10000),
    ADD COLUMN summarize_after_n_messages INTEGER NOT NULL DEFAULT 50
        CHECK (summarize_after_n_messages >= 10 AND summarize_after_n_messages <= 1000),
    ADD COLUMN summarizer_keep_recent INTEGER NOT NULL DEFAULT 10
        CHECK (summarizer_keep_recent >= 2 AND summarizer_keep_recent <= 100);

-- summarizer_keep_recent MUST stay below summarize_after_n_messages,
-- otherwise the summarizer would never have anything to condense (the
-- "older than KEEP_RECENT" window would be empty). A row-level CHECK
-- enforces it.
ALTER TABLE memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_summarizer_window_check
    CHECK (summarizer_keep_recent < summarize_after_n_messages);
