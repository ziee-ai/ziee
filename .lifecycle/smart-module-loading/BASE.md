# BASE — conflict surface

**Base branch:** `feat/frontend-perf` (this effort's branch; builds on the
Stores.X removal + the lazy module glob + chat-extension deferral already
committed here). NOT branched off main — this feature is the continuation and
supersedes Part 1's ad-hoc lazy glob with a manifest-driven loader.

**Scope:** frontend + build-config ONLY. No backend, no migrations, no OpenAPI.

- Highest migration touched: **none** (no `src-app/server/migrations` change).
- OpenAPI regen implied: **no** (no Rust type change).
- Files main is actively changing that we also touch: the module loader
  (`src-app/ui/src/modules/loader*.ts`) and the SDK module-system
  (`sdk/packages/framework/src/module-system/*`, `module.ts`). These are stable;
  low collision risk. The SDK change rides the `chat-lazy-store-extension`
  submodule branch (same as the rest of this effort).
- New build dependency: a Vite plugin (in-repo, no npm dep) under
  `src-app/ui/vite/` — additive, no collision.

**Supersedes on this branch:** the `import.meta.glob('./**/module.tsx')` lazy
glob in `loader.ts` (commit 983e7be03) is replaced by the manifest-driven loader;
the chat-extension deferral (d328d3770) stays (complementary).
