# HUMAN_FEEDBACK — showcase-modular-seed

Living ledger of human feedback on this feature (verbatim → resolution).

The human approved the PLAN ("go ahead", 2026-07-13) with all three scope decisions
set to the recommended path (DEC-A full migration, DEC-B include desktop, DEC-C
hand-authored typed literals). The feature was then built end-to-end through the
8-phase lifecycle (plan → audit → tests → decisions → implement + drift → blind
multi-angle audit → fix/re-audit to 0 new → gated tests), and is 8/8 with both
workspaces' `npm run check` + `gate:ui` runtime canary green.

**no human feedback received** on the running feature yet — it has not been
human-reviewed since it became testable. This ledger fills (as `FB-N` entries,
verbatim) the moment the human reviews the running gallery and gives feedback.

## Candidate generalizable rules surfaced during the build (for the orchestrator's harvest)
Not human feedback — lessons from the blind audit + gate-integration work, offered
as fleet-wide rules the orchestrator may fold into the lifecycle/lints:
- Any gate that WALKS module source (`gen-*-coverage`, `gen-state-matrix`,
  `gen-testid-registry`, `gen-overlay-registry`) must exclude a new co-located
  authoring-metadata file (here `gallery.tsx`) — the plan anticipated only ONE
  such coupling; there were four.
- A "prod-exclusion" / tree-shaking check must grep for a RUNTIME string, never a
  comment (minifiers strip comments → the check is vacuous). The runtime marker
  here caught a real gallery-leak-to-prod the comment marker had hidden.
- A `gate:ui`-style per-surface verdict must subtract BOTH `baselined` AND
  `harness` findings (mirror the runtime-health gating formula) or it fails on
  dev-server noise that only surfaces via a worktree's symlinked node_modules.
- `isMain` guards in `.mjs` gate scripts must use `pathToFileURL(argv[1]).href`
  (the naive `file://${argv[1]}` silently disables the gate on Windows).
