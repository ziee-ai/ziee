-- Round 2 (ITEM-40): per-run result preview + change summary powering the runs
-- timeline (J1/J2) and the series follow-up seed's deltas (J5).
--
-- Both nullable + backfill-free: pre-existing run rows keep NULL (the UI renders a
-- neutral row), and every new firing writes them in dispatch::finalize_success.
--   * result_preview     — a short (~PREVIEW_CHARS) plain-text digest of the result.
--   * change_summary_json — { changed: bool, new_count: int, new_items?: [..] } from
--                           the existing change-detection outcome.
ALTER TABLE scheduled_task_runs
    ADD COLUMN result_preview TEXT,
    ADD COLUMN change_summary_json JSONB;
