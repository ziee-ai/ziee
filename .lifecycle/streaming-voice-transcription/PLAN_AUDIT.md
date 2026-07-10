# PLAN_AUDIT — streaming-voice-transcription

Plan audited against the merged voice code (backend `modules/voice/*`, frontend
`modules/chat/extensions/voice/*` + `modules/voice/*`) at `af44c73c2`.

## Breakage risk

- The batch `POST /api/voice/transcribe` path is **untouched in behavior**: the
  final on-stop decode still flows through it unchanged, so every existing
  `tests/voice/*` and `14-voice/*` assertion holds. ITEM-5 only makes
  `forward_to_whisper` shared (`pub(super)` + a timeout parameter); the batch
  caller passes the current 300 s, so its behavior is byte-identical.
- ITEM-1/2/3 are **additive** columns/fields with defaults, so existing GET/PUT
  callers and the `voice_runtime_settings` seed row keep working; no existing DTO
  field changes type. Risk: forgetting to thread a new field through
  `repository.rs` SELECT/UPDATE would silently drop it — covered by TEST-7.
- ITEM-7 changes `recorder.start()` → `recorder.start(timeslice)`. This is safe:
  a timeslice only makes `ondataavailable` fire periodically; the accumulated
  `chunks` blob on stop is identical, so the batch finalization is unaffected.
  Interim decoding uses the accumulate-from-start blob (a valid container),
  never a lone mid-stream chunk.
- Interim requests hold `InflightGuard`, keeping the whisper instance hot during
  recording (desired) and correctly participating in the reaper drain; no new
  drain semantics.

## Pattern conformance

- **PASS** — the streaming handler mirrors `transcribe.rs` one-for-one (same
  multipart ingest, WAV validate, `ensure_running`/`InflightGuard`/
  `forward_to_whisper`, `RequirePermissions<(VoiceTranscribe,)>`, `with_permission`
  docs). No new architectural seam.
- **PASS** — settings additions mirror the existing `validate_settings_patch`
  numeric arms + `repository.rs` COALESCE patch; capability additions mirror the
  existing `VoiceCapability` snapshot.
- **PASS** — store loop mirrors the existing module-scope imperative resources +
  `requestGeneration`/`isSuperseded` guard; pure logic extracted to `voiceLogic.ts`
  exactly as the merged code established.
- **PASS** — MicButton toggle reuses the existing localStorage-pref precedent
  (`ziee.voice.privacyHintDismissed`); admin fields reuse the `VoiceConfigCard`
  form idiom.

## Migration collisions

- Next free number is **153** (highest committed is 152). One additive migration.
  Merge-gate C2 re-checks against real main. No collision within this branch.

## OpenAPI regen

- **Required** (new route + 3 changed schemas). `just openapi-regen` regenerates
  BOTH `ui/` and `desktop/ui/`. The `types_ts_parity` golden test gates drift.
  Any e2e route-mock for `/api/voice/transcribe/stream` (R2-5) matches a live
  route only after this regen — sequence the regen before running e2e.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive ALTER on `voice_runtime_settings`; mirrors migration 151's CHECK-constrained columns; next free number 153.
- **ITEM-2** — verdict: PASS — mirrors existing DTO fields + `validate_settings_patch` numeric arms + `repository.rs` COALESCE patch; additive, no caller breaks.
- **ITEM-3** — verdict: CONCERN — extends `VoiceCapability`, a generated response schema → requires `just openapi-regen` (ITEM-6) or the `types_ts_parity` golden fails; tracked, not blocking.
- **ITEM-4** — verdict: PASS — new route mirrors `transcribe.rs`; reuses `VoiceTranscribe` gate (no new permission) and the existing runtime; distinct 409 on `streaming_enabled=false`.
- **ITEM-5** — verdict: PASS — sharing `forward_to_whisper` is a visibility change (`pub(super)`) + a caller-supplied timeout param; the batch caller keeps 300 s (byte-identical behavior), the stream caller passes a bounded interim timeout. No clamp/window logic (full-buffer re-decode, DEC-2).
- **ITEM-6** — verdict: CONCERN — mechanical regen; must run for BOTH binaries and be committed, else parity golden + desktop client drift. Standard, tracked.
- **ITEM-7** — verdict: PASS — mirrors the existing store state machine + supersession guard; the timeslice change is backward-compatible; interim path is additive and self-contained.
- **ITEM-8** — verdict: PASS — pure helpers mirroring the existing `voiceLogic.ts` split; no browser deps; directly unit-testable.
- **ITEM-9** — verdict: PASS — additive transient UI + a localStorage-backed toggle reusing the existing pref precedent; gating unchanged. New render state → needs a gallery cell (ITEM-11), flagged.
- **ITEM-10** — verdict: PASS — two fields (`streaming_enabled` Switch + `stream_interval_ms`) in the existing react-hook-form/zod card; bounds mirror the backend; `VoiceConfig.store` already generic.
- **ITEM-11** — verdict: CONCERN — a NEW conditional render state (recording-with-interim) trips `check:state-matrix`; the gallery cell is mandatory, not optional — budgeted as its own item so phase 8 stays green.

No `BLOCKED` verdicts. The two `CONCERN`s (ITEM-3/6 regen, ITEM-11 gallery) are
sequencing obligations already captured as items/tests, not plan defects.
