# FIX_ROUND-1 — background-spawn-loop-guard

Merged the round-1 blind ledger (3 angles: correctness, design-conformance,
concurrency-resource). Correctness: clean. Concurrency-resource: 1 LOW
(connection held while blocking on the advisory lock) → wontfix + documented
(bounded by the cap, spawn is not a hot path). Design-conformance: 1 HIGH + 1
MEDIUM, both fixed:

- **HIGH (INV-3 reachable violation) — FIXED.** The terminal-run dedup clause
  keyed off `created_at` (SPAWN time). A run longer than `SPAWN_DEDUP_WINDOW_SECS`
  (300s) already has `created_at` outside the window when it completes and
  re-injects `[Background task complete]`, so its immediate identical re-spawn
  escaped dedup — and, because such runs are sequential, never tripped the cap
  either: the exact >5min-task loop the feature must break. Fixed by keying the
  `completed`/`failed` clause off `updated_at` (which `mark_status` stamps
  `= NOW()` on the terminal transition = completion time).
  `src-app/server/src/modules/workflow/repository.rs`.
- **MEDIUM (hollow acceptance test) — FIXED.** The INV-3 acceptance test only
  seeded created_at=now / created_at=1h-ago and would have passed WITH the
  created_at bug present. `seed_bg_run` now controls `created_at` AND `updated_at`
  independently, and `completed_reinjection_does_not_respawn` now seeds the run
  as CREATED 1h ago but COMPLETED just now (the long-run case) — which fails under
  the bug and passes only with the `updated_at` fix. The negative control seeds
  both timestamps old (genuinely finished long ago → re-run allowed).
  `src-app/server/tests/background_mcp/spawn_guard.rs`.

Both fixes compile (`cargo check --test integration_tests` green). A focused blind
re-audit of the round's diff (below) confirms the fix and finds nothing new.

**New confirmed findings:** 0
