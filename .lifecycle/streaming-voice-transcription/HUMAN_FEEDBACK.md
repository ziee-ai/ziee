# HUMAN_FEEDBACK — streaming-voice-transcription

Living ledger of human review feedback (recorded VERBATIM when given), plus the
audit-surfaced product tradeoff raised for the human's confirmation.

- **FB-1** [status: resolved] — (audit-surfaced tradeoff, raised for your confirmation) The Phase 6 blind audit flagged that full-stitched interim re-decodes the WHOLE accumulating clip every tick — O(n²) work on the single shared whisper-server and an authenticated-compute amplification vector (several concurrent live-caption users can slow batch dictation). → First applied the graceful-degradation fixes (transient 503 not 500 on a slow/failed interim tick, the caption strip shows the live tail, `capability.streaming_enabled` gated on runtime-readiness). **You then chose "add a decode-window cap"** → implemented an admin-configurable `stream_max_decode_secs` (default 30s, bounds 5..=600, migration 153): the server clamps each interim clip to its trailing window (`clamp_wav_tail`) before decoding, so per-tick whisper cost is bounded regardless of recording length. Clips at/under the window stay fully stitched (typical dictation); longer ones show the recent window while recording; the FINAL on-stop decode is always the complete, unclamped clip. [generalizable: yes — when the blind audit surfaces that a feature's cost/behavior conflicts with a user's explicit product decision, surface it to the human as a tracked item rather than silently reversing the decision.]

_No human review feedback on the RUNNING feature has been received yet — this file
exists from the start of the lifecycle per the Phase 9 discipline and will be updated
verbatim the moment the human reviews the running feature (including any override of
FB-1's interim resolution)._
