# BASE — collapse-border-overlay

## Base branch (NON-STANDARD — read this first)

This branch is cut from **`origin/khoi`**, not `origin/main`, and the PR targets
**`khoi`**. `origin/khoi` is **382 commits ahead of `main`**.

Consequence: every lifecycle command MUST pass `--base origin/khoi`. The
validator defaults to `origin/main`, which would pull all 382 upstream commits
into `git diff base...HEAD` and make the Phase-6 coverage law
(every hunk reviewed by ≥3 angles) both infeasible and meaningless.

```bash
node .claude/lifecycle/lifecycle-check.mjs --phase <N> \
  --repo /data/khoi/home-workspace/ziee/wt-collapse-border \
  --base origin/khoi
```

`origin/khoi` head at branch time: `6ca93f123` (Merge PR #182). Branch was 0
commits ahead / clean at start.

## Conflict surface vs current khoi

- **Migrations** — none. This is a UI-only diff; no `src-app/server/migrations`
  file is added, so there is no migration-number collision to check.
- **OpenAPI regen** — NOT implied. No Rust type changes, so neither
  `openapi.json` nor `api-client/types.ts` is regenerated, and the diff is not
  treated as backend work.
- **Files also moving on khoi** — the touched files
  (`chat/components/{ChatMessage,CollapsibleBlock,collapsible}.tsx|ts`,
  `chat/gallery.tsx`, `dev/gallery/*`) were last moved by the F1/F2 SDK import
  repoints (`6071d350e`, `00801be2c`), which are already merged into the base.
  No active concurrent work identified on them.
- **Generated registries** — ITEM-5 regenerates the gallery registries
  (`galleryCoverage.generated.ts`, `stateMatrix.generated.ts`, seed/testid). These
  are high-churn shared files; if khoi moves under us, re-run the `gen:*` scripts
  rather than hand-resolving the conflict.

## Environment setup performed (fresh worktree)

A fresh worktree ships none of these; all were required before anything ran:

1. `git submodule update --init --recursive` — **`agent-kit`** (the
   `.claude/lifecycle` + `.claude/skills` symlink targets; without it the
   lifecycle tooling does not exist) and **`sdk`** (the `@ziee/*` workspace
   packages) and `src-app/server/vendor/pgvector`.
2. `npm install` at the repo root — must run **AFTER** the `sdk` submodule is
   present, since root `workspaces` includes `sdk/packages/*`. An install done
   before that silently omits all 9 `@ziee/*` packages.
3. `src-app/server/binaries/hub-seed/` copied from the main checkout
   (`/home/khoi/ziee/ziee`) — preflight treats a missing seed as blocking because
   the Rust build panics without it.
4. `src-app/server/config/dev.yaml` — auto-seeded by preflight with a fresh
   `jwt.secret`.

`preflight.sh` is green after the above.
