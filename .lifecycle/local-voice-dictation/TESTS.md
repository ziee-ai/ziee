# TESTS — local-voice-dictation (full managed whisper runtime)

Every ITEM covered by ≥1 test. Tiers mirror the codebase: unit (`#[cfg(test)]`
/ `*.store.ts`), integration (`server/tests/voice/`), e2e (`ui/tests/e2e/`). UI
work carries e2e specs. No cosmetic tests — mocks only at the external boundary
(the GitHub-release host + the HF model host, via the debug mirror seams); real
transcription runs a real `whisper-server` against a fixture WAV.

**Fixtures:** `src-app/stub-whisper-server/` (a tiny axum server mimicking
`whisper-server` `/` + `/inference`, modeled on `stub-engine/`) and
`server/tests/voice/mock_release.rs` (`MockReleaseServer` packaging the stub as a
release asset, modeled on `tests/llm_local_runtime/mock_release.rs`). These let
the version/download/update/lifecycle tests exercise the real code paths with no
network and no paid credentials.

## Backend — unit

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/voice/engine/download.rs` — asserts: `archive_name`/`asset_backend` round-trip for the 5 platforms; `get_latest_version`/`list_releases` parse a canned GitHub JSON; the 2 GiB cap aborts an over-cap stream; mirror-env seams are honored only under debug.
- **TEST-2** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/voice/binary_manager.rs` — asserts: `select_version` precedence (system-default → latest); `check_for_updates` marks a release `installed` vs `binary_ready`; `set_system_default` clears the previous default.
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/voice/runtime_version/download_task.rs` — asserts: `start_or_join` dedupes a concurrent second POST onto the same task key and replaces a terminal entry on retry; the shutdown notify cancels an in-flight run.
- **TEST-4** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/voice/model/download.rs` — asserts: model-name→`ggml-<name>.bin`+URL resolution; the pinned sha256 table verifies a match and rejects a mismatch (deleting the partial); a pre-staged (air-gap) file short-circuits the download.
- **TEST-5** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/voice/engine/health.rs` — asserts: `Starting→Healthy`, crash→`Restarting{backoff}` with exponential `next_at`, flap cap (5 crashes/60 s)→`Failed`, and `from_persisted` rebuild.
- **TEST-6** (tier: unit) [covers: ITEM-7] file: `src-app/server/src/modules/voice/deployment/local.rs` — asserts: the `whisper-server` argv builder emits `--host 127.0.0.1 --port <p> -m <model> -l <lang>` and rejects argv values with shell metachars/leading `-` (hardening).
- **TEST-7** (tier: unit) [covers: ITEM-15] file: `src-app/server/src/modules/voice/handlers.rs` — asserts: WAV magic-byte validation accepts RIFF/WAVE and rejects non-WAV; the `/inference` JSON response→transcript parser; over-cap clip length/bytes produce the typed error (pure helpers, no subprocess).
- **TEST-8** (tier: unit) [covers: ITEM-12] file: `src-app/server/src/modules/voice/permissions.rs` — asserts: the `PermissionCheck` constants (`voice::transcribe`, `voice::admin::read`, `voice::admin::manage`) have the expected `PERMISSION`/`MODULE`.
- **TEST-9** (tier: unit) [covers: ITEM-14] file: `src-app/server/src/core/config.rs` — asserts: absent `voice:` ⇒ enabled=true; `voice: { enabled: false }` ⇒ false.
- **TEST-10** (tier: unit) [covers: ITEM-11] file: `src-app/server/src/modules/voice/runtime_settings/models.rs` — asserts: `VoiceRuntimeSettings::default()` (idle 1800 / auto-start 30 / drain 30 / model base / language auto / caps) and the PATCH range-validation rejects out-of-bounds.

## Backend — integration (`server/tests/voice/`, Postgres + TestServer)

- **TEST-11** (tier: integration) [covers: ITEM-15, ITEM-7, ITEM-9] file: `src-app/server/tests/voice/transcribe_test.rs` — asserts: `POST /api/voice/transcribe` with a fixture 16 kHz mono WAV of known speech auto-starts a **real** `whisper-server` (tiny model pre-staged on the Linux host) and returns 200 with a transcript matching the expected phrase (case-insensitive substring). `#[ignore]`-gated ONLY on a stub-binary build / model-absent (genuine asset gate, not green-washing — runs for real in CI with the tiny model staged).
- **TEST-12** (tier: integration) [covers: ITEM-15] file: `src-app/server/tests/voice/transcribe_test.rs` — asserts: an over-`max_upload_bytes` upload and an over-`max_clip_seconds` clip are rejected (413/422); non-WAV/corrupt body → clear 4xx (magic-sniff), not 500.
- **TEST-13** (tier: integration) [covers: ITEM-12, ITEM-13, ITEM-15] file: `src-app/server/tests/voice/permissions_test.rs` — asserts: transcribe → 401 without a token, 403 for a user lacking `voice::transcribe`, allowed for a default `Users` member (migration 134); admin endpoints 403 for a non-admin.
- **TEST-14** (tier: integration) [covers: ITEM-14, ITEM-16] file: `src-app/server/tests/voice/config_gate_test.rs` — asserts: with `voice: { enabled: false }` (or a stub build) the module self-disables — transcribe + admin routes return graceful 404/503, `init()` logs the disable, the server still boots.
- **TEST-15** (tier: integration) [covers: ITEM-2, ITEM-3, ITEM-4, ITEM-17] file: `src-app/server/tests/voice/version_download_test.rs` — asserts: against `MockReleaseServer` (mirror seams), `POST /api/voice/versions/download` runs the full resolve→download→extract→register path, the SSE `downloads/{key}/events` stream emits `progress`→`complete`, and a `voice_runtime_versions` row appears.
- **TEST-16** (tier: integration) [covers: ITEM-5, ITEM-17] file: `src-app/server/tests/voice/version_update_test.rs` — asserts: `GET /api/voice/versions/check-updates` lists an available newer release as `binary_ready && !installed`; `POST /api/voice/versions/{id}/set-default` flips the default (and emits `sync:voice_runtime_version`).
- **TEST-17** (tier: integration) [covers: ITEM-3, ITEM-17] file: `src-app/server/tests/voice/version_delete_test.rs` — asserts: `DELETE /api/voice/versions/{id}` refuses with 409 when the version is the active/default in-use one, succeeds (204) otherwise, and `?remove_binary=true` clears the on-disk dir.
- **TEST-18** (tier: integration) [covers: ITEM-18] file: `src-app/server/tests/voice/settings_test.rs` — asserts: `GET/PUT /api/voice/settings` round-trip (admin read/manage), 403 for non-admin, range validation → 400, `model`/`language`/caps persist.
- **TEST-19** (tier: integration) [covers: ITEM-18, ITEM-6] file: `src-app/server/tests/voice/model_test.rs` — asserts: `GET /api/voice/model/status` reports `present:false`→`true` around a `POST /api/voice/model/download` (HF host = loopback mock), admin-gated; the SSE progress stream completes.
- **TEST-20** (tier: integration) [covers: ITEM-9, ITEM-10, ITEM-18] file: `src-app/server/tests/voice/lifecycle_test.rs` — asserts: the managed `whisper-server` auto-starts on first transcribe, `GET /api/voice/instance` reports `running/healthy`, the idle-reaper (short `WHISPER_RUNTIME_REAPER_TICK_MS`) drains+stops it after idle, and `POST /api/voice/instance/restart` brings it back.
- **TEST-21** (tier: integration) [covers: ITEM-18] file: `src-app/server/tests/voice/sync_emit_test.rs` — asserts: a settings PUT emits `VoiceSettings` and a set-default emits `VoiceRuntimeVersion` to the `VoiceAdminRead` audience (via `SyncProbe`), origin-suppressed for the mutating connection.
- **TEST-22** (tier: integration) [covers: ITEM-19] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` golden test stays green after regen (committed `types.ts` == fresh emit from `openapi.json`), proving ITEM-19 regen ran for both binaries.

## CI / fork release contract

- **TEST-32** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/voice/engine/download.rs` — asserts: for every `{platform,arch,backend}` matrix entry, `archive_name(...)` equals the asset filename the `ziee-ai/whisper.cpp` release workflow publishes (from a checked-in naming-contract fixture shared with the fork), so the fork CI and the downloader cannot drift. The fork's `release.yml` itself is validated locally via `act` against a temp bare repo (auto-generated keys/secrets), per the "test CI workflows locally" rule — recorded as a manual pre-tag step in the fork repo, with this contract test the in-tree backstop.

## Frontend — unit (store / helper)

- **TEST-23** (tier: unit) [covers: ITEM-21] file: `src-app/ui/src/modules/chat/extensions/voice/Voice.store.ts` — asserts: the recording state machine transitions idle→requesting→recording→transcribing→idle, a successful transcription **appends** to a mocked `TextStore` and **never** calls `sendMessage`, and a denied `getUserMedia` routes to the error state.
- **TEST-24** (tier: unit) [covers: ITEM-21] file: `src-app/ui/src/modules/chat/extensions/voice/audio/wav.ts` — asserts: the resampler downsamples a 48 kHz buffer to 16 kHz with the expected sample count and the encoder writes a valid 16 kHz mono RIFF/WAVE header.
- **TEST-25** (tier: unit) [covers: ITEM-22] file: `src-app/ui/src/modules/voice/stores/VoiceDownloadProgress.store.ts` — asserts: `loadActive()` re-subscribes to non-terminal downloads on mount (page-reload survival) and the `sync:voice_runtime_version` / `sync:voice_settings` handlers self-gate the refetch on `VoiceAdminRead` (no-403 rule).

## Frontend — e2e (`ui/tests/e2e/`, Playwright)

- **TEST-26** (tier: e2e) [covers: ITEM-21] file: `src-app/ui/tests/e2e/14-voice/dictation-inserts-not-sends.spec.ts` — asserts: with `getUserMedia`/`MediaRecorder` stubbed and `ApiClient.Voice.transcribe` returning a canned transcript, clicking the mic records, clicking again transcribes, the transcript appears **in the composer input**, **no message is sent**, and cancel discards without transcribing.
- **TEST-27** (tier: e2e) [covers: ITEM-21, ITEM-15] file: `src-app/ui/tests/e2e/14-voice/mic-button-gating.spec.ts` — asserts: the mic button is disabled (with tooltip) when the feature is off / no model / permission denied, enabled otherwise; a denied-permission attempt shows the `message.error` toast; the button exposes `aria-label` + `aria-pressed`.
- **TEST-28** (tier: e2e) [covers: ITEM-22, ITEM-17] file: `src-app/ui/tests/e2e/14-voice/voice-runtime-admin.spec.ts` — asserts: an admin opens `/settings/voice`, runs "check for updates", installs a version (mock release) watching the SSE `<Progress>` to completion, sets it default, and deletes another; a non-admin cannot reach the page (route gate).
- **TEST-29** (tier: e2e) [covers: ITEM-18, ITEM-22] file: `src-app/ui/tests/e2e/14-voice/voice-settings-admin.spec.ts` — asserts: the admin edits idle/timeout, selects a model + default language + caps, downloads the model (mock, SSE progress), saves, and the values persist across reload.
- **TEST-30** (tier: e2e) [covers: ITEM-20, ITEM-21] file: `src-app/desktop/ui/tests/e2e/voice-desktop-surface.spec.ts` — asserts: the mic button renders in the desktop composer (voice extension not blocklisted) and `/settings/voice` is reachable on the desktop bundle (native OS mic prompt is out of Playwright scope, noted in the spec).
- **TEST-31** (tier: e2e) [covers: ITEM-23] file: `src-app/ui/tests/e2e/visual/voice-states.spec.ts` — asserts: the gallery cells for the mic button (idle/recording/transcribing/disabled/error) and the voice admin page (versions installed/available/downloading/empty, model absent) render with zero runtime HIGH findings (console/error/contrast) and match the Layer B baseline; `check:state-matrix` covers the new states.

## UX states (normal user + admin)

- **TEST-33** (tier: integration) [covers: ITEM-18] file: `src-app/server/tests/voice/capability_test.rs` — asserts: `GET /api/voice/capability` is reachable by a normal `voice::transcribe` user (not admin-gated) and reports `can_transcribe=false` with the right `runtime_ready`/`model_ready` flags when unprovisioned, `true` once a runtime + model are present; a user lacking `voice::transcribe` gets 403.
- **TEST-34** (tier: e2e) [covers: ITEM-21] file: `src-app/ui/tests/e2e/14-voice/mic-not-ready.spec.ts` — asserts: with `capability` returning enabled-but-not-ready, the mic is **disabled with the "contact an administrator" tooltip** and clicking never calls a download/admin endpoint; with the feature off (or `getUserMedia` absent) the mic is **hidden**; the first-use **privacy hint** appears once and stays dismissed after reload.
- **TEST-35** (tier: e2e) [covers: ITEM-21] file: `src-app/ui/tests/e2e/14-voice/mic-recording-ux.spec.ts` — asserts: the recording indicator shows an elapsed timer, **auto-stops at `max_clip_seconds`** (short test cap) with the "reached maximum length" note and still transcribes, and a simulated cold start surfaces the staged **"Starting voice engine…" → "Transcribing…"** status (`aria-live`).
- **TEST-36** (tier: e2e) [covers: ITEM-22] file: `src-app/ui/tests/e2e/14-voice/admin-empty-state.spec.ts` — asserts: on a fresh (unprovisioned) deployment the `/settings/voice` page leads with the "enabled but not ready" setup banner and the runtime/model empty-install states; after a mock install + model download the banner clears and the instance-health block shows `running/healthy`.
