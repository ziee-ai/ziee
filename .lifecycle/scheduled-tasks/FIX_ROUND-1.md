# FIX_ROUND-1 — scheduled-tasks

All 12 confirmed Phase-6 ledger findings fixed, then a full blind re-audit of the
fix diff was run (fresh agent, diff-only). The re-audit confirmed all 12 fixes
correct with no new behavioral defect; it raised ONE LOW incomplete-cleanup item,
now resolved.

## Fixes applied (ledger finding → fix)
- **F1 (HIGH, retry re-execution)** → removed the in-run `dispatch` retry loop; the target runs EXACTLY ONCE. Transient tolerance is provided by the existing consecutive-failure cap (a single blip never pauses; the tick only auto-pauses at the cap). No more duplicate workflow spawns / double prompt side-effects.
- **F4 (MED, allow-listed tool pauses)** → in an unattended run, an ALLOW-LISTED approval-required tool now AUTO-RUNS (pre-authorized, DEC-17.4); non-allow-listed is denied. Interactive path unchanged.
- **F2 (MED, preemptive-pause false positive)** → `conversation_deleted` now requires a prior SUCCESSFUL fire (`last_status ∈ {completed,no_change}`); a failed first-fire (`last_status='failed'`) no longer bricks the task.
- **F3 (MED, on_change buries skips)** → the silent no-notification early-return now also requires `skipped_tools.is_empty()`; a degraded run always notifies.
- **F8 (MED, required inputs)** → the drawer validates required declared workflow inputs are non-empty (typed-input mode).
- **F5 (LOW, fire-time model TOCTOU)** → `dispatch_prompt` re-validates model access at fire time (403 → terminal pause).
- **F12 (LOW, test-fire fidelity)** → test-fire runs under the unattended read-only safe floor (no Always-mode pre-exec during a Test).
- **F7 (LOW, dead code)** → removed `UnattendedToolGrant::list_allows`.
- **F6 (LOW, DOW=7)** → `Number(n) % 7` normalizes cron dow 7→Sun; no more "undefined".
- **F9 (LOW, picker widens grants)** → the allow-list picker preserves existing per-tool grants for still-selected servers.
- **F10 (LOW, copy)** → notification skipped-tools text: proper singular/plural, no emoji.
- **F11 (LOW, a11y)** → day-toggle `aria-label` is the full weekday name.

## Re-audit (blind, fix-diff only)
Raised: **1 LOW** — after removing the retry, `failure::retry_backoff_ms` +
`FailureClass::is_retryable` have only test callers (workspace `dead_code = "warn"`,
so warn-only, not a build break) and a stale comment referenced the deleted
`MAX_IN_RUN_ATTEMPTS`. **Resolved:** `#[allow(dead_code)]` + doc-note on both (kept
as tested classification utilities) and the stale comment corrected. No behavioral
change.

All other 12 fixes: re-audit verdict PASS (correct, no regression, interactive
chat path byte-unchanged, all fail-closed gates intact).

**New confirmed findings:** 0
