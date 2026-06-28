-- Consolidate the two parallel download-instance tables into one canonical
-- table. `download_instances` (migration 5) is the actively-used table: full
-- CRUD in modules/llm_model/repository.rs, the in-progress dedup unique index
-- (migration 119), and the hub install-tracking JOIN all target it.
--
-- `llm_download_instances` (migration 4) was superseded immediately by
-- migration 5 and has ZERO readers or writers anywhere in the codebase — it
-- only ever held the empty initial schema. No data move is required (nothing
-- is ever inserted into it); drop it so there is a single canonical table.
DROP TABLE IF EXISTS llm_download_instances CASCADE;
