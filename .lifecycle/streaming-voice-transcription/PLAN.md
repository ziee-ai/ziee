# PLAN — streaming-voice-transcription

Real-time / streaming voice transcription: render **live interim captions** while
the user speaks, layered onto the already-merged batch push-to-talk dictation
(`feat/local-voice-dictation`, main `af44c73c2`). The interim caption is a
**transient preview**; on stop the existing authoritative full-clip decode is
inserted into the composer (review-before-send unchanged). Reuses the whisper
engine, `voice::transcribe` permission, the settings singleton, and the MicButton
surface — no parallel infra.

Mechanism (see DECISIONS): the client keeps recording with MediaRecorder, and while
recording fires a cadence loop that re-decodes the **entire accumulating** audio
buffer (client POSTs the full accumulating WAV each tick) against a new
`POST /api/voice/transcribe/stream` endpoint, rendering each result as a **full,
stitched** live caption (whisper decodes the whole clip each tick → one coherent
running transcript, no manual stitching). On stop, the existing
`POST /api/voice/transcribe` full-clip path produces the authoritative transcript
that lands in the composer. Transport is repeated multipart POST (the app has no
websocket infra; whisper `/inference` is batch-only). Per-tick decode cost grows with
clip length but is bounded by `max_clip_seconds` and single-flighted, so it degrades
gracefully (the caption updates less often on long clips) rather than piling up.

## Items

- **ITEM-1**: Migration `00000000000153_add_voice_streaming_settings.sql` — add `streaming_enabled BOOL NOT NULL DEFAULT TRUE`, `stream_interval_ms INT NOT NULL DEFAULT 1000 CHECK (300..=10000)`, and (FB-1) `stream_max_decode_secs INT NOT NULL DEFAULT 30 CHECK (5..=600)` to the `voice_runtime_settings` singleton.
- **ITEM-2**: Extend the settings DTOs + repository + validation: add the three fields to `VoiceSettings` (GET) and `UpdateVoiceSettingsRequest` (PUT), COALESCE-patch them in `VoiceRepository::{get_settings,update_settings}`, and add bounds checks to `validate_settings_patch` (mirroring the existing numeric-range arms).
- **ITEM-3**: Extend `VoiceCapability` (+ `handlers::get_capability`) with `streaming_enabled` (gated on `can_transcribe`) and `stream_interval_ms` so the composer can run/pace live mode via the non-admin `GET /voice/capability` (never an admin call). `stream_max_decode_secs` stays server-side only (the clamp is applied on the server).
- **ITEM-4**: New streaming endpoint `POST /api/voice/transcribe/stream` (`modules/voice/stream.rs`): gate `VoiceTranscribe`, require `settings.enabled && settings.streaming_enabled` (else 409), validate WAV + size caps (NOT the clip-length cap — an interim buffer legitimately grows toward it), clamp the clip to its trailing `stream_max_decode_secs` window (FB-1 cost bound), `ensure_running` + forward to whisper with a bounded interim timeout (DEC-12) mapping failures to a transient 503, return `TranscriptionResponse`. Register the route in `routes.rs` with the same 64 MiB body limit.
- **ITEM-5**: Make `forward_to_whisper` + `read_audio_field` + `validate_wav` shared (`pub(super)`, timeout param) so both the batch (300 s) and stream (bounded) handlers reuse them; add `clamp_wav_tail(wav, secs) -> Vec<u8>` (pure, unit-tested) that keeps the trailing `secs` of PCM (no-op when the clip is at/under the window or the header is unparseable).
- **ITEM-6**: Regenerate OpenAPI + `api-client/types.ts` for BOTH binaries (`just openapi-regen`): the new `Voice.transcribeStream` endpoint key + the changed `VoiceCapability`/`VoiceSettings`/`UpdateVoiceSettingsRequest` schemas land in `src-app/ui/` and `src-app/desktop/ui/`.
- **ITEM-7**: `Voice.store.ts` interim streaming loop: start MediaRecorder with a timeslice (`recorder.start(stream_interval_ms)`) so `chunks` accumulate; while `recording` and live mode is active, run a cadence loop that (single-flight, generation-guarded, interim-errors-non-fatal) builds the FULL accumulating blob → `recordedBlobToWav16k` → `ApiClient.Voice.transcribeStream(formData)` → sets `interimText` to the returned full running transcript. Tear the loop down on stop/cancel/unmount and clear `interimText`; the final full-clip decode + `appendTranscript` into the composer is unchanged.
- **ITEM-8**: `voiceLogic.ts` pure streaming helpers (unit-testable, mirroring the existing split): `shouldRunInterim(status, capability, livePref)`, `resolveLivePref(stored, streamingEnabled)` (per-device localStorage default follows `streaming_enabled`), `composeInterimCaption(text)` (trim; blank → cleared).
- **ITEM-9**: `MicButton.tsx` — a transient live-caption preview strip (shows the full stitched `interimText` while recording, `aria-live`-announced discretely, cleared on stop) + a "Live captions" toggle control (persists the per-device pref to `localStorage`), shown only when `capability.streaming_enabled`. Gating unchanged (hidden without `voice::transcribe` / capability / `isRecordingSupported`).
- **ITEM-10**: `VoiceConfigCard.tsx` + zod schema — two admin fields: `streaming_enabled` (Switch) and `stream_interval_ms` (300–10000), mirroring the existing numeric fields and the backend bounds. (`VoiceConfig.store` is already generic over the settings object.)
- **ITEM-11**: Gallery + `check:state-matrix` coverage for the new MicButton **live-caption** render state (recording-with-interim) and the Live-captions toggle, so `npm run check` / `gate:ui` stay green.

## Files to touch

Backend (`src-app/server/`):
- `migrations/00000000000153_add_voice_streaming_settings.sql` (new)
- `src/modules/voice/models.rs` (VoiceSettings, UpdateVoiceSettingsRequest, VoiceCapability)
- `src/modules/voice/repository.rs` (get_settings / update_settings)
- `src/modules/voice/handlers.rs` (validate_settings_patch, get_capability)
- `src/modules/voice/stream.rs` (new — streaming interim handler)
- `src/modules/voice/transcribe.rs` (make forward_to_whisper `pub(super)` + timeout param)
- `src/modules/voice/routes.rs` (register `/voice/transcribe/stream`)
- `src/modules/voice/mod.rs` (`mod stream;` if needed)
- `openapi/openapi.json` + `src/openapi/*` regen output (generated)

Frontend (`src-app/ui/`, shared by desktop via glob discovery):
- `src/modules/chat/extensions/voice/Voice.store.ts`
- `src/modules/chat/extensions/voice/voiceLogic.ts`
- `src/modules/chat/extensions/voice/components/MicButton.tsx`
- `src/modules/voice/components/VoiceConfigCard.tsx`
- `src/api-client/types.ts` (generated), `openapi/openapi.json` (generated)
- `src/dev/gallery/*` (state-matrix / coverage entries for the new state)
- `tests/e2e/14-voice/*` (new specs) + `voice-helpers.ts`

Desktop (`src-app/desktop/ui/`):
- `src/api-client/types.ts` + `openapi/openapi.json` (generated by the same regen)
- `tests/e2e/voice-desktop-surface.spec.ts` (extend for the live-caption surface)

## Patterns to follow

- **Streaming endpoint** — mirror `modules/voice/transcribe.rs` (multipart `file`
  ingest, WAV validate, `ensure_running` + `InflightGuard` + `forward_to_whisper`,
  `RequirePermissions<(VoiceTranscribe,)>`, `with_permission` docs). It is the
  closest existing module by construction.
- **Settings fields + validation** — mirror the existing numeric arms in
  `handlers::validate_settings_patch` and the COALESCE-patch in
  `repository.rs::update_settings`; the singleton-settings pattern is already
  established for voice (`code_sandbox_settings`/`session_settings` lineage).
- **Capability extension** — mirror the existing `VoiceCapability` field set +
  `get_capability` (non-admin, `voice::transcribe`-gated readiness snapshot).
- **Store streaming loop** — mirror the existing `Voice.store.ts` imperative
  module-scope resources + `requestGeneration`/`isSuperseded` supersession guard
  and the `appendTranscript` insert-not-send rule; extract pure logic into
  `voiceLogic.ts` exactly as the merged code did.
- **MicButton toggle + localStorage pref** — mirror the existing first-run
  privacy-hint Popover + `ziee.voice.privacyHintDismissed` localStorage precedent
  in `MicButton.tsx`.
- **Admin card fields** — mirror the existing `VoiceConfigCard` react-hook-form +
  zod field wiring (Switch + number inputs with min/max).
- **Integration tests** — mirror `tests/voice/transcribe_test.rs` (register the
  `stub_whisper_binary()` runtime + `stage_model`, post a fixture WAV) and
  `tests/voice/{settings,capability,permissions}_test.rs`.
- **Real-voice gold-smoke** — mirror `tests/voice/real_repo_test.rs`'s soft-skip
  discipline (probe the external `ziee-ai/whisper.cpp` release first; early-return
  with a `SOFT-SKIP [external gate]` marker if unreachable; otherwise download the
  real `whisper-server` + the `base.en` ggml model and hard-assert). Uses a
  committed short English speech WAV fixture (not silence).
- **e2e** — mirror `tests/e2e/14-voice/*` (mocked getUserMedia/MediaRecorder +
  cassette API) and `voice-helpers.ts`.
