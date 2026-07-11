# BASE — conflict-surface scoping (follow-up #3)

Branch base: `origin/khoi` @ `ab6127e1` (the merge of #134). Branch reset `--hard` to it,
so the new PR carries ONLY this follow-up. PR target: `khoi`. Gate base ref: `origin/khoi`.

## What current base touches that this branch also touches
- **UI-only** (`src-app/ui/**`). No backend, no migration, no OpenAPI type change.
- Highest migration `..153` — none added → no collision.
- No `openapi.json` / `api-client/types.ts` regen.
- `src-app/desktop/ui` has no hand-written override of the touched files (aliases
  `../../ui/src`); only the `ui` workspace is touched.
- The edited files (`ConversationPage.tsx`, `ToolCallPendingApprovalContent.tsx`, the
  07-mcp e2e spec) are #133/#134 code; no other active worker is known to touch them.

## Files added/edited
All under `src-app/ui/` (see PLAN.md). A `check:state-matrix` regen
(`stateMatrix.generated.ts` + `STATE_MATRIX.md`) may be required after editing
`ConversationPage.tsx` (a new effect/branch) — mechanically generated, covered in
AUDIT_COVERAGE.tsv.
