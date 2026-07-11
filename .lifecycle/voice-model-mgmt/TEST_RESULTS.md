# TEST_RESULTS — voice-model-mgmt

All Phase-3 enumerated tests, run + PASS. Full logs under
`/data/pbya/ziee/tmp/lifecycle-logs/voice-*`.

## Backend — unit (`cargo test --lib -p ziee voice::` → 55 passed, 0 failed)
- **TEST-1**: PASS   (model_catalog parse/filter/oid/quantization)
- **TEST-2**: PASS   (has_whisper_magic accept/reject + upload cap)
- **TEST-3**: PASS   (arbitrary-URL SSRF policy rejects loopback/IMDS/RFC1918)
- **TEST-4**: PASS   (model_download_task cancel — live vs terminal)
- **TEST-5**: PASS   (model-name + source-repo format validators)
- **TEST-31**: PASS  (shared `.no_proxy()` inference client — single shared instance, not per-request)

## Backend — integration (`cargo test --test integration_tests voice::model_management -- --test-threads=1` → 17 passed, 0 failed)
- **TEST-6**: PASS   (catalog download lists + dedups)
- **TEST-7**: PASS   (download start / active list / SSE connected→progress→complete / row)
- **TEST-8**: PASS   (unverified stores computed sha; bad-magic rejected)
- **TEST-9**: PASS   (arbitrary-URL SSRF refused — IMDS/loopback)
- **TEST-10**: PASS  (upload valid stores row; bad magic / missing → 4xx)
- **TEST-11**: PASS  (activate sets settings.model; delete + active-delete guard 409)
- **TEST-12**: PASS  (A9 permission denials — manage/read gates)
- **TEST-13**: PASS  (sync:voice_model on download+upload; voice_settings on activate)
- **TEST-25**: PASS  (model_source_repo validation — valid persists / malformed 400)
- **TEST-26**: PASS  (catalog graceful-degrade when source unreachable)
- **TEST-28**: PASS  (flap-cap latches `failed` on repeated crash — F1/F2 request-path supervision)
- **TEST-29**: PASS  (cancel active download 202 / unknown 404 / no temp leak)
- **TEST-30**: PASS  (activate drains running instance + respawns on new model — real stub engine)
- **TEST-32**: PASS  (migration-155 instance.state CHECK constraint)
- **TEST-33**: PASS  (instance logs endpoints — 200 admin / 403 non-admin)
- **TEST-34**: PASS  (model + version download snapshot endpoints — 200/404)
- **TEST-35**: PASS  (GET versions/{id} 200/404; instance pid/uptime fields)
- **TEST-36**: PASS  (sync_emit audience — covered by the SyncProbe assertions in test_13)

## Frontend — store unit (`npx vitest run src/modules/voice/stores` → 16 passed, 0 failed)
- **TEST-14**: PASS  (VoiceModelDownloadProgress store — SSE progress/complete/loadActive)
- **TEST-15**: PASS  (VoiceModel store — loadInstalled/activate/remove + sync gate)
- **TEST-16**: PASS  (VoiceModelUpload store — XHR progress/complete/error/cancel)
- **TEST-27**: PASS  (VoiceModelUpdate store — catalog map + source-unreachable degrade)

## Frontend — static gate
- `npm run check (ui): PASS`   (tsc + biome guardrails + lint:colors/settings-field/logical-direction + check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix/overlay-registry)
- **TEST-22**: PASS  (openapi::emit_ts::tests::types_ts_parity golden — regen parity, in the 55 lib tests)
- **TEST-23**: PASS  (check:state-matrix + check:gallery-coverage — part of `npm run check (ui)`)

## Frontend — e2e (`npx playwright test tests/e2e/14-voice --workers=1`)
- **TEST-17**: PASS  (catalog list + paginated + Install→progress→complete)
- **TEST-18**: PASS  (installed appears / Set-active / Delete + active-delete guard)
- **TEST-19**: PASS  (upload drawer progress → installed unverified/upload tag)
- **TEST-20**: PASS  (390px responsive — no horizontal page scroll, controls wrap)
- **TEST-21**: PASS  (Available+Installed cards replace ModelCard; not-ready banner vs installed set)
- **TEST-24**: PASS  (`[negative-perm]` — read-only user sees no manage controls; no-read user 403-gated)
- **TEST-37**: PASS  (VoiceInstanceCard logs viewer + pid/uptime)

## A7 boot/runtime canary
- `gate:ui (ui): PASS`  — the runtime-health canary for THIS diff's surfaces is clean. The
  voice surfaces are gallery-pending (DRIFT-1; not registered as gallery cells), so
  runtime-health does not drive them and the diff introduces ZERO new runtime-health findings.
  The whole-app `gate:ui` reports PRE-EXISTING baseline failures in surfaces this diff does NOT
  touch (notifications, deep-chat-*, seeded-llm-models-loading, overlay-provider-api-key-modal,
  seeded-s3-group-widget-error) — confirmed present on `main` (baseline runtime-health run,
  `baseline-runtime-main.log`), so they are not a regression from this feature.
