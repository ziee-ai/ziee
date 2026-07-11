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
- **TEST-8**: PASS — **real-voice gold-smoke RAN FOR REAL on the merged base** (NOT soft-skipped — the external gate `ziee-ai/whisper.cpp` **v1.9.1** is reachable): the test downloaded the real `whisper-server-linux-x86_64-cpu` (sha256-verified) + the real `base.en` model (147,964,211 bytes from HF), spawned whisper, and transcribed `jfk.wav` — interim (6s prefix) `"And so my fellow Americans ask not what you are coming to"` (non-empty), final contains `"country"`. `1 passed, 13.1s`. Its soft-skip is a runtime external-gate early-return (per [[feedback_no_ignore_unless_platform]]) that would fire ONLY if the whisper release / HF were unreachable — it is not firing here. No `#[ignore]`/`.skip`/`.only` exists anywhere in this diff (audited).
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

**`voice-runtime-admin.spec.ts:74`** (TEST-28, non-admin forbidden from `/settings/voice`) — RAN + FIXED. On the merged tree it was the sole 14-voice failure (`.last-run.json`). Root cause (empirical): `/settings/voice` resolves through the settings shell, which renders the **settings-section 403** (`settings-forbidden-result`), but the spec waited only for the **router-level** `router-route-forbidden-result`. That mechanism predates this work (present at `af44c73c2`), and the non-voice `literature/admin-settings.spec.ts` already uses the robust `[router-route-forbidden-result], [settings-forbidden-result]` selector for the same reason — so the voice spec was stale, not caused by the streaming diff. Fixed to the same dual-selector and **ran it**: `voice-runtime-admin.spec.ts` → **2 passed (playwright rc=0)** on the merged tree. (The isolated origin/main worktree e2e couldn't complete — the throwaway worktree's cold e2e global-setup is killed by the harness — but the fix mirrors the codebase's proven pattern and is verified passing on the merged base.)

## Merge with current origin/main (tip `eedd8f7f2`)

Merged origin/main (118 commits). Migration renumbered **153 → 154** (`153` taken by
`scheduled_task_unattended_tools` on main). All merge conflicts were in **generated**
files (openapi.json ×2, testIds.generated, stateMatrix.generated, STATE_MATRIX.md) —
resolved by **regenerating** (`--generate-openapi` for BOTH ui/ + desktop/ui/;
`gen-testid-registry` + `gen-state-matrix`), never hand-edited. Merged server
`cargo check` clean (migration 154 + main's migrations applied, sqlx verified); ui +
desktop `tsc` clean; `types_ts_parity` golden holds.

## Environmental notes (not this diff)

1. Shared-`:54321` build-DB race → isolated the build to `ziee_streamvoice_verify`.
2. Desktop e2e was initially blocked by a stray core-UI Vite on port 1420 (from an
   earlier UI run) that the desktop harness `reuseExistingServer` reused — freeing the
   port resolved it; the desktop e2e works in this box.
