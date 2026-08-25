# TESTS — background-spawn-loop-guard

Bipartite coverage: every ITEM ↔ ≥1 TEST; every INV ↔ ≥1 `[acceptance]` test.
Backend-only diff → no `tier: e2e` requirement, no permission introduced → no
`[negative-perm]` spec. All tests are backend (unit + integration).

- **TEST-1** (tier: integration) [acceptance] [invariant: INV-1] [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/server/tests/background_mcp/spawn_guard.rs` — asserts: with an identical NON-TERMINAL background run already present for a conversation (seeded `job_kind='subagent'`, `status='running'`, matching `inputs_json`), a second `spawn_background` of the same spec via the real `/api/background/mcp` returns a clear "already running/queued" result carrying the EXISTING run_id and creates NO second row (workflow_runs count for the conversation is unchanged = 1).

- **TEST-2** (tier: integration) [acceptance] [invariant: INV-2] [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/server/tests/background_mcp/spawn_guard.rs` — asserts: with the conversation already at the cap of non-terminal background runs (seed `fan_out_max_threads` = default 6 DISTINCT-spec `status='running'` rows), a further `spawn_background` of a NEW distinct spec is refused with a clear over-cap error (`BACKGROUND_SPAWN_CAP_EXCEEDED`) and creates NO row (count unchanged); a positive control below cap still spawns.

- **TEST-3** (tier: integration) [acceptance] [invariant: INV-3] [covers: ITEM-1, ITEM-4] file: `src-app/server/tests/background_mcp/spawn_guard.rs` — asserts: with a RECENTLY-COMPLETED identical run present (seeded `status='completed'`, `created_at=now()`, matching `inputs_json`) — the state after a `[Background task complete]` re-injection — a re-spawn of the same spec (what the re-engaged model attempts) is refused as a duplicate and creates NO new run, so completion feedback yields no new run.

- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/background_mcp/resume.rs` — asserts: `build_resume_message(...)` contains no spawn-inducing directive (no `spawn_background` / "spawn a background" instruction) — the completion injection is DATA + a continue instruction, never a spawn trigger.

- **TEST-5** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/background_mcp/spawn_guard.rs` — asserts: the FIRST spawn of a spec (no prior run) succeeds (`status: "pending"`, real run_id, one row created) — the positive control proving the guard does not block legitimate first spawns (mirrors the "count must MOVE" control in spawn_contract.rs).

- **TEST-6** (tier: integration) [covers: ITEM-1, ITEM-2] file: `src-app/server/src/modules/workflow/repository.rs` (`spawn_background_run_drives_to_terminal`, updated) — asserts: `spawn_background_run` with `guard: None` still drives a background run to terminal `completed` (unchanged detached/scheduler behavior; `BackgroundSpawnResult::Spawned`).

## Coverage map
- ITEM-1 → TEST-1, TEST-2, TEST-3, TEST-6
- ITEM-2 → TEST-1, TEST-2, TEST-5, TEST-6
- ITEM-3 → TEST-1, TEST-2, TEST-5
- ITEM-4 → TEST-3, TEST-4
- INV-1 → TEST-1 [acceptance]
- INV-2 → TEST-2 [acceptance]
- INV-3 → TEST-3 [acceptance]
