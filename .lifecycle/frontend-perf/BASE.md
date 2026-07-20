# frontend-perf — BASE (conflict surface vs current main)

- Base commit: `origin/main` = `e2800d9a1` (verified via `git ls-remote`).
- Highest migration: `src-app/server/migrations-merged/202607150000_seed_ledger.sql`.
  **This feature adds NO migrations** — it is frontend-only. No migration collision.
- OpenAPI regen: **not implied** — no backend handler/type change. The generated
  `openapi.json` / `api-client/types.ts` are untouched.
- Files this branch touches that main may also touch: the chat markdown render
  path (`src/modules/chat/**`), the shell boot hooks (`sdk/packages/shell/**`),
  the vite config, and `src/utils/lazyWithPreload*`. These are stable areas; the
  merge-gate re-checks against real main at merge time.
- Workspaces touched: `src-app/ui` (primary). `src-app/desktop/ui` only if a
  shared change needs a desktop override diff (rule R2-3) — checked per item.
