# FIX_ROUND-1 — streaming-voice-transcription

Merged the Phase 6 blind-audit ledger (12 angles, 4 blind reviewers) and fixed
every confirmed finding. Rejected findings (`perms-authz`, `patterns-conformance`,
`a11y` = "no defects found") need no action.

## Confirmed findings → fixes

- **error-handling / correctness (interim timeout → 500)** — `stream.rs` now maps a
  failed/timed-out interim `forward_to_whisper` to a transient **503
  `VOICE_INTERIM_UNAVAILABLE`** (logged at debug), not a 500. A slow tick on a long
  buffer is expected; the client single-flights and skips that caption.
- **api-contract (capability omitted runtime readiness)** — `get_capability` now sets
  `streaming_enabled = can_transcribe && settings.streaming_enabled` (was
  `enabled && …`), so the interim loop can never be advertised against an
  unprovisioned runtime/model. Field doc updated (`models.rs`).
- **concurrency/security (misleading "flood" comment + amplification)** — corrected the
  `validate_settings_patch` comment to describe `stream_interval_ms` accurately (a
  client cadence hint, not a server gate). The amplification RESIDUAL is documented in
  DEC-2 (graceful-degradation + accepted-residual) and raised in HUMAN_FEEDBACK FB-1
  for the user's call on a stronger bound.
- **state-management (caption froze on opening words)** — `MicButton` live-caption strip
  now shows the **tail** (newest words) of the stitched transcript via a `dir="rtl"`
  overflow box with an inner `<bdi dir="ltr">`, instead of `truncate` clipping the end.
- **perf (O(n²) full re-decode)** — inherent to the user's DEC-2 full-stitch choice; made
  to degrade gracefully (503 + tail caption) and documented + escalated (FB-1). Not a
  correctness defect; client interim errors are non-fatal and the final decode is
  unaffected.
- **tests-quality (streaming-toggle count race)** — `streaming-toggle.spec.ts`
  `recordBrieflyThenStop` now waits for the FINAL transcript to land in the composer
  (the real completion signal) before asserting counts, and uses `expect.poll` for
  `streamCount`; removes the `voice-elapsed`-only race.
- **tests-quality (gold-smoke soft-skip gap)** — `streaming_real_test.rs` now soft-skips
  (with a marker) when the whisper-server binary download OR the base.en model
  provisioning (batch warmup ≠ 200) is unreachable, not just on the GitHub release probe
  — so an HF-unreachable / asset-less box soft-skips instead of hard-failing.

## Post-fix verification

- `cargo check -p ziee` — clean. OpenAPI + `api-client/types.ts` regenerated for BOTH
  binaries (the capability doc-comment change flows to the spec). `tsc --noEmit` (ui +
  desktop) — clean. `lint:logical-direction` / `lint:colors` / `lint:guardrails` /
  `check:testid-registry` — green.

## Re-audit (fresh blind round)

A second blind round (fresh reviewers, diff-only) was run over `git diff main...HEAD`
after the fixes; findings recorded below.

**New confirmed findings:** 0
