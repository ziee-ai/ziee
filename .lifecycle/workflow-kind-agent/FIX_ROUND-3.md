# FIX_ROUND-3 — workflow-kind-agent

Round-3 blind re-audit (backend atomicity rework from round-2). It verified the lock/unlock key
identity, block_in_place wrapping, cross-workflow sweep isolation, the failed-DB-commit 500 path, and
the disk-source-of-truth honesty — and found **3 new confirmed findings (0 high, 2 medium, 1 low)** in
the advisory-lock + sweep I had just added. All fixed.

## New findings this round + resolution (backend `dev.rs`)
- **R3-1 (MED) + R3-3 (LOW) — DB advisory lock leaked on future-drop/panic** (manual unlock skipped;
  sqlx pool-return ≠ connection-close, so the session held the lock → per-workflow deadlock until
  restart; unlock Result also discarded/unlogged). **Fixed:** replaced the DB advisory lock with an
  **in-process keyed async mutex** — `static WORKFLOW_DEF_LOCKS: Lazy<DashMap<Uuid, Arc<Mutex<()>>>>`
  (reusing code_sandbox's `CONVERSATION_LOCKS` idiom). The guard auto-releases on Drop (success, error,
  client-disconnect future-drop, panic) → leak-free. Deterministic removal via
  `remove_if(strong_count == 1)` under the DashMap shard lock (race-safe vs a waiter). This is also
  architecturally correct: the bundle it guards is node-local disk, so a cross-node DB lock was wrong.
- **R3-2 (MED) — sweep destroyed the sole surviving bundle.** A `.old-<uuid>` is the only bundle copy
  if a prior update crashed between the two swap renames (bundle_root missing); the sweep deleted it →
  permanent loss. **Fixed:** the sweep now RECOVERS — if `bundle_root` is missing and a `.old-*` exists,
  it promotes the newest (by mtime) `.old-*` back to `bundle_root` (logged), and it deletes `.old-*`
  ONLY after `bundle_root` exists; if recovery fails it returns without deleting any `.old-*`.

## Build after round-3 fixes
cargo check `-p ziee` clean.

**New confirmed findings:** 3
