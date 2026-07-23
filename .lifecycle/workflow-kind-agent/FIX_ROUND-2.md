# FIX_ROUND-2 — workflow-kind-agent

Round-2 full blind re-audit (2 fresh diff-only agents on the round-1 fix commit). The FE fixes
(per-row type tracking, honest StepDef doc) were verified **clean — 0 findings**. The backend
staging/swap I added in round-1 drew **6 new confirmed findings** (0 high, 4 medium, 2 low), all in the
new atomicity/error-handling. Each is now fixed.

## New findings this round + resolution (all backend `dev.rs`/`runner.rs`)
- **R2-1/R2-2 (MED) — concurrent same-workflow updates not serialized; a mid-swap race could return
  200 without placing the def on disk.** Fixed: a per-workflow **Postgres session advisory lock**
  (`pg_advisory_lock(hashtextextended('wf-def-update:<id>',0))`) on a dedicated pooled connection wraps
  the whole validate→stage→swap→commit body; `existing` is re-read under the lock; unlock on every path
  (+ connection-close backstop). Same-workflow updates now serialize.
- **R2-3 (MED) — restore claimed success unconditionally (`let _ = rename`).** Fixed: the restore
  Result is checked; if the restore also fails, it logs that the live bundle is MISSING and returns
  **500 `WORKFLOW_UPDATE_BUNDLE_MISSING`** — never a false "restored" claim.
- **R2-4 (MED) — 200 returned on a known disk/DB divergence.** Fixed: reordered to disk-source-of-truth
  (atomic swap → DB `update_definition` LAST); every post-stage failure returns a 500
  (`WORKFLOW_UPDATE_SWAP_FAILED` / `..._BUNDLE_MISSING` / `..._METADATA_FAILED`). The only 200 is when
  disk AND DB are both consistently updated.
- **R2-5 (LOW) — copy_dir_recursive silently dropped symlinks/special files.** Fixed: it now uses
  `symlink_metadata` and REJECTS a symlink (`WORKFLOW_WORKSPACE_SYMLINK`) / special file
  (`WORKFLOW_WORKSPACE_SPECIAL_FILE`), matching `pack_workspace_dir_measured`.
- **R2-6 (LOW) — orphan `.staging-*`/`.old-*` dirs on a remove failure.** Fixed: a scoped, best-effort
  `sweep_orphan_siblings` (this workflow's name-prefix only, under the lock) self-heals leftover
  staging/old dirs at the start of each update.

## Build after round-2 fixes
cargo check `-p ziee` clean.

**New confirmed findings:** 6
