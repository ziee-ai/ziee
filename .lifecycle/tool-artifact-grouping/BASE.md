# BASE — conflict-surface scoping (follow-up round)

Branch base: `origin/khoi` @ `83e94a6a` (the merge of #133 — this branch's own prior
work). Branch reset `--hard` to it, so the new PR carries ONLY the two follow-up fixes.
PR target: `khoi`. Gate base ref: `origin/khoi`.

## What current base touches that this branch also touches
- **UI-only** (`src-app/ui/**`). No backend, no migration, no OpenAPI type change.
- Highest migration `..153` — this branch adds none → no collision.
- No `openapi.json` / `api-client/types.ts` regen.
- `src-app/desktop/ui` has no hand-written override of the touched files (aliases
  `../../ui/src`); only the `ui` workspace is touched.
- The files edited (`toolRun.ts`, `extension.tsx`, `ToolCallPendingApprovalContent.tsx`)
  are the ones #133 already introduced/edited — no other active worker is known to touch
  them.

## Files this branch adds/edits
All under `src-app/ui/` (see PLAN.md "Files to touch"). A `check:state-matrix` regen
(`stateMatrix.generated.ts` + `STATE_MATRIX.md`) may be required after editing the
touched components / adding a gallery state (mechanically generated — see
`.lifecycle` notes; cover in AUDIT_COVERAGE.tsv).
