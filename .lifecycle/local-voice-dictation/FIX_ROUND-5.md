# FIX_ROUND-5 — fix the e2e-spec-review findings, converge

## Fixes applied (`2bcd9703f`)

- **[MED×6] false error-state coverage** (`stateCoverage.ts`) — the 6
  `AvailableVersionsCard:empty/:error`, `InstalledVersionsCard:error`,
  `ModelCard:error`, `VoiceConfigCard:error`, `VoiceInstanceCard:error` entries
  were reworded from a false "covered by <spec>" to an HONEST tracked gap: the
  card **error** render-branches (a failed voice GET) are not driven by any spec
  (routeVoice serves 200s), so they are marked as a deferred gallery-cell gap
  (DRIFT-1) — not a coverage claim. The entries that ARE honestly exercised
  (happy-path / loading `:delayed` / the populated-empty `InstalledVersionsCard:empty`
  via `admin-empty-state.spec.ts`, and `MicButton:open` via the dictation spec)
  keep their true references.
- **[LOW] recording-ux flake** (`mic-recording-ux.spec.ts`) — widened
  `max_clip_seconds` 1 s → 3 s so the "Recording started" announcement + elapsed
  timer are asserted in a stable window, no longer racing the auto-stop that
  overwrites the shared live region; the auto-stop path is still exercised.

## Re-verification

These fixes are annotation-honesty (reason strings; no runtime behavior) + a
single test-timing widening (still asserts the same auto-stop + staged-transcribe
behavior). No product code changed, so there is no new behavior to blind-audit;
`npm run check` stays green with the corrected markers, and the spec still tests
auto-stop + the staged status. The prior blind rounds (1–4) already drove the
PRODUCT code to 0 confirmed defects.

**New confirmed findings:** 0
