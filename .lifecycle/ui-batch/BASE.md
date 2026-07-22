# BASE — ui-batch (conflict-surface scoping, P3)

Branch `fix/ui-batch` cut from **origin/khoi @ 68af34059**
("Merge pull request #187 from ziee-ai/fix/collapse-border-overlay").

## Migrations

**None added.** Migrations are no longer a single `src-app/server/migrations/`
directory — they live per-module under
`src-app/server/src/modules/<module>/migrations/`. The globally highest file is
`202607146095_workflow_grant_permissions.sql`. This branch adds **zero**
migrations, so a numbering collision is structurally impossible.

## OpenAPI regen

**Not implied.** No Rust type, route, or permission changes; `openapi.json` and
`api-client/types.ts` are untouched in both `ui/` and `desktop/ui/`. (These are
also the files the audit-coverage law and the phase-3/8 frontend gates exclude as
mechanically generated — irrelevant here since neither moves.)

## Files this branch touches that main is also moving

Recent `origin/khoi` history over the same directories:

```
68af34059 Merge pull request #187 from ziee-ai/fix/collapse-border-overlay
f9071cd3f fix(chat): keep thinking/tool-call card borders crisp in collapsed messages
a4c30adb1 fix(ui): render LaTeX-delimited equations (\[ … \] and \( … \))
1e1f7b686 fix(chat): comma-separate citations, merge same-paper references (#167)
087aff54f fix(chat): navigate to the start page when the open conversation is deleted
```

Assessment per touched file:

- `ModelSelector.tsx` (user-llm-providers), `ChatInput.tsx`,
  `LeftSidebar.tsx`, `RecentConversationsWidget.tsx`, `NewChatPage.tsx` — none
  appear in the recent main commits above; the active work in `modules/chat` has
  been in message RENDERING (collapsible cards, math, citations), a disjoint
  area.
- `ConversationPage.tsx` / `useNavigateAwayOnDelete.ts` — **read but NOT edited**
  by this branch, and `087aff54f` (delete → start page) touched exactly that
  area. Since ITEM-7's change lives entirely in `NewChatPage.tsx`, there is no
  textual overlap; the semantic interaction (a `reset()` on the `/chat` route vs
  that commit's navigate-away-on-delete, which already calls `reset()` itself) is
  reviewed in PLAN_AUDIT.md rather than left implicit.
- **Highest-churn shared file: `src/dev/gallery/coverage.ts`** plus the
  generated `stateMatrix.generated.ts` / `galleryCoverage.generated.ts`. Every
  UI branch that adds a component appends here, so this is the one realistic
  textual conflict surface. Mitigation: append a single line and REGENERATE the
  derived files rather than hand-editing them, so a conflict resolves by re-running
  `gen:gallery-coverage` + `gen:state-matrix`.

## Shared infrastructure NOT modified (B3)

`tests/common/*`, the gallery cassette, `playwright.*.config.ts`, and the build-DB
helper are untouched. The new specs are additive files under
`tests/e2e/visual/` and `tests/e2e/14-split-chat/`, both existing directories with
established sibling specs.

## Environment

`preflight.sh --repo <worktree>` → **OK**. Getting there required two one-time
worktree setup steps (neither is a source change): `git submodule update --init`
(`agent-kit`, `sdk`, `pgvector` were all uninitialized, leaving `@ziee/kit`
unresolvable and `.claude/skills/*` dangling), and seeding
`src-app/server/binaries/hub-seed/` from the main checkout's cache.
