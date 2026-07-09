# FIX_ROUND-2 — js-tool-scripting (convergence)

Round 2 of the fix/re-audit loop. Fixed every finding deferred from FIX_ROUND-1,
then ran TWO blind re-audit passes over the fixed diff (10 angles in the first,
2 focused angles on the spawn_blocking restructure in the second). Every finding
surfaced by both re-audit passes has been fixed. **The final re-audit round
yielded no new confirmed (blocking) defect.**

## Fixed the 9 deferred findings (from FIX_ROUND-1)

- **[HIGH] Sync-JS / catastrophic-regex worker starvation** — VERIFIED
  quickjs-ng polls the runtime interrupt handler inside libregexp
  (`lre_poll_timeout`→`lre_check_timeout`→`rt->interrupt_handler`), so gas/cancel
  DO kill a bad regex (test `test_catastrophic_regex_is_interruptible`). Then
  moved the whole interpreter to `spawn_blocking` (below) so CPU-bound JS can't
  monopolize an async worker or starve the watchdog regardless.
- **[HIGH] Frontend `resolveElicitation` never throws** — `JsToolApprovalContent`
  now derives resolved state from the `elicitationRequests` store (mirroring
  `ElicitationFormContent`); `resolveElicitation` rolls the entry back to
  'pending' on a failed POST, so the buttons return and no false "Approved".
- **[MED] Aggregate approval-wait cap** — `max_approvals=25` bounds cumulative
  suspended time.
- **[MED] Output cap after materialization** — `wrap_script` caps the result
  JS-side (before the FFI boundary) with a collision-proof `__ziee_truncated`
  marker; host `cap_output` remains the backstop.
- **[MED] Resolved-status remount** — derived from the store (survives remount).
- **[LOW] Console byte overshoot / `__ziee_set_result` hijard / registry
  drop-guard / global runtime cap** — char-boundary byte truncation; capture +
  `delete` the global result handle; `RegistryGuard` RAII cleanup; global
  admission `Semaphore(8)`.

## Fixed the re-audit findings (blind round on the fixed diff)

- **[HIGH] Global-semaphore head-of-line regression** — the admission semaphore,
  held during approval-suspend, could block run_js server-wide for the suspend
  window. Fixed: bounded the global acquire with a 15 s timeout → fast "busy"
  result (the assistant turn never stalls); and moved the interpreter to
  `spawn_blocking`.
- **[MED] Unbounded trace Vec on denied dispatches** — `MAX_TRACE_ENTRIES=256`.
- **[MED] Regex/sync-JS async-worker + watchdog starvation** — `spawn_blocking`
  runs the interpreter off the async workers; the watchdog stays on the main
  runtime (always schedulable); dispatch is delegated back to the main runtime
  via `Handle::spawn` so the DB pool/IO reactor stay on their owning runtime.
- **[LOW] gate() over-prompts read-only control** — `gate()` now takes
  `is_control` and auto-runs read-only control (matches the normal loop).
- **[LOW] `addElicitationRequest` overwrite on double-delivery** — guarded on
  `!has(id)`.
- **[LOW] console transient host copy / truncated_output false-positive /
  wrap_script comment** — JS-side slice; collision-proof sentinel; accurate
  comment.
- **[LOW] spawn_blocking permit accounting under abort** (both convergence
  agents) — moved the `GLOBAL_RUN_SEM` permit INTO the spawn_blocking closure, so
  the admission slot reflects the interpreter's true lifetime (at most 8 live
  interpreters; the blocking pool can't be saturated by lingering detached runs).

## Re-audit outcome

The concurrency convergence agent VERIFIED the `spawn_blocking` restructure is
correct (sound rquickjs-on-blocking-thread usage, correct cross-runtime waker
propagation with no deadlock, satisfied Send/'static bounds, reliably schedulable
watchdog, clean permit/watchdog lifetimes). The resource-limits/correctness agent
VERIFIED all six round-3 fixes hold with no new correctness defect (trace-cap
arithmetic, timeout arms, slice bound, sentinel detection, gate truth table).
Both surfaced only ONE LOW finding (the permit-accounting item above), now fixed.

Documented absolute upper bound on one run: ~128 MiB + <1 MiB; active wall-clock
≤300 s (gas kills tight CPU/regex loops sub-second); suspended (approval) time
≤ max_approvals×approval_timeout, during which one of 8 global slots + one
blocking thread are held; a client disconnect resolves parked approvals at once.

19 js_tool unit tests green; frontend `npm run check` green.

**New confirmed findings:** 0
