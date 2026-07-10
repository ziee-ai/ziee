# TEST_RESULTS

Backend build/tests: `CARGO_TARGET_DIR=…/file-upload-size-cap-target` on the shared
`:54321` pgvector cluster. Logs under `…/tmp/lifecycle-logs/`.

## Phase-3 tests
- **TEST-1**: PASS — `core::config::max_file_upload_tests::{default_is_128, omitted_key_deserializes_to_default, explicit_key_overrides_default}` (lib, 3/3 ok).
- **TEST-2**: PASS — `core::app_state::tests::max_file_upload_bytes_round_trip_and_derived_body_limit` (lib ok).
- **TEST-3**: PASS — `file::test_upload_size_boundary_respects_configured_cap` (integration, `--test-threads=1`): 0.9 MiB→201, 1.5 MiB→400 `FILE_TOO_LARGE`, ~18 MiB rejected by the body-limit layer before the handler (observed 400 `UPLOAD_ERROR`, i.e. multipart stream truncated — NOT `FILE_TOO_LARGE`, proving the body limit is derived from the cap).
- **TEST-4**: PASS — `project::files_test::upload_and_attach_respects_configured_cap` (integration).
- **TEST-5**: PASS — `file::test_upload_gzip_rds_binary_accepted` (integration): gzip `.rds`→201, stored `mime_type=application/gzip`.
- **TEST-6**: PASS — `file::utils::magic::tests::{gzip_framed_rds_sniffs_gzip_and_is_allowed, uncompressed_binary_rds_is_unknown_and_allowed}` (lib).
- **TEST-7**: PASS — `file::project_extension::handlers::description_tests::upload_description_has_no_stale_hardcoded_cap` (lib).
- **TEST-8**: PASS — `ui/tests/e2e/chat/file-upload-size-limit.spec.ts` (Playwright, `--workers=1`): 1 passed. Over-128 MiB attach → "128MB" toast, no upload POST.
- **TEST-9**: PASS — `core::app_state::tests::docker_web_plumbs_max_file_upload_var` (lib).
- **TEST-10**: PASS — `core::app_state::tests::nginx_body_size_covers_default_body_limit` (lib).

Lib run: `21 passed; 0 failed` (incl. `openapi::emit_ts::tests::types_ts_parity` +
`types_ts_parity_desktop` — confirms the surgical openapi.json description edit did
NOT break the golden parity for either workspace).

## Frontend gate
- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors/settings-field/…
  + check:kit-manifest/testid-registry/design-spec/gallery-coverage/gallery-crawl/
  state-matrix/overlay-registry all green (state matrix regenerated for the
  line-number shift from the shared-constant imports; no new render state).

## Notes on unrelated failures observed in the broad `file:: project::` run
The broad run showed 15 failures, ALL pre-existing / environmental, none from this diff:
- 9 chat-integration tests (`file::file_attachments_test::*`) + 5 real-provider
  tests panic with "No AI provider API keys found … set in tests/.env.test" — this
  box has no `.env.test`; they are unrelated to the upload-size change.
- `project::conversations_test::chat_list_returns_all_user_conversations` fails
  deterministically (even isolated) at `body.as_array()` because
  `GET /conversations` returns a paginated object `{conversations,total}`
  (`ConversationListResponse`) since commit `9954e3ec` on khoi — the test is stale
  on `origin/khoi` and predates this branch; the chat module is untouched here.

## UI runtime/visual (gate:ui)
This diff adds NO new render state or gallery surface (state-matrix diff was pure
line-number shift; `check:state-matrix` + `check:gallery-coverage` pass) and no
visual change (the too-large toast fires only on an oversize attach, not a gallery
state).

`npm run gate:ui --skip-visual`: tsc PASS, lint PASS, runtime-health **157/161
surfaces PASS**. The gate reports HIGH findings on **4 surfaces this diff does NOT
touch** — all PRE-EXISTING on `origin/khoi`, none referencing any changed file
(grep of RUNTIME_FINDINGS.jsonl for `constants|MAX_FILE_UPLOAD|FileUpload*` = 0):
- `seeded-llm-models-loading` — React "Rendered more hooks than previous render"
  in the llm-models loading component (untouched module).
- `seeded-s3-group-widget-error` — a DELIBERATE gallery-forced error state (the
  event-only LLMProviderGroupWidget documented under CLAUDE.md "Known Issues").
- `deep-chat-streaming` — `File.store.ts::loadFileTextContent` `response.text is
  not a function` TypeError (the file STORE, which this diff does not modify — only
  the six upload components + `constants.ts` changed).
- `deep-chat-right-panel-file` — WCAG contrast on the right-panel viewer (untouched).

The upload-composer surfaces that this diff DOES touch (FileUploadButton /
FileUploadArea / FilePasteHandler / FileAttachMenuItem) are among the 157 PASS, so
the gate:ui failure is not a regression from this change. Visual (Layer B) skipped
(no rendered-surface change).
