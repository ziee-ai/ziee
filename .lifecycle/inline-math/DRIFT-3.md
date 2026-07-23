# DRIFT-3 — implementation vs plan, after live container verification

- **DRIFT-3.1** — verdict: impl-wins — running the real stack (SPA image built from this
  branch, real GPT-OSS 120B) surfaced a genuine under-conversion that no test covered,
  because every existing test happened to put display math in its own paragraph. The
  display pass emits its `$$` fences with SINGLE newlines — deliberate, so the block stays
  inside its list item / blockquote — which leaves the block in the SAME
  blank-line-delimited paragraph as the surrounding prose. ITEM-4's coarse "any live `$`
  blocks" rule therefore fired on the fence the display pass had just emitted, and
  `The energy is \[ E=mc^2 \] where \( m \) is mass.` silently kept `( m )` literal.
  Observed live in the rendered user-message bubble, not deduced. Fixed as **ITEM-10**:
  the guard now pairs `$` runs BY LENGTH the way micromark does, so only the two genuinely
  unsafe shapes block. PLAN.md, PLAN_AUDIT.md and TESTS.md amended; TEST-21 added.

- **DRIFT-3.2** — verdict: resolved — the tightening was gated on a claim the earlier work
  had NOT verified: that a `$$` run cannot close a single-`$` opener. Confirmed against the
  installed micromark before writing any code, across all six relevant shapes (paired `$$`
  + inline, mid-paragraph display + inline, unpaired `$$` + inline, paired singles +
  inline, the lone-`$` hijack that must still block, and a `\( \)` inside a `$$…$$` body
  that must still be skipped). Two of those six are the new *negative* controls, so the
  relaxation is bounded by evidence rather than by reasoning alone.

- **DRIFT-3.3** — verdict: none — TEST-5 was split: the unpaired-single and inside-a-span
  cases stay (they still block), while `$5 and $10 for \( E \)` moved to TEST-21 and now
  asserts CONVERSION. Its old comment claimed "an even count still hijacks", which was
  true of the counting rule but is not true of the pairing rule: those two singles resolve
  into their own span, and pairing is left-to-right, so a span added after them cannot
  change how they resolved. Verified live rather than assumed. No TEST-ID was removed
  (A5 holds).

**Unresolved drifts:** 0
