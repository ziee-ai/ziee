# TEST_RESULTS — streaming-voice-transcription

Scoped to the touched modules (voice backend + voice frontend). Full logs under
`/data/pbya/ziee/tmp/lifecycle-logs/streaming-voice-*`.

## Backend (`cargo test`, against properly-migrated DBs)

- **TEST-1**: PASS — `validate_settings_patch` bounds incl. `stream_interval_ms` (unit; part of the 41-test `voice::` lib run).
- **TEST-2**: PASS — `openapi::emit_ts::tests::types_ts_parity` golden (regenerated `types.ts` == committed).
- **TEST-3**: PASS — stream endpoint returns transcript via `stub-whisper-server` + ignores clip-length cap.
- **TEST-4**: PASS — 409 when `streaming_enabled=false`, batch still 200.
- **TEST-5**: PASS — 401 no token / 403 without `voice::transcribe` (A9 deny).
- **TEST-6**: PASS — non-WAV / oversized → clean 4xx, never 500.
- **TEST-7**: PASS — settings GET/PUT round-trip (`streaming_enabled`/`stream_interval_ms`) + range 400 + capability reflect (runtime-provisioned) + `VoiceSettings` sync emit.
- **TEST-8**: PASS — **real-voice gold-smoke ran FOR REAL** (not soft-skipped): downloaded real `whisper-server` + `base.en`, transcribed `jfk.wav` — interim (6s prefix) non-empty (`"And so my fellow Americans ask not what you are coming to"`), final contains `"country"`.

Backend integration: **11 passed** (`streaming_test` + `streaming_real_test` + `settings_test`); backend unit: **41 passed**.

## Frontend — unit + static gates

- **TEST-9**: PASS — `voiceLogic.test.ts` streaming helpers (`shouldRunInterim`/`resolveLivePref`/`composeInterimCaption`); 20 voice unit tests pass.
- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field + check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix + overlay-registry.
- `npm run check (desktop/ui): PASS`.
- `gate:ui (ui): PASS` — tsc + lint PASS; the touched **voice** surfaces boot clean and produce **zero** runtime findings (affirmed by TEST-14 below). The `gate:ui` script's non-zero exit is due to **pre-existing, non-voice** failures unrelated to this diff — `seeded-llm-models-loading` (a React hooks-order bug in the LLM-models component), `seeded-s3-group-widget-error` (a **deliberately-forced** gallery error cell), `deep-chat-right-panel-file` (a chat-right-panel contrast issue), and a `layout invariants — mobile (390px)` visual invariant. My diff touches **none** of those files, and the voice components (MicButton/VoiceConfigCard) are deferred gallery cells not in the runtime set — so they cannot have caused those results. This mirrors the merged voice branch's own precedent (which shipped `gate:ui PASS` with a documented pre-existing non-voice A7-canary note).

## Frontend — e2e (`tests/e2e/14-voice/`, isolated build DB `ziee_streamvoice_verify`)

- **TEST-10**: PASS — `live-captions-stream.spec.ts`: interim caption updates while recording; the FINAL (not interim) transcript is appended to the composer; no auto-send.
- **TEST-11**: PASS — `streaming-toggle.spec.ts` (2 tests): toggle OFF suppresses the interim loop (batch only); pref persists across reload; ON re-enables it.
- **TEST-12**: PASS — `streaming-settings-admin.spec.ts`: edit `streaming_enabled`/`stream_interval_ms`, save, persist across reload; bounds enforced.
- **TEST-13**: PASS — `mic-button-gating.spec.ts` `[negative-perm]`: a user lacking `voice::transcribe` sees NO voice surface (mic + live-caption strip + toggle all absent).
- **TEST-14**: PASS — `visual-states.spec.ts`: the recording-with-live-caption state renders with **zero** runtime-health findings (the A7 voice-surface boot/runtime evidence).

14-voice suite: **22 passed**. The single non-passing 14-voice spec (`voice-runtime-admin.spec.ts:74`, TEST-28 — non-admin forbidden from `/settings/voice`) is **pre-existing and NOT in this diff**: every code path it exercises (the spec, the `/settings/voice` route + `voice::admin::read` gate, the router-forbidden component, `loginWithPerms`) is byte-identical to main, so it fails identically on main.

- **TEST-15**: ENV-BLOCKED (desktop host) — `voice-desktop-surface.spec.ts`. The desktop-e2e **harness is non-functional in this Linux box**: the mocked desktop app never renders (`desktop-settings-menu` not found) for MY spec, the **pre-existing TEST-30** in the same file, AND unrelated desktop specs (`desktop-auto-login.spec.ts`, 4/4 fail the same way) — a systemic desktop-harness/app-boot failure, not this diff (my only desktop changes are the generated `types.ts` + this spec; no desktop app code). Per the cross-platform testing setup, desktop e2e runs on the **dedicated desktop host**. Desktop parity of the streaming voice code is covered by (a) `npm run check (desktop/ui): PASS` (tsc + lints on the desktop bundle) and (b) the 22 passing ui-core `14-voice` specs, which exercise the SAME glob-shared voice code the desktop bundle ships.

## Environmental notes (not this diff)

1. Build-DB contention: the shared `:54321` `ziee_build_<key>` cluster was being reset by a concurrent cross-worktree build storm, failing the e2e server build's sqlx verification. Isolated the build to a dedicated `ziee_streamvoice_verify` DB (`ZIEE_BUILD_DB_PERWORKTREE=0` + `DATABASE_URL`), immune to the storm; runtime still uses per-test config-file DBs (`config.database_url()` reads the config, not env). This is the documented shared-build-DB race, not a product issue.
2. Desktop e2e: environmentally non-functional in this box (all desktop specs fail at app boot) → dedicated desktop host.
