# FIX_ROUND-2 — fix the round-1 re-audit findings, then re-audit

## Fixes applied (the 3 findings from the round-1 re-audit) — `0d48c4ff4`

- **[HIGH] deploy kill switch bypassable** (`mod.rs`) — `VoiceModule` now stores `enabled` (set in `init` from config) and `register_routes` early-returns the router unchanged when disabled, so `voice: { enabled: false }` unmounts the ENTIRE voice surface (transcribe/capability/admin/version/instance) → a `voice::transcribe` user gets 404 and no whisper-server can ever be spawned. Mirrors the `control_mcp` §16 pattern.
- **[MED] aria-live region was per-branch** (`MicButton.tsx`) — hoisted ONE stable `aria-live`/`aria-atomic` `sr-only` node to a fixed position-0 child of the top-level Fragment, with the state UI moved into a `content` variable rendered after it. The live-region DOM node now persists across every transition (only its text changes) so announcements fire on a mutation, not a remount.
- **[MED] transcribe-success had no supersession guard** (`Voice.store.ts`) — captured `gen = requestGeneration` before the transcribe await; both the success and catch paths drop the result when `requestGeneration !== gen` (a cancel/unmount during `transcribing` bumps the token), so a resolved POST no longer appends into a left conversation.

## Re-audit (round 2 → full blind round, backend + frontend)

Two fresh blind agents verified all three fixes **correct and complete** (kill-switch gating confirmed across the module lifecycle + all mount points; live-region node confirmed stable at position 0; gen-guard confirmed to fire on unmount-during-transcribing with no wrongful-drop window). The holistic sweep surfaced two NEW low-severity, self-healing races not previously caught:
1. **[LOW]** Model download used a fixed `<name>.tmp` path (no lock) — a concurrent admin-download + transcribe-autostart of the same model could interleave into a spurious sha256 mismatch (fail-closed, self-heals on retry).
2. **[LOW]** `stopRecording`'s onstop-finalization promise could hang (leaking its closure) if a concurrent Cancel nulled `onstop` during the MediaRecorder finalization gap.

**New confirmed findings:** 2
