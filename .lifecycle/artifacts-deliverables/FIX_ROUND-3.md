# FIX_ROUND-3 — real bug surfaced by the phase-8 e2e

Running the phase-8 e2e for real (per the exit condition) surfaced a genuine product
bug the static/unit/integration layers could not: **canvas edits to `code`/`csv`
deliverables appeared to not persist after Save + reload.**

## Investigation (empirical, not hand-waved)

- Pre-save assertions proved the edit LANDS in the editor and Save enables (dirty set).
- A `handleSave` diagnostic proved `getContent()` returns the EDITED content
  (`"…CODE_EDIT_MARKER = 42\n"`), so the client sends the right bytes.
- A new integration test (`test_append_version_text_reextracted`, csv + python mime)
  proved the BACKEND correctly re-extracts text pages so `GET /text` returns the edited
  head — the append path is sound.
- The gap: the browser served a **stale `GET /files/{id}/text`**. Root cause —
  `FILE_CONTENT_CACHE_CONTROL = "private, max-age=3600"` was applied to the endpoints
  that serve a file's **head** content keyed by `file_id` (`/text`, `/content` (`get_raw`),
  `/preview`, `/thumbnail`). The head's bytes CHANGE when a version is appended (canvas
  Save, MCP `edit_file`/`rewrite_file`), so a 1-hour cache serves the pre-edit content —
  a co-edited deliverable silently shows stale text for up to an hour. Markdown's e2e
  passed only because it verifies via the `?format=md` export URL (a different cache key
  never populated pre-edit).

## Confirmed → FIXED

- **[correctness/HIGH] head-content endpoints cached the mutable head 1h → stale after
  Save.** Added `FILE_HEAD_CACHE_CONTROL = "private, no-cache"` (revalidate, still
  `private`) and switched `get_text_content` / `get_raw` / `get_preview` /
  `get_thumbnail` to it (`handlers/download.rs`, `handlers/management.rs`). The
  version-PINNED `/versions/{v}/text` (immutable by URL) keeps `max-age=3600`.
- **[tests-quality] no coverage for non-markdown text re-extraction.** Added
  `test_append_version_text_reextracted` (csv + python append → `/text` reflects the
  edit) — a real regression test for the co-edit path.

## Re-verification

The phase-8 e2e (`tests/e2e/14-artifacts/*`) is the verification: with the cache fixed,
the `code`/`csv` specs assert the saved head text carries the edit (authoritative `/text`,
polled). Re-run recorded in TEST_RESULTS.md.

**New confirmed findings:** 0
