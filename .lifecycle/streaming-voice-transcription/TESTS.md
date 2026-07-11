# TESTS — streaming-voice-transcription

Every ITEM maps to ≥1 TEST; every TEST names ITEM(s), a tier, a target file, and
what it proves. Backend items get unit + integration; user-visible UI items get
e2e. Mocks only the external boundary (whisper `/inference` via the real
`stub_whisper_binary()` for deterministic tiers; the REAL `whisper-server` +
`base.en` for the gold-smoke; browser media APIs in e2e) — no cosmetic tests.

## Backend — unit (`#[cfg(test)]`)

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/voice/handlers.rs` — asserts: `validate_settings_patch` rejects out-of-range `stream_interval_ms` (300..=10000) with a 400 `VALIDATION_ERROR`, accepts in-range values, and accepts `streaming_enabled` toggles.
- **TEST-2** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` golden — `types.ts` regenerated from the committed `openapi.json` matches the committed `types.ts` (so the new `Voice.transcribeStream` key + changed schemas are regenerated, not hand-edited).
- **TEST-16** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/voice/stream.rs` — asserts: `clamp_wav_tail` (FB-1) keeps the trailing `secs` of PCM + rewrites RIFF/`data` sizes (result ≈ window, newest sample preserved); is a no-op when the clip is at/under the window (fully stitched) or the header is non-WAV / `secs==0`.

## Backend — integration (`tests/voice/streaming_test.rs`, deterministic stub whisper)

- **TEST-3** (tier: integration) [covers: ITEM-4, ITEM-5] file: `src-app/server/tests/voice/streaming_test.rs` — asserts: the hero path — with the `stub_whisper_binary()` runtime registered + model staged, `POST /voice/transcribe/stream` (multipart WAV, `voice::transcribe` user) auto-starts whisper, forwards the full buffer to `/inference`, and returns 200 `{ text, language, duration_ms }` with the canned transcript.
- **TEST-4** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/voice/streaming_test.rs` — asserts: with `streaming_enabled=false` (PUT settings), `POST /voice/transcribe/stream` returns 409 (feature-off) while batch `/voice/transcribe` still works — the two modes are independently toggled.
- **TEST-5** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/voice/streaming_test.rs` — asserts: the backend deny path (A9) — `POST /voice/transcribe/stream` returns 401 with no token and 403 for a user lacking `voice::transcribe`.
- **TEST-6** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/voice/streaming_test.rs` — asserts: a non-WAV / oversized body is rejected with a clean 4xx (`VOICE_NOT_WAV` / `VOICE_CLIP_TOO_LARGE`), never a 500, before any runtime is touched.
- **TEST-7** (tier: integration) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/server/tests/voice/settings_test.rs` — asserts: `GET/PUT /voice/settings` round-trips `streaming_enabled`/`stream_interval_ms`/`stream_max_decode_secs` (admin-gated, defaults 30s), out-of-range values (incl. `stream_max_decode_secs` 5..=600) 400, `GET /voice/capability` reflects `streaming_enabled` for a non-admin `voice::transcribe` user (gated on `can_transcribe`), and PUT emits the `VoiceSettings` sync event.

## Backend — gold-smoke (real whisper + real voice, soft-skip gated — DEC-13/14)

- **TEST-8** (tier: integration) [covers: ITEM-4, ITEM-5] file: `src-app/server/tests/voice/streaming_real_test.rs` — asserts: with the real `ziee-ai/whisper.cpp` release reachable (else `SOFT-SKIP [external gate]`), downloads the real `whisper-server` + `base.en`, streams a committed short English speech WAV through the full production path, and hard-asserts (a) ≥1 mid-recording `/voice/transcribe/stream` interim response is non-empty and (b) the final `/voice/transcribe` transcript contains the expected keywords (case-insensitive) — the only test proving live captions work on real acoustics.

## Frontend — unit (node:test)

- **TEST-9** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/modules/chat/extensions/voice/voiceLogic.test.ts` — asserts: `shouldRunInterim` is true only while `recording` with `capability.streaming_enabled` and the live pref on; `resolveLivePref` defaults a per-device pref to `streaming_enabled` when unset and honors a stored value; `composeInterimCaption` trims and maps blank → cleared.

## Frontend — e2e (`tests/e2e/14-voice/`, Playwright, mocked media + cassette)

- **TEST-10** (tier: e2e) [covers: ITEM-7, ITEM-9] file: `src-app/ui/tests/e2e/14-voice/live-captions-stream.spec.ts` — asserts: with streaming enabled + live pref on, recording shows the live-caption strip growing into the full stitched transcript from mocked `/voice/transcribe/stream` responses; on Stop the authoritative `/voice/transcribe` text is appended to the composer, the caption clears, and NO message is auto-sent (composer retains the text; send never fires).
- **TEST-11** (tier: e2e) [covers: ITEM-9, ITEM-7] file: `src-app/ui/tests/e2e/14-voice/streaming-toggle.spec.ts` — asserts: toggling "Live captions" OFF suppresses the interim loop (no `/voice/transcribe/stream` request; batch-only behavior) and ON re-enables it, and the per-device pref persists across a reload.
- **TEST-12** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/14-voice/streaming-settings-admin.spec.ts` — asserts: an admin edits `streaming_enabled`/`stream_interval_ms` in `VoiceConfigCard`, the values persist (reload), and out-of-range input is validated before submit.
- **TEST-13** (tier: e2e) [negative-perm] [covers: ITEM-9] file: `src-app/ui/tests/e2e/14-voice/mic-button-gating.spec.ts` — asserts: a user LACKING `voice::transcribe` sees NO voice surface at all — no composer mic button, no live-caption strip, no Live-captions toggle (defensive; not A10-forced since streaming introduces no new permission).
- **TEST-14** (tier: e2e) [covers: ITEM-11, ITEM-9] file: `src-app/ui/tests/e2e/14-voice/visual-states.spec.ts` — asserts: the new MicButton recording-with-live-caption gallery state renders with zero runtime-health findings (drives the `gate:ui` / `check:state-matrix` coverage for the new render state).
- **TEST-15** (tier: e2e) [covers: ITEM-9] file: `src-app/desktop/ui/tests/e2e/voice-desktop-surface.spec.ts` — asserts: desktop parity — the streaming-augmented (live-captions) voice module still glob-discovers + boots cleanly in the desktop bundle (voice settings menu item present, zero console/page errors). The mocked desktop harness renders only the settings menu, not the composer, so the toggle/caption rendering itself is covered by the ui `14-voice` specs on the SAME shared code.

## Frontend static gate (recorded in TEST_RESULTS.md at phase 8)

- `npm run check (ui): PASS` and `npm run check (desktop/ui): PASS` (tsc + biome +
  lint:colors + check:state-matrix + check:testid-registry + …).
- `gate:ui (ui): PASS` (A7 boot/runtime canary — runtime-health + Layer A/axe).
