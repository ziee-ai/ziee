# DRIFT-15 — FB-24 UI-polish batch (focus indicator reverses DEC-28)

Reconciling the FB-24 UI directives (ITEM-75..78) + ITEM-74 against the plan/DECs.

- **DRIFT-15.1** — verdict: impl-wins — **Focus indicator: dim (opacity-45) instead of
  a 2px ring.** DEC-28 specified "a `--ring` outline; the other pane is NOT dimmed". The
  human, reviewing the live panes (FB-24), directed the opposite: no ring, dim the
  unfocused pane. Recorded as a DEC-28 AMENDMENT (human-approved, screenshot-confirmed) —
  a deliberate product-taste reversal, not a silent cut. Legibility-of-both-streams (the
  original clause's concern) is the accepted tradeoff. The 17 focus e2e assertions moved
  `ring-primary` → `opacity-100`.

- **DRIFT-15.2** — verdict: none — ITEM-76 (1px separator) / ITEM-77 (imperative resize) /
  ITEM-78 (hide split button at cap) introduce no plan/DEC conflict — they refine the
  DEC-24/25 split chrome + fix a perf defect; the resize/divider behavior (DEC-24) is
  unchanged functionally (persistence.spec still green).

- **DRIFT-15.3** — verdict: resolved — ITEM-74 (local-delete closes pane) closes the FB-23
  gap; it EXTENDS (does not change) the ITEM-29 close-on-delete behavior to the local
  origin. No plan conflict.

**Unresolved drifts:** 0
