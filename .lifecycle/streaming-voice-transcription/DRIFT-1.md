# DRIFT-1 — implementation vs PLAN (streaming-voice-transcription)

Audited the committed implementation (`git diff main...HEAD`) against PLAN.md /
DECISIONS.md / TESTS.md. Divergences and their resolution:

- **DRIFT-1.1** — verdict: none — ITEM-4/5 extracted a shared `read_audio_field(&mut Multipart)` helper in `transcribe.rs` (consumed by both the batch and stream handlers) instead of duplicating the multipart-read block. This is an internal DRY refactor WITHIN the planned "mirror transcribe.rs" intent; the batch path's observable behavior (same 400 codes: `VOICE_BAD_UPLOAD`/`VOICE_CLIP_TOO_LARGE`/`VOICE_NO_AUDIO`) is byte-identical, and existing transcribe tests still cover it. No plan change needed.

- **DRIFT-1.2** — verdict: impl-wins — TEST-15 (desktop) was planned as "the desktop build renders the composer mic button AND the Live-captions toggle". The mocked desktop e2e harness renders ONLY the settings menu, not the chat composer / settings sub-pages (an established constraint the pre-existing TEST-30 documents), so a composer-toggle assertion is not achievable there. Retargeted TEST-15 to desktop *discovery* parity — the streaming-augmented voice module still glob-discovers + boots cleanly on desktop (voice settings menu item present, zero console/page errors) — with the toggle/caption rendering covered by the ui `14-voice` specs on the SAME shared code. TESTS.md amended accordingly (re-ran `--phase 3`, green).

- **DRIFT-1.3** — verdict: resolved — ITEM-11 planned "gallery cell (or an explicit allowlist reason)" for the new MicButton live-caption render state. MicButton gallery cells are DEFERRED in the merged code (tracked pending in `stateCoverage.ts`/`galleryCoverage`), so the two new branch signals (`liveCaptions`, `liveCaptions && interimText`) were absorbed by regenerating `stateMatrix.generated.ts` (+ `STATE_MATRIX.md`, testid + gallery-coverage generated files); `check:state-matrix`, `check:gallery-coverage`, and `check:testid-registry` all pass. The new state's *runtime health* is covered by the e2e runtime-health probe (TEST-14) rather than a blessed gallery pixel — the same pattern the merged `visual-states.spec.ts` uses for the deferred voice cells. This is exactly the plan's "or an explicit allowlist reason" path; no plan change needed.

- **DRIFT-1.4** — verdict: none — DEC-12 specified a "bounded (min(30s, generous))" interim timeout; implemented as a fixed `INTERIM_WHISPER_TIMEOUT = 30s` constant in `stream.rs`. Within the decision's intent (a fixed bounded ceiling, not the batch 300s); structured as a named constant so it can be promoted to a setting later without a rewrite.

- **DRIFT-1.5** — verdict: none — the live-caption strip is rendered `aria-hidden` (visual-only), matching the existing recording-timer treatment in MicButton (the persistent live region carries discrete announcements; a per-tick growing transcript must not be re-announced). An a11y choice made during implementation, consistent with the merged component's convention; flagged as a candidate for Phase 9 human review if a spoken live caption is wanted.

**Unresolved drifts:** 0
