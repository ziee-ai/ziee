# FIX_ROUND-2 — chats-page-virtualization

Fixed the 3 findings the FIX_ROUND-1 re-audit surfaced.

## Fixed (confirmed → resolved)

- **estimator modeled the meta as inline-only → under-estimated the STACKED
  narrow layout** (perf, LOW). `ConversationCard` is `flex-col sm:flex-row`, so
  below the `sm` (640px) content width the meta row (count + relative time)
  stacks BELOW the title (title gets full width + one extra meta row). The
  estimator now branches on `width < SM_BREAKPOINT`: stacked → full-width title
  lines + `META_ROW_HEIGHT`; inline → the prior meta-reserve model. This removes
  the systematic ~18–20px under-estimate (and its extra corrections) at the 390px
  narrow surface. GUARDED by the new **TEST-12** (narrow-surface window + idle
  jank e2e).

- **width-sensitive unit test passed by equality** (tests-quality, LOW). It used
  the ~150-char `LONG` title, which saturates the 2-line cap at BOTH 320px and
  1200px, so `narrow >= wide` held even for a width-ignoring impl. Rewritten with
  a 100-char `BOUNDARY` title (one line inline-wide, two lines + stacked meta
  narrow) and a STRICT `narrow > wide`.

- **memoization unit test compared two saturated values** (tests-quality, LOW).
  Same `LONG`-saturation issue; the `narrow >= wide` bucket-independence check was
  trivially equal. Switched to `BOUNDARY` with a strict `narrow > wide`, so a
  cache that collapsed buckets to one value would now fail.

- **(follow-on) monotonic message_count test** moved to a WIDE width (900px,
  inline layout) because message_count only affects the title wrap in the inline
  layout — at the old 520px it now falls in the stacked branch where the count
  doesn't narrow the title. Uses `BOUNDARY` with a strict `>`.

## Re-audit

A THIRD full blind round (fresh diff-only agent over the FIXED diff) was run.
Result recorded below.

**New confirmed findings:** 0
