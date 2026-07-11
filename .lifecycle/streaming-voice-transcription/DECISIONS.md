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
**Resolution:** **Rolling FULL re-decode of the entire accumulating buffer** each
tick (no tail-clamp / no window). The client sends the full accumulating WAV; the
server forwards the whole buffer to `/inference` and returns the full transcript, so
the caption is a coherent **stitched** running transcript with zero manual
stitching (whisper decodes the whole clip each time). The authoritative transcript is
the same full-clip decode on stop. Per-tick cost grows with clip length (O(n²)
overall) but is bounded by `max_clip_seconds` and single-flighted, so it degrades
gracefully (caption updates slow on long clips) instead of piling up.
**Basis:** user (chose "full stitched transcript" over windowed-recent) + codebase —
whisper-server has no native streaming endpoint; full re-decode gives a fully-stitched
transcript for free (no commit-prefix heuristics, no client-side PCM/webm slicing),
and dictation clips are short + capped, so the O(n²) tail cost is acceptable. Known
tradeoff: whisper may revise earlier words between ticks (mild caption flicker); an
LCP/commit-prefix stability filter is a future enhancement if the reviewer flags it.

**Graceful-degradation (from the Phase 6 blind audit).** The full re-decode's cost
has real failure modes that are now handled so the feature degrades gracefully rather
than faulting: (a) an interim decode that exceeds the bounded interim timeout (or
otherwise fails) returns a **transient 503 `VOICE_INTERIM_UNAVAILABLE`** (not a 500) —
the client single-flights and simply skips that caption, recording + the authoritative
final decode are unaffected; (b) the live-caption strip shows the **tail** (newest
words) of the stitched transcript via a `dir="rtl"` overflow box, so a long transcript
stays live instead of freezing on its opening words; (c) `capability.streaming_enabled`
is gated on `can_transcribe`, so the interim loop never starts against an unprovisioned
runtime. **Cost bound (FB-1, user-approved):** the reviewer chose to add the stronger bound, so
each interim clip is **server-side clamped to its trailing `stream_max_decode_secs`
window** (`clamp_wav_tail` in `stream.rs`; admin-configurable, default 30s) before
decoding — per-tick whisper cost is now bounded regardless of recording length,
protecting the shared engine under concurrent live-caption users. Clips at/under the
window are decoded whole (fully stitched — typical dictation is unaffected at the 30s
default); longer ones show the recent window while recording. The FINAL on-stop decode
(batch `/transcribe`) remains unclamped, so the composer always gets the complete
transcript. This partially walks back full-stitch on long (>window) clips, per the
user's explicit FB-1 decision.

### DEC-3: Interim vs final semantics — does the composer ever receive interim text?
**Resolution:** **No.** Interim results render only in a transient live-caption
**preview strip** (the full stitched running transcript); the composer receives ONLY
the authoritative full-clip transcript on stop (via the unchanged `appendTranscript`
path).
**Basis:** convention + correctness — preserves the merged "insert, never send /
review-before-send" contract cleanly; the volatile stitched preview stays out of the
editable composer until it is finalized on stop.

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

### DEC-5 (Configurable-settings rule): cadence + enable — fixed or admin-configurable?
**Resolution:** **Admin-configurable**, added to the existing `voice_runtime_settings`
singleton via migration 153: `streaming_enabled BOOL DEFAULT TRUE`,
`stream_interval_ms INT DEFAULT 1000 CHECK (300..=10000)`, and (FB-1)
`stream_max_decode_secs INT DEFAULT 30 CHECK (5..=600)`. Read-at-use, GET/PUT gated by
the existing `voice::admin::{read,manage}`, synced via the existing `VoiceSettings`
sync entity, bounds-validated in `validate_settings_patch`, surfaced in
`VoiceConfigCard`. `streaming_enabled` + `stream_interval_ms` are mirrored into
`VoiceCapability` (non-admin read) so the composer can run/pace live mode without an
admin call; `stream_max_decode_secs` stays server-side (the clamp runs on the server).
**Basis:** the mandatory configurable-settings DEC rule — the decode cadence and the
per-tick decode-window cost bound (FB-1) are operational tunables; default to
admin-configurable following the established voice settings pattern.

### DEC-6: New endpoint vs reuse `/voice/transcribe` for interim?
**Resolution:** A **dedicated** `POST /api/voice/transcribe/stream`. It requires
`streaming_enabled`, forwards the full accumulating buffer with a bounded interim
timeout, and does NOT hard-fail on `max_clip_seconds` (an interim buffer legitimately
grows toward the cap). It reuses `forward_to_whisper` and the whole runtime.
**Basis:** codebase — the repo favors explicit routes; a distinct interim contract
(no clip-length hard-fail + independent 409 + bounded timeout) is cleaner and
separately testable than overloading the batch route with a mode flag.

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

### DEC-11: Interim caption default + content style — resolved by user
**Resolution:** When `streaming_enabled`, the per-device Live-captions pref
**defaults ON** (opt-out to batch), and the interim caption shows the **full stitched
running transcript up to the `stream_max_decode_secs` window** (default 30s): clips
at/under the window are fully stitched (typical dictation); longer ones show the
recent window while recording (the FB-1 cost bound — see DEC-2). The on-stop composer
text is always the full authoritative transcript regardless.
**Basis:** user — chosen at plan review via AskUserQuestion (default-ON opt-out;
full-stitched over windowed-recent), then at Phase-9 review the reviewer chose to add
the decode-window cost bound (FB-1), which caps the fully-stitched window.

### DEC-12: Interim whisper timeout?
**Resolution:** The interim `forward_to_whisper` uses a **bounded** timeout
(min(30 s, generous)) rather than the batch 300 s, so a slow tick can't wedge the
loop; the batch final decode keeps 300 s.
**Basis:** convention — interim must stay responsive; the shared `forward_to_whisper`
takes a caller-supplied timeout (ITEM-5).

### DEC-13: Which whisper model does the real-voice (gold-smoke) test use?
**Resolution:** **`base.en`** (English-only, ~140 MB). The gold-smoke test
(`tests/voice/streaming_real_test.rs`, TEST-8) downloads the real `whisper-server` +
`base.en` and transcribes a committed short English speech WAV, hard-asserting the
final transcript contains the expected keywords (case-insensitive) and that ≥1
mid-recording interim response is non-empty. The **product default model stays
`base`** (multilingual, unchanged) — `base.en` is only the test model.
**Basis:** user — chosen at plan review. `base.en` is accurate enough to hard-assert
keywords yet lighter than `small`; whisper ggml models pull from the public
`ggerganov/whisper.cpp` HF repo (no API key), so it won't hit the placeholder-key
soft-skip.

### DEC-14: How is the real-voice test gated so it doesn't falsely fail CI?
**Resolution:** **Soft-skip at runtime**, mirroring `real_repo_test.rs`: it lives in
the default `voice::` suite, probes the external gate (the `ziee-ai/whisper.cpp`
release reachability) BEFORE any work, and early-returns with a
`SOFT-SKIP [external gate: whisper-release]` marker when the release / GitHub / model
download is unreachable. When reachable it runs for REAL with every step a hard
assertion — never `#[ignore]`.
**Basis:** codebase + [[feedback_no_ignore_unless_platform]] — the merged voice suite
already established exactly this soft-skip discipline for its real external
dependency; the gold-smoke reuses it rather than hiding behind an ignore attribute.

### DEC-15: Is custom / additional whisper-model download in scope?
**Resolution:** **No — out of scope for this feature.** Streaming reuses the existing
closed 4-model allow-list (`tiny/base/base.en/small`) exactly as the merged voice
module ships it; interim and final decodes both use the single admin-configured
`settings.model`. Letting admins download models beyond that set (a code change to
the pinned allow-list, or arbitrary/HF-by-name custom download with its own SSRF +
integrity threat model) is tracked as a **separate `feat/voice-custom-models`
lifecycle**, planned independently. This branch does not touch `model.rs`'s allow-list
or the download path.
**Basis:** user — decided at plan review to keep streaming lean and split custom-model
management into its own security-audited feature. Documents the scope boundary so the
streaming diff/audit stays focused.
