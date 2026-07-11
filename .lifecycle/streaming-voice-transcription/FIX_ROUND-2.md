# FIX_ROUND-2 — streaming-voice-transcription (FB-1 decode-window cap)

The Phase-9 reviewer chose to add the decode-window cost bound (FB-1). That
increment (`stream_max_decode_secs` + `clamp_wav_tail`) was blind-re-audited
(security/robustness + correctness + api-contract). Correctness + api-contract:
clean. One security finding, fixed:

## Confirmed finding → fix

- **security (unchecked u32 multiply on user-controlled fmt)** — `clamp_wav_tail`'s
  `byte_rate = sample_rate * block_align` multiplied fmt-chunk fields that
  `validate_wav` never bounds (it only checks the RIFF/WAVE magic). A crafted WAV
  (`sample_rate=1e9, channels=2, bits=32`) overflows u32 → panics under
  overflow-checks (dev/test builds = the documented `cargo run` workflow), a
  per-request DoS for any `voice::transcribe` user; wraps in release. **Fix:**
  compute `block_align` overflow-free (`(bits/8)*channels`, ≤ 8191·65535) and
  `byte_rate` via `sample_rate.checked_mul(block_align)`, falling back to the
  whole-clip no-op on overflow/zero/no-data (whisper rejects a genuinely malformed
  WAV itself). New unit test `does_not_panic_on_overflowing_fmt` feeds the exact
  crafted fmt and asserts no panic + input returned.

All other arithmetic in `clamp_wav_tail` was traced clean by the auditor
(`want` saturating-mul; `start` guarded by the `data_len ≤ want` early-return;
header size casts bounded by the 256 MiB absolute upload cap). The batch path stays
unclamped; the interim-503 mapping + `can_transcribe`-gated capability from
FIX_ROUND-1 remain intact.

## Re-verify

`cargo test --lib voice::stream::tests` — 4 pass (incl. the overflow guard). The
fix is localized to the two arithmetic lines; the auditor's trace already confirmed
every other index/slice/arith op safe, so no new surface was opened.

**New confirmed findings:** 0
