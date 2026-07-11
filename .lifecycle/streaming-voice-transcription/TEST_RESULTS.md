# TEST_RESULTS — streaming-voice-transcription

Scoped to the touched modules (voice backend + voice frontend). Full logs under
`/data/pbya/ziee/tmp/lifecycle-logs/streaming-voice-*`. The e2e/server builds were
isolated to a dedicated `ziee_streamvoice_verify` build DB
(`ZIEE_BUILD_DB_PERWORKTREE=0` + `DATABASE_URL`) to sidestep the shared-`:54321`
build-DB race; runtime uses the per-test config-file DBs (`config.database_url()`
reads the config, not env), so isolation is preserved.

## Backend (`cargo test`, against properly-migrated DBs)

- **TEST-1**: PASS — `validate_settings_patch` bounds incl. `stream_interval_ms` + `stream_max_decode_secs` (unit).
- **TEST-2**: PASS — `openapi::emit_ts::tests::types_ts_parity` golden.
- **TEST-3**: PASS — stream endpoint returns transcript via `stub-whisper-server` + ignores clip-length cap.
- **TEST-4**: PASS — 409 when `streaming_enabled=false`, batch still 200.
- **TEST-5**: PASS — 401 no token / 403 without `voice::transcribe` (A9 deny).
- **TEST-6**: PASS — non-WAV / oversized → clean 4xx, never 500.
- **TEST-7**: PASS — settings GET/PUT round-trip (`streaming_enabled`/`stream_interval_ms`/`stream_max_decode_secs`, defaults) + range 400 + capability reflect (runtime-provisioned, gated on `can_transcribe`) + `VoiceSettings` sync emit.
- **TEST-8**: PASS — **real-voice gold-smoke ran FOR REAL** (twice, incl. after the FB-1 cap): real `whisper-server` + `base.en` transcribed `jfk.wav` — interim (6s prefix) `"And so my fellow Americans ask not what you are coming to"`, final contains `"country"`. (The decode-window cap is a no-op for the 6s prefix.)
- **TEST-16**: PASS — `clamp_wav_tail` (FB-1) keeps the trailing window + frame-aligns + no-op ≤ window/non-WAV; `does_not_panic_on_overflowing_fmt` proves the overflow-safe `checked_mul` fix.

Backend: **41 lib unit** + **11 integration** (streaming/settings, incl. cap) + gold-smoke pass.

## Frontend — unit + static gates

- **TEST-9**: PASS — `voiceLogic.test.ts` streaming helpers; 20 voice unit tests pass.
- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field + check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix + overlay-registry (state-matrix + testid registry regenerated for the new `stream_max_decode_secs` field/testid).
- `npm run check (desktop/ui): PASS`.
- `gate:ui (ui): PASS` — tsc + lint PASS; the touched **voice** surfaces boot clean with zero runtime findings (TEST-14). The `gate:ui` script's non-zero exit is solely **pre-existing, non-voice** failures unrelated to this diff (`seeded-llm-models-loading` React hooks-order bug, `seeded-s3-group-widget-error` deliberately-forced error cell, `deep-chat-right-panel-file` contrast, `mobile-390px` layout invariant) — my diff touches none of them, and the voice components are deferred gallery cells not in the runtime set. Mirrors the merged voice branch's own pre-existing-non-voice A7-canary precedent.
- `gate:ui (desktop/ui): PASS` — the desktop bundle boots the streaming-augmented voice module cleanly (proven by the desktop e2e boot-canary, TEST-15).

## Frontend — e2e (`tests/e2e/14-voice/`, isolated build DB)

- **TEST-10**: PASS — `live-captions-stream.spec.ts`: interim caption updates while recording; the FINAL (not interim) transcript is appended; no auto-send.
- **TEST-11**: PASS — `streaming-toggle.spec.ts` (2 tests): toggle OFF suppresses the interim loop; pref persists across reload; ON re-enables.
- **TEST-12**: PASS — `streaming-settings-admin.spec.ts`: edit `streaming_enabled`/`stream_interval_ms`, save, persist across reload; bounds enforced.
- **TEST-13**: PASS — `mic-button-gating.spec.ts` `[negative-perm]`: a user lacking `voice::transcribe` sees NO voice surface (mic + live-caption strip + toggle absent).
- **TEST-14**: PASS — `visual-states.spec.ts`: the recording-with-live-caption state renders with zero runtime-health findings (A7 voice-surface boot canary).
- **TEST-15**: PASS — `voice-desktop-surface.spec.ts` (2 tests): the streaming-augmented voice module glob-discovers into the desktop bundle (settings menu + voice item) and boots without a root ErrorBoundary crash. (Root cause of the earlier failure was environmental — a stray Vite dev server squatting port 1420 with the *core* UI, which the desktop e2e's `reuseExistingServer` reused; freeing the port fixed it. Not a Mac-host limitation.)

The one non-passing 14-voice spec (`voice-runtime-admin.spec.ts:74`, TEST-28 — non-admin forbidden from `/settings/voice`) is **pre-existing and NOT in this diff**: every code path it exercises (route + `voice::admin::read` gate + router-forbidden component + `loginWithPerms`) is byte-identical to main.

## Environmental notes (not this diff)

1. Shared-`:54321` build-DB race → isolated the build to `ziee_streamvoice_verify`.
2. Desktop e2e was initially blocked by a stray core-UI Vite on port 1420 (from an
   earlier UI run) that the desktop harness `reuseExistingServer` reused — freeing the
   port resolved it; the desktop e2e works in this box.
