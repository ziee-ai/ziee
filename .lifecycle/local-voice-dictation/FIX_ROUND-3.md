# FIX_ROUND-3 — fix the round-2 re-audit findings, then re-audit (convergence)

## Fixes applied (the 2 LOW findings from the round-2 re-audit) — `dd00d373c`

- **[LOW] model-download shared temp path** (`model.rs`) — per-attempt unique temp name `ggml-<name>.bin.<uuid>.tmp`, so a concurrent admin-download + transcribe-autostart of the same absent model no longer interleave byte streams into a spurious sha256 mismatch. Cleaned up on every error path; atomic rename publish unchanged.
- **[LOW] stopRecording onstop hang** (`Voice.store.ts`) — captured the supersession token before the finalization await + added a `setTimeout(settle, 1500)` fallback so the promise always settles even if a concurrent Cancel nulled `onstop` (no hung frame / closure leak), and a post-await gen check so a cancel during finalization doesn't resurrect `transcribing`.

## Re-audit (round 3 → full blind convergence round)

A fresh blind agent verified BOTH fixes correct + complete across all required properties (per-attempt temp isolation + cleanup + atomic publish; the gen-token + fallback closing the hang, the Stop-then-Cancel leak, and the resurrect-after-cancel paths, with no double-transcription and no wrong-blob). The holistic sweep of the ENTIRE voice diff (transcribe caps + WAV validation, argv hardening + loopback verification, auto-start single-flight + flap machine, the `voice::admin::{read,manage}` vs `voice::transcribe` authz split, download-param path-traversal guards, the delete in-use guard, download-task serialization, sync audiences) found **no confirmed defect of any severity**.

One **suspected** (not confirmed) low latent race was noted — a sub-millisecond `stopRecording` re-entry window (max-clip auto-stop racing a user Stop) that could fire a spurious "no audio" toast ~1.5 s after a successful transcription, with no data loss and not reachable by ordinary interaction. Although only suspected (not a convergence blocker), it was **closed defensively** (`6fd63b2b9`) with a `finalizing` in-progress latch on `stopRecording` (cleared after finalization + on cancel).

**New confirmed findings:** 0
