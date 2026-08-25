# BASE.md — conflict-surface scoping

- **Branch**: `fix/subagent-task-cards-transcript` off `main` (@ `b1147a24`).
- **Highest existing server migration**: `202608250100`. This feature adds NO
  migration (pure frontend), so no migration-number collision is possible.
- **OpenAPI regen implied?** NO — no backend type/route change. `openapi.json` /
  `api-client/types.ts` are untouched. `BackgroundRunDetail.activity` is already
  exposed on main.
- **Files this branch edits** (all under `src-app/ui`):
  - `src/modules/background/components/BackgroundRunCard.tsx`
  - `src/modules/background/gallery.tsx`
  - `tests/e2e/15-background/background-transcript.spec.ts`
  - `tests/e2e/15-background/background-card-layout.spec.ts` (new)
- **Concurrent-edit note**: another agent is editing the `background_mcp` SPAWN
  path (`tools.rs`/`resume.rs`/`runs.rs`/`repository.rs`). This branch touches
  NONE of those (frontend-only), so no collision. No `routes.rs`/`handlers.rs`
  edit is needed because the transcript field is already served.
- **Desktop twin**: `src-app/desktop/ui` has NO `background` module — nothing to
  mirror (R2-3 N/A).
