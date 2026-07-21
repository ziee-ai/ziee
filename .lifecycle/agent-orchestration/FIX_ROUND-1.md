# FIX_ROUND-1 — Phase-6 findings dispositions

All 17 blind-audit findings are dispositioned below (FIXED with a fail-on-revert regression test,
REFUTED with evidence, or ACCEPTED+documented). The fixes landed across 6 file-disjoint tranches
(FIX-A agent-core, FIX-B scheduler, FIX-C mcp+chat, FIX-D workflow+background_mcp, FIX-E code_sandbox,
FIX-F frontend), each verified: combined `cargo check -p ziee` green + `cargo check -p agent-core` green;
agent-core 100 lib tests (4 new); server 26 targeted lib tests + 21 background_mcp/scheduler integration
tests; FE `tsc` + `check:gallery-coverage` PASS. Every new regression test was confirmed to FAIL when its
fix is reverted (genuine, not vacuous).

**New confirmed findings:** 0

(Convergence basis: the fixes are surgical, each guarded by a fail-on-revert test; the combined build +
targeted test suites pass; no fix introduced a new failure. The remaining reds in the FE gate family —
state-matrix/overlay/override/testid registries — are byte-identical to `main` (pre-existing kit→SDK
drift; testid is SDK-boundary), not introduced by this branch, and are handled in Phase 8 / a separate SDK
workstream.)

## HIGH
- **H1 — agent-core delegate depth not structural** (core.rs/core_tools.rs) — **FIXED** (FIX-A). `handle_delegate` returns `is_error` without fan-out when `!parent_scope.allow_delegate` → an injected child can no longer spawn grandchildren; `max_depth=1` is now structural. task_*/schedule_next already self-guard via their ports (no gap). Test: `delegate_refused_when_allow_delegate_false`.
- **H2 — scheduler self-paced write-back not guarded by outcome.success** (tick.rs) — **FIXED** (FIX-B). Write-back gated on `outcome.success`; a failed firing keeps `record_outcome`'s authoritative failure/pause state (no more masking as 'completed'; failure cap honored). Test: `self_paced_failed_firing_does_not_mask_failure_as_completed`.
- **H3 — stale galleryCoverage; npm check fails** (dev/gallery) — **FIXED** (FIX-F). Regenerated `galleryCoverage.generated.ts` (390 surfaces: +14 new / −108 phantom kit→SDK) + `coverage.ts` entries + `GALLERY_SEED_MANIFEST.md`. `check:gallery-coverage` PASS. (Other registry checks remain pre-existing-red / SDK-boundary — Phase 8.)

## MEDIUM
- **M1 — per-tool approval override ignored for built-ins** (mcp.rs) — **FIXED** (FIX-C). Admin override consulted first (wins for all system servers incl built-ins); absent override ⇒ byte-identical bypass. Unit test.
- **M2 — override not on agent-core chat path** (gate.rs decide_pure) — **FIXED** (FIX-C). Override threaded into `decide_pure`. Unit test.
- **M3 — sandbox detached bypasses conv_lock** (code_sandbox/handlers.rs) — **FIXED** (FIX-E). `execute_command_detached` acquires the per-conversation `conv_lock` (RAII, released on every exit). Rootfs-free test.
- **M4 — orphaned background subagent never terminalizes** (workflow/job_kind.rs) — **FIXED** (FIX-D). `SUBAGENT_POLICY.orphan_sweep`=Fail → crashed bg run becomes terminal `failed` on boot. Terminal-state test.
- **M5 — compaction upto index mismatch** (agent-core/compaction.rs) — **VERIFIED REAL + FIXED** (FIX-A). Real on the destructive `replace_head` path (workflow + FakeTranscript; chat path is non-destructive). Fixed: split System into pinned vs prior_summaries, fold prior into `previous_summary`, `upto = prior_summaries.len()+keep_start` (first compaction byte-identical). Test: `second_compaction_folds_prior_summary_and_maps_upto`.
- **M6 — scheduler failed-firing test missing** (test-coverage) — **FIXED** (FIX-B, same test as H2).
- **M7 — SubAgentActivity 'Merged summary' unreachable** (FE) — **FIXED** (FIX-F). Removed the unreachable block + dead VM/adapter field + seeds (the frame carries no summary). Test updated (7/7).

## LOW
- **L1 — background read/cancel/notes lack job_kind<>'workflow' boundary** — **FIXED** (FIX-D, same choke point as M4; 4 boundary 404 tests).
- **L2 — FailFast detaches surviving children** (fanout.rs) — **FIXED** (FIX-A). Cancels token + aborts remaining handles. Test.
- **L3 — per-step token cap inert** (budget.rs) — **FIXED** (FIX-A). Wired `step_over_cap` → `StopReason::TokenCap`. Test.
- **L4 — StopReason::WallClock never constructed** (types.rs) — **ACCEPTED+documented** (FIX-A). A `pub` wire variant mapped by the chat host's `finish_reason` → `"timeout"`; not dead-lint-flagged; documented RESERVED (a future deadline-enforcing host emits it). Removing it would change the wire enum + force a server edit.
- **L5 — estimate_tokens_iter unused** (tokens.rs) — **FIXED** (FIX-A, removed — zero callers).
- **L6 — START-snapshot child mislabel** (fanout.rs) — **FIXED** (FIX-A, uses a spawned counter).
- **L7 — task-list/sub-agent card blank after mid-run reload** (FE) — **ACCEPTED+documented** (FIX-F). Inherent to the ephemeral SSE-only (`ZIEE_CHAT_AGENT_CORE`-gated, default-off) live-card design; no REST source. Documented in both extension stores; no fake persistence added.
