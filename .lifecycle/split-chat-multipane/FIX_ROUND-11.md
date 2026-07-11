# FIX_ROUND-11 — split-chat-multipane (iteration round 3 blind audit)

Blind multi-angle audit of the round-3 DELTA (ITEM-43 / FB-8: the explicit 3-way
open-conversation prompt), on the merged base.

## Blind round (2 fresh agents, diff-only: uncommitted `git diff`)

- **Agent A — correctness / state-management / api-contract** → **[]**. Verified
  the `dialog.choose` settled-guard resolves the CLICKED option key (not null) —
  the option's `onClick` runs before Radix's `onOpenChange(false)`, which then
  no-ops on the guard; `null` only on a true Esc/overlay dismiss. `confirm`/alert
  contracts unchanged (`okText` still required for those; `dialog.confirm` still
  `Promise<boolean>`). `needsOpenChoice` is exactly `auto && panes>=2 && !open`.
  All branch outcomes handled (cancel/single/replace/new + cap backstop). No
  realistic stale-`sv` race (modal focus-trap + reducer re-reads pane state).
- **Agent B — patterns-conformance / tests-quality / a11y** → **[]**.
  `dialog.choose` follows the imperative-dialog seam faithfully; testids stamp to
  the exact `open-conversation-choice-opt-{single,replace,new}` the specs select.
  a11y valid (multiple actions in one AlertDialog is fine; focus trap; least-
  destructive default focus). Every e2e option test asserts the ACTUAL resulting
  layout, and each negative test pairs absence-of-prompt with a positive assertion
  (navigation / focus-ring) that would fail if the prompt logic broke. The
  `sidebar-reroute` edit preserved its original assertion. Unit test has complete
  branch coverage of `needsOpenChoice`.

Both angles clean; no fix required.

**New confirmed findings:** 0
