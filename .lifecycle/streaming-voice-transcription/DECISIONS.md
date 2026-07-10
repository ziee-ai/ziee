# DECISIONS — streaming-voice-transcription

Every human/product input the implementation needs, resolved up front — zero
unresolved markers.

### DEC-1: Streaming transport — chunked POST vs websocket vs the runtime proxy?
**Resolution:** Client-orchestrated **repeated multipart POST** to a new
`POST /api/voice/transcribe/stream`. No websocket, no SSE, no reverse proxy.
**Basis:** codebase — the app has NO websocket infrastructure (realtime sync and
download progress are SSE; there is no user-facing whisper proxy). whisper-server's
`/inference` is one-shot batch (a global mutex serializes calls), so SSE-from-one-call
doesn't map. Repeated POST reuses the exact `transcribe.rs` request/response +
`forward_to_whisper` path — the "reuse, don't invent" rule.

### DEC-2: whisper.cpp partial-decode approach — rolling re-decode vs sliding window?
**Resolution:** **Rolling re-decode of the accumulating buffer**, with a
**server-side tail-clamp** to `stream_window_secs` for interim decodes (bounding
whisper cost so tail ticks don't fall behind); the authoritative transcript is a
single **full-clip** decode on stop. The client sends the full accumulating WAV each
tick (dumb client); the server clamps the tail before `/inference`.
**Basis:** codebase + web research — whisper-server has no native streaming endpoint;
clamping raw 16 kHz mono PCM is a trivial, unit-testable RIFF `data`-chunk slice,
whereas slicing a MediaRecorder webm container client-side is not. Full-clip final
decode preserves today's accuracy.

### DEC-3: Interim vs final semantics — does the composer ever receive interim text?
**Resolution:** **No.** Interim results render only in a transient live-caption
**preview strip**; the composer receives ONLY the authoritative full-clip transcript
on stop (via the unchanged `appendTranscript` path). Interim is windowed/recent-word
preview, not stitched committed text.
**Basis:** convention + correctness — preserves the merged "insert, never send /
review-before-send" contract cleanly; volatile interim text rewriting the composer
input would flicker and muddy review. (Full-utterance stitched interim is a possible
future enhancement — flagged for human review, DEC-11.)

### DEC-4: How does live mode coexist with / toggle against batch mode?
**Resolution:** Two layers. (a) Admin `streaming_enabled` (settings row) = live
captions available deployment-wide. (b) A per-device user pref "Live captions"
(localStorage `ziee.voice.liveCaptions`) = this user wants interim captions;
opt-out falls back to today's batch. **On-stop behavior is identical in both modes**
(full-clip decode → composer), so review-before-send holds either way. Live mode is
purely additive: it overlays interim captions while recording.
**Basis:** convention — mirrors the existing per-device localStorage pref precedent
(`ziee.voice.privacyHintDismissed`); the admin toggle mirrors the existing `enabled`
runtime toggle. Keeps ONE mic button (no second surface).

### DEC-5 (Configurable-settings rule): cadence + window + enable — fixed or admin-configurable?
**Resolution:** **Admin-configurable**, added to the existing `voice_runtime_settings`
singleton via migration 153: `streaming_enabled BOOL DEFAULT TRUE`,
`stream_interval_ms INT DEFAULT 1000 CHECK (300..=10000)`,
`stream_window_secs INT DEFAULT 15 CHECK (2..=120)`. Read-at-use, GET/PUT gated by the
existing `voice::admin::{read,manage}`, synced via the existing `VoiceSettings` sync
entity, bounds-validated in `validate_settings_patch`, surfaced in `VoiceConfigCard`.
Also mirrored into `VoiceCapability` (non-admin read) so the composer can run/tune
live mode without an admin call.
**Basis:** the mandatory configurable-settings DEC rule — cadence and window are
operational tunables (an operator on slow hardware must lengthen the interval /
shorten the window); default to admin-configurable following the established voice
settings pattern.

### DEC-6: New endpoint vs reuse `/voice/transcribe` for interim?
**Resolution:** A **dedicated** `POST /api/voice/transcribe/stream`. It requires
`streaming_enabled`, applies the server-side tail-clamp, and does NOT hard-fail on
`max_clip_seconds` (an interim buffer legitimately grows toward the cap). It reuses
`forward_to_whisper` and the whole runtime.
**Basis:** codebase — the repo favors explicit routes; a distinct interim contract
(clamp + no clip-length hard-fail + independent 409) is cleaner and separately
testable than overloading the batch route with a mode flag.

### DEC-7: Does streaming introduce a new permission?
**Resolution:** **No.** Both the interim (`/transcribe/stream`) and final
(`/transcribe`) endpoints gate on the existing `voice::transcribe`. The admin
settings reuse `voice::admin::{read,manage}`.
**Basis:** minimal surface — interim decoding is the same capability as transcribing.
No migration grant; A10's new-permission trigger does not fire (a defensive
`[negative-perm]` e2e is still enumerated, TEST-12).

### DEC-8: Streaming response type — new type or reuse `TranscriptionResponse`?
**Resolution:** **Reuse `TranscriptionResponse`** (`{ text, language, duration_ms }`).
The client knows a result is interim by the call site, so a `partial` marker is
redundant.
**Basis:** minimize OpenAPI churn; no new schema needed.

### DEC-9: Interim request concurrency + cancellation model?
**Resolution:** **Single interim request in flight at a time** (fire the next tick
only after the previous settles or is superseded); interim errors are **non-fatal**
(skip that caption update, keep recording); a late interim result is dropped via the
existing `requestGeneration`/`isSuperseded` token (the generated client has no
AbortSignal). Stop/cancel/unmount tears the loop down and clears `interimText`.
**Basis:** codebase — mirrors the merged `Voice.store.ts` supersession pattern
exactly; avoids stacking `InflightGuard`s / contending whisper's serialized decode.

### DEC-10: MediaRecorder capture for interim decoding?
**Resolution:** `recorder.start(stream_interval_ms)` (add a timeslice) so
`ondataavailable` accumulates `chunks`; each interim tick decodes the **whole
accumulating blob** (a valid container from the first chunk) via the existing
`recordedBlobToWav16k`, and the server clamps the tail. Never decode a lone
mid-stream chunk (webm chunks after the first lack headers).
**Basis:** MediaRecorder container semantics; reuses the merged WAV encoder verbatim.

### DEC-11: Interim caption default ON, and windowed-preview vs stitched — human-review items
**Resolution:** When `streaming_enabled`, the per-device Live-captions
pref **defaults ON** (opt-out to batch); the interim caption shows the **windowed
recent** decode (not a full stitched running transcript) — both chosen as the
shipped default and revisitable at human review. These are the two genuine
product/UX judgment calls; both are recorded here as the shipped default and are the
prime candidates for the Phase 9 HUMAN_FEEDBACK ledger if the reviewer prefers
opt-in and/or full-stitched interim.
**Basis:** user — deferred to plan approval / human review; a windowed default is the
lower-risk, lower-cost starting point and the on-stop composer text is always the
full authoritative transcript regardless.

### DEC-12: Interim whisper timeout?
**Resolution:** The interim `forward_to_whisper` uses a **bounded** timeout
(min(30 s, generous)) rather than the batch 300 s, so a slow tick can't wedge the
loop; the batch final decode keeps 300 s.
**Basis:** convention — interim must stay responsive; the shared `forward_to_whisper`
takes a caller-supplied timeout (ITEM-5).
