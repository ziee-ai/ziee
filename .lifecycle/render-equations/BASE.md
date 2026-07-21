# BASE — render-equations (P3 conflict-surface scoping)

## Branch / base refs

- Worktree: `/data/khoi/home-workspace/ziee/wt-render-equations`
- Branch: `fix/render-equations`, cut off **`origin/khoi`** (not `origin/main`).
  Platform-wide UI fix → PR targets **`khoi`**.
- `git rev-list --left-right --count origin/main...HEAD` → **`4  0`**. `origin/main`
  is 4 commits ahead; HEAD carries nothing main lacks, i.e. **merge-base(main, HEAD)
  == HEAD**. So `git diff origin/main...HEAD` is currently EMPTY and will contain
  exactly this feature's changes once committed — the phase-6 coverage law and the
  phase-3/8 touched-workspace derivation both compute cleanly with no stale-branch
  noise. No rebase needed.
- `origin/khoi` HEAD: `6ca93f123` (Merge PR #182, `fix/reference-display`).

## Migration collisions

**None — this change adds no migration.** Note the server no longer carries
`src-app/server/migrations`; migrations now live per-crate under the `sdk`
submodule (`sdk/crates/ziee-{auth,file,notification,onboarding,seed}/migrations`)
plus `src-app/desktop/tauri/migrations`. Nothing in this feature touches any of
them, so there is no migration-number surface to collide on.

## OpenAPI regen

**Not implied.** No Rust type, route, or response shape changes. `openapi.json` and
`api-client/types.ts` are untouched in both `src-app/ui` and `src-app/desktop/ui`,
so `just openapi-regen` is not required and the C3 regen-parity merge gate is a
no-op for this branch.

## Files this branch will touch that main is also changing

Reviewed the five product files in PLAN.md's *Files to touch* against the 4
commits `origin/main` holds beyond HEAD:

- `src-app/ui/src/components/common/markdownPreprocess.ts` — **the one to watch.**
  It is shared by all markdown rendering, so it is the natural collision point if
  main lands another preprocessing change. Re-check at merge time.
- `src-app/ui/src/modules/chat/components/TextContent.tsx`,
  `modules/skill/components/SkillDetailDrawer.tsx`,
  `modules/workflow/components/StepOutputExpander.tsx` — single-line import + call
  wrap each; low collision surface.
- `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — moderate; a concurrent
  markdown change on main would plausibly touch this same spec.
- The three new files cannot collide.

`src-app/desktop/ui` does **not** mirror any of these (it has no `TextContent`, no
Streamdown call site, no `streamdownPlugins` module), so the R2-3 desktop-override
diff review is N/A for this feature.

## Environment prerequisites resolved at preflight

The worktree started with `agent-kit`, `sdk`, and `src-app/server/vendor/pgvector`
submodules uninitialized, no `node_modules`, and no hub-seed — every one of
`npm run check`, `gate:ui`, and `lifecycle-check.mjs` resolves through them.
Resolved before writing this plan: submodules initialized, `npm install` run at the
repo root (workspace hoist), hub-seed copied from the main checkout,
`config/dev.yaml` auto-seeded by preflight. `preflight.sh` now exits 0.
