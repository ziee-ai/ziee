# DRIFT-2 — collapse-border-overlay (post-audit convergence)

Round 2, after the phase-6 blind audit. One structural reversal and the fallout.

- **DRIFT-2.1** — verdict: plan-wins — **ITEM-3 (the split) is REVERTED in full.**
  `ChatMessage.tsx`, `collapsible.ts` and `collapsible.test.ts` are restored to
  `origin/khoi`; the diff no longer touches them. Three independent blind
  auditors converged on the same two HIGH regressions, and I reproduced one
  directly: a long turn ending on a structural node rendered **1044px tall with
  no collapsible and no toggle**, where it previously clamped to 384px. Root
  cause of the regression: the collapse DECISION is computed over the whole
  message while the split made the clamp SCOPE only the trailing prose, so a turn
  whose bulk is not in the final prose run silently lost its height bound. The
  reproduction had already established that the inset ALONE fixes the reported
  bug, so the split was never load-bearing. Escalated to the user as an option
  picker (audit-vs-user-decision rule); they chose to drop it. DEC-1 revised;
  DEC-2/3/4 marked moot.
- **DRIFT-2.2** — verdict: resolved — **I tuned a test around the defect instead
  of recognising it.** When the fixture's prose measured 368px and the message
  stopped collapsing, I recorded it as a fixture-sizing problem and enlarged the
  fixture from 8 to 12 copies. That WAS the HIGH regression surfacing — the
  clamp measuring only the trailing prose — and an auditor identified it from my
  own comment. The comment is removed and the underlying defect is gone with the
  split. Recorded because the failure mode (treating a surprising measurement as
  a fixture problem rather than a signal) is the generalizable lesson, not the
  specific number.
- **DRIFT-2.3** — verdict: resolved — the regression spec was rewritten to assert
  the **effect** rather than the mechanism. The original TEST-4 asserted
  `paddingLeft === '2px'` / `marginLeft === '-2px'` — a class-application check
  restated as geometry, which would fail spuriously on any equivalent
  re-implementation and would not have caught the original bug. It now asserts
  that every card inside the clamp has ≥1px between its border box and the
  container's clip edge, which is precisely the condition under which a
  1px-spread ring survives, and is technique-agnostic. Verified to FAIL on the
  unfixed code with an actionable message: *"thinking-card: only 0px between the
  card and the clip edge — its 1px ring is clipped (LEFT). This is issue #183."*
- **DRIFT-2.4** — verdict: resolved — three further audit findings on the old
  spec are addressed by the rewrite: TEST-8's card assertion was **vacuous**
  (post-split, cards were never inside the clamp in any state, so it merely
  restated TEST-3 and had no non-emptiness guard); TEST-2's "order preserved"
  claim was **unbacked** (the order array excluded text nodes, so a genuine
  reorder was undetectable); and the theme loop was **coverage inflation** (every
  assertion was structural and theme-independent). The rewrite adds a
  `>= 3 cards inside the clamp` guard, includes prose in the order signature, and
  makes the theme loop meaningful by having it exercise the ring-room condition
  in both themes (`ring-foreground/10` resolves differently per theme).
- **DRIFT-2.5** — verdict: resolved — a new **TEST-5** pins height-bounding
  (`data-collapsed === 'true'`, clamp ≤400px, "Show more" present). This is the
  guarantee the reverted split broke, and nothing previously asserted it on this
  surface. Its absence is why the split's regression reached the audit rather
  than being caught at implementation time.
- **DRIFT-2.6** — verdict: none — the audit's remaining CSS finding is accepted
  and documented rather than coded around: the inset's correctness depends on the
  parent supplying ≥2px horizontal padding (today `ChatMessage`'s `px-0.5`), an
  invariant no type enforces. Mitigation: the spec asserts the ring-room EFFECT,
  so a parent that stops absorbing the 4px fails the test regardless of mechanism.
  The component comment now states the requirement explicitly.
- **DRIFT-2.7** — verdict: none — audit findings deliberately NOT actioned, with
  reasons: (a) the inset is horizontal-only, so a bordered prose element that is
  the FIRST child of the clamped region still has its TOP hairline clipped — real,
  but pre-existing, not part of #183, and adding `-my/py` would change the clamp's
  vertical geometry and the overflow measurement; filed as a follow-up rather than
  widened here. (b) `classifyNode` failing open on unknown content types — moot,
  the function no longer exists. (c) The gallery `expand-collapsed-message`
  interaction is retained even though the spec drives the toggle directly: it
  gives runtime-health and the geometry audit an expanded-state cell for free.

**Unresolved drifts:** 0
