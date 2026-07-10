# BASE — conflict-surface scoping (streaming-voice-transcription)

Branch base: `origin/main` @ `af44c73c2` (already contains the merged voice module).

## Migrations

- Highest existing migration: `00000000000152_grant_voice_permissions_to_users.sql`.
- This branch adds exactly **one**: `00000000000153_add_voice_streaming_settings.sql`
  (ALTER `voice_runtime_settings`, additive columns with defaults — no data
  backfill, no destructive change).
- Collision risk: another in-flight branch could also claim `153`. The merge-gate
  (C2) re-checks migration-number collisions against real main at merge time; if
  main advances past 152 before merge, renumber to the next free slot.

## Files main also touches

The voice module was **just merged** and is not under active concurrent change on
main. The files this branch edits are voice-owned
(`modules/voice/*`, `modules/chat/extensions/voice/*`, `modules/voice/*` UI). Low
cross-branch collision risk. No shared-harness edits (B3): the streaming
integration tests reuse the existing `stub_whisper_binary()` + `tests/voice/`
helpers; no change to `tests/common/*`, the gallery cassette, or
`playwright.*.config`.

## OpenAPI regen — YES (required)

This branch adds a route (`POST /api/voice/transcribe/stream` →
`Voice.transcribeStream`) and changes three response/request schemas
(`VoiceCapability`, `VoiceSettings`, `UpdateVoiceSettingsRequest`). `just
openapi-regen` must run and regenerate **both** `src-app/ui/` and
`src-app/desktop/ui/` (`openapi.json` + `api-client/types.ts`). The
`emit_ts::tests::types_ts_parity` golden test is the backstop. Per the lifecycle,
the generated `openapi.json`/`types.ts` are excluded from the phase-6 coverage law
and do not, by themselves, make the diff "UI work".

## Permissions

**No new permission.** Streaming reuses `voice::transcribe` (already granted to the
Users group by migration 152). Migration 153 grants nothing. Therefore A10's
new-permission trigger does not fire; a `[negative-perm]` e2e is still enumerated
(TEST-12) to prove the new live-caption surface stays hidden for an unpermitted
user, but it is defensive, not gate-forced.
