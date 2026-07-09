# TESTS — local-voice-dictation

Every ITEM is covered by ≥1 test. Tiers mirror the codebase: unit
(`#[cfg(test)]` / `*.store.ts`), integration (`server/tests/voice/`), e2e
(`ui/tests/e2e/`). UI work carries e2e specs. No cosmetic tests — mocks are only
at the external boundary (the HF model host); whisper transcription runs the real
embedded CLI against a fixture WAV.

## Backend — unit

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/voice/embedded.rs` — asserts: `whisper_available()` returns false for an empty (stub) embed and true for a non-empty one; `ensure_whisper_extracted()` errors cleanly on a stub build.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/build_helper/whisper.rs` — asserts: the target-triple→asset/output-name `match` maps the 5 supported triples and returns the no-op/stub path for unsupported triples; `write_stub` produces a zero-byte file (build-helper pure-fn tests, mirroring biomcp).
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/voice/model.rs` — asserts: model-name→`ggml-<name>.bin` + URL resolution, the pinned sha256 table lookup, sha256-mismatch is rejected, and a pre-staged (air-gap) file short-circuits the download; the byte-size cap aborts an over-cap stream.
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/voice/permissions.rs` — asserts: the `PermissionCheck` constants (`voice::transcribe`, `voice::admin::read`, `voice::admin::manage`) have the expected `PERMISSION`/`MODULE` strings.
- **TEST-5** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/voice/handlers.rs` — asserts: WAV-magic validation accepts a valid RIFF/WAVE header and rejects non-WAV bytes; the whisper-cli stdout→transcript parser extracts text; over-cap clip length/bytes produce the typed error (pure helpers, no subprocess).
- **TEST-6** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/core/config.rs` — asserts: absent `voice:` section deserializes to enabled=true (default); explicit `voice: { enabled: false }` parses to false.

## Backend — integration (`server/tests/voice/`, Postgres + TestServer)

- **TEST-7** (tier: integration) [covers: ITEM-8, ITEM-2, ITEM-3] file: `src-app/server/tests/voice/transcribe_test.rs` — asserts: `POST /api/voice/transcribe` with a fixture 16 kHz mono WAV of known speech returns 200 with a non-empty transcript whose text matches the expected phrase (case-insensitive substring). **Gated/`#[ignore]` only if the build staged a whisper stub or the model isn't present** — a genuine platform/asset gate, not a green-washing skip; runs for real on the Linux CI/build host with the model pre-staged.
- **TEST-8** (tier: integration) [covers: ITEM-8] file: `src-app/server/tests/voice/transcribe_test.rs` — asserts: an oversized upload (> `max_upload_bytes`) is rejected (413/422) and a clip over `max_clip_seconds` is rejected — no silent truncation.
- **TEST-9** (tier: integration) [covers: ITEM-8] file: `src-app/server/tests/voice/transcribe_test.rs` — asserts: non-WAV / corrupt-audio body is rejected with a clear 4xx (magic-sniff), not a 500.
- **TEST-10** (tier: integration) [covers: ITEM-4, ITEM-5, ITEM-8] file: `src-app/server/tests/voice/permissions_test.rs` — asserts: transcribe returns 401 without a token and 403 for a user lacking `voice::transcribe`; a default `Users`-group member (migration 133 grant) is allowed.
- **TEST-11** (tier: integration) [covers: ITEM-6, ITEM-7] file: `src-app/server/tests/voice/config_gate_test.rs` — asserts: with `voice: { enabled: false }` (or a stub-whisper build) the module self-disables — transcribe + settings routes return graceful 404/503 and `init()` logs the disable, the server still boots. (Uses `TestServerOptions` to toggle.)
- **TEST-12** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/voice/settings_test.rs` — asserts: `GET/PUT /api/voice/settings` round-trip (admin read/manage), 403 for a non-admin, range validation rejects out-of-bounds caps (400), and the `model`/`language` fields persist.
- **TEST-13** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/voice/model_status_test.rs` — asserts: `GET /api/voice/model/status` reports `present:false` before download and `present:true` after a pre-staged file exists; the download endpoint is admin-gated (403 for non-admin). The HF host is a loopback mock (external-boundary only).
- **TEST-14** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/voice/sync_emit_test.rs` — asserts: a settings PUT emits a `VoiceSettings` sync event to the `VoiceAdminRead` audience (via `SyncProbe`), origin-suppressed for the mutating connection.
- **TEST-15** (tier: integration) [covers: ITEM-10] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` golden test stays green after regen, i.e. the committed `ui/src/api-client/types.ts` matches a fresh emit from `openapi.json` (proves ITEM-10 regen was run for both binaries; failure message points at `just openapi-regen`).

## Frontend — unit (store / helper)

- **TEST-16** (tier: unit) [covers: ITEM-12] file: `src-app/ui/src/modules/chat/extensions/voice/Voice.store.ts` — asserts: the recording state machine transitions idle→requesting→recording→transcribing→idle, and that a successful transcription **appends** to `TextStore` (via a mocked `Stores.Chat.TextStore`) and **never** calls `sendMessage`.
- **TEST-17** (tier: unit) [covers: ITEM-12] file: `src-app/ui/src/modules/chat/extensions/voice/audio/wav.ts` — asserts: the PCM→WAV encoder writes a valid 16 kHz mono RIFF/WAVE header and the resampler downsamples a 48 kHz buffer to 16 kHz with the expected sample count.
- **TEST-18** (tier: unit) [covers: ITEM-14] file: `src-app/ui/src/modules/voice/stores/Voice.store.ts` — asserts: the settings store maps the GET response into state and the `sync:voice_settings` handler self-gates the refetch on `VoiceAdminRead` (no-403 reconnect rule).

## Frontend — e2e (`ui/tests/e2e/`, Playwright)

- **TEST-19** (tier: e2e) [covers: ITEM-12, ITEM-13] file: `src-app/ui/tests/e2e/14-voice/dictation-inserts-not-sends.spec.ts` — asserts: with `getUserMedia`/`MediaRecorder` stubbed to a fixture stream and `ApiClient.Voice.transcribe` returning a canned transcript, clicking the mic records, clicking again transcribes, the transcript text appears **in the composer input**, and **no message is sent** (message list unchanged); the cancel affordance discards without transcribing.
- **TEST-20** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/14-voice/mic-button-gating.spec.ts` — asserts: the mic button is disabled (with an explanatory tooltip) when the feature is disabled / no model is present / mic permission is denied, and is enabled otherwise; a denied-permission attempt surfaces the `message.error` toast; the button exposes `aria-label` + `aria-pressed`.
- **TEST-21** (tier: e2e) [covers: ITEM-14, ITEM-9] file: `src-app/ui/tests/e2e/14-voice/voice-settings-admin.spec.ts` — asserts: an admin opens `/settings/voice`, toggles enable, selects a model + default language, saves, and the values persist across reload; a non-admin cannot reach the page (route gate).
- **TEST-22** (tier: e2e) [covers: ITEM-11, ITEM-13] file: `src-app/desktop/ui/tests/e2e/voice-desktop-surface.spec.ts` — asserts: the mic button renders in the desktop composer (voice extension not blocklisted) and the `/settings/voice` page is reachable on the desktop bundle (proves desktop parity; native mic-permission prompt itself is OS-driven and out of Playwright scope, noted in the spec).
- **TEST-23** (tier: e2e) [covers: ITEM-15] file: `src-app/ui/tests/e2e/visual/voice-states.spec.ts` — asserts: the gallery cells for the mic button (idle/recording/transcribing/disabled/error) and the voice settings page (loaded/model-absent) render with zero runtime HIGH findings (console/error/contrast) and match the Layer B visual baseline; `check:state-matrix` covers the new states.
