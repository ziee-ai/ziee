# message-scroll-perf — DECISIONS

Every human/product input the implementation needs, resolved up-front so
implementation runs nonstop. No open markers.

### DEC-1: What shape is the content-aware height estimate (ITEM-1)?
**Resolution:** `estimateMessageHeight(msg, width)` = a base of 72px (bubble
chrome: padding + the actions/branch row) + per-content-block additive terms,
summed over `msg.contents`: a `text` block adds `ceil(text.length / charsPerLine)
* lineHeight` where `charsPerLine = max(24, floor(width / 8))` and `lineHeight =
24`, capped at 900px per text block; a block whose text contains a markdown table
delimiter row (`|---`) adds a flat +300 (near the `min(60vh,36rem)` cap midpoint);
a markdown image (`![`) or a raw allowed `<img` adds +240 (the reserved-image
default, DEC-4); a fenced code block (```` ``` ````) adds +160; a
`tool_use`/`tool_result`/`file_attachment`/`image`(file) block adds +120. A short
user turn therefore estimates ~110–140px (matching today's constant), a
table/image answer ~400–800px. The function is total (undefined/empty message →
the 140px floor) so it never throws where the old constant could not.
**Resolution basis width:** the estimator reads the live viewport width from
`getScrollElement()?.clientWidth` at virtualizer construction, falling back to 768
(the app's `max-w-4xl` content width) when the scroller isn't ready yet.
**Basis:** convention — mirrors the additive-heuristic style used elsewhere in the
kit for measured content and keeps the short-turn estimate at today's 140 floor so
nothing regresses for text-only conversations.

### DEC-2: How are measured heights written back + what is the width bucket (ITEM-2)?
**Resolution:** Write-back rides the virtualizer's `onChange(instance)` callback:
on each change, iterate `instance.measurementsCache` and upsert
`measuredHeightCache.set(messageId, widthBucket, size)` only when the size differs
from the cached value (settled-only, no-op when unchanged) — this reads the
virtualizer's OWN measurements, adding no second observer. The width bucket is
`Math.round(width / 120)` (≈120px granularity), so ordinary responsive jitter
stays in one bucket while a real layout change (sidebar open/close, window resize)
crosses buckets and correctly misses stale-width heights. `initialMeasurementsCache`
is built once at construction from the cache for the current bucket.
**Basis:** codebase — `onChange` + `measurementsCache` are the documented
react-virtual 3.14.5 seams; the settled-only guard follows the existing
"write only on real change" discipline in the chat stores.

### DEC-3: Where does ReservedImage sit relative to the image security policy (ITEM-3)?
**Resolution:** The security check in the `img` override is UNCHANGED and stays
FIRST: external / `data:` / malformed src → `BlockedImage` exactly as today. Only
the already-permitted branches (`src.startsWith('/')` and the same-origin URL
branch) swap their bare `return <img {...props}/>` for `return <ReservedImage
{...props}/>`. `ReservedImage` never relaxes any origin check — it only wraps an
`<img>` the policy already approved. The phase-6 security angle verifies the policy
branches are byte-identical.
**Basis:** convention — preserves the documented anti-exfil contract in
`useStreamdownComponents.tsx`; ITEM-3 is a layout change, never a policy change.

### DEC-4: What height does ReservedImage reserve when intrinsic dims are unknown (ITEM-3)?
**Resolution:** When the `<img>` carries numeric `width` AND `height`, reserve via
`aspect-ratio: w / h` on a `max-w-full` box (exact, zero post-load shift). When
dimensions are absent (the common markdown `![](src)` case), reserve a stable box
of `min-height: 240px` with the image `object-contain`; on `onLoad`, drop the
min-height so the final layout equals the natural height. 240px matches the
estimator's image term (DEC-1) so estimate and reserved height agree. This bounds
(not eliminates) the post-load delta for dimensionless images to the difference
between 240 and natural height — and the virtualizer's above-viewport adjustment
(3.14.5 default) absorbs that for off-screen rows.
**Basis:** convention — mirrors the definite-height approach proven by
`inline-csv-height-stability` and keeps the reserve value equal to the estimator's
image term for consistency.

### DEC-5: What overscan value (ITEM-5)?
**Resolution:** (revised — see DRIFT-2) Keep `overscan` at **8** (main's original
value). The initial plan dropped it to 4 to halve heavy off-screen mounts, but
phase-8 e2e (`lazy-load-messages` anchor invariant) showed overscan 4 regressed
the reverse-infinite-scroll anchor by ~120px (vs the <80px tolerance): with fewer
off-screen rows measured above the viewport, the prepend anchor-restore leans on
the coarser estimate and the view drifts. Anchor precision outweighs the marginal
off-screen-mount saving, especially since `ChatMessage` is memoized (extra
overscan rows don't re-render on scroll — the perf cost is only their one-time
mount). The other perf wins (content-aware estimate, measured-height seed,
reserved images) are unaffected, and the user's lag was never overscan-driven.
**Basis:** codebase + e2e measurement — the anchor invariant is the objective
gate; DEC-5's own "revise via a drift entry if a regression shows" clause fired.

### DEC-6: What is the ITEM-6 anchor-reconcile mechanism, and may it be a no-op?
**Resolution:** Default mechanism: in `restoreAnchor`, after computing the
index offset, restore in a `requestAnimationFrame` AFTER the prepended rows have
had one measure pass, and set the virtualizer scroll via `scrollToOffset` with a
guard that skips if the current offset is already within 2px of the target (the
virtualizer's own above-viewport first-measure adjustment already pinned it) —
eliminating the double-adjust while keeping the restore as a backstop. If phase-5
measurement (TEST-10 instrumentation) shows the existing single-shot restore
already yields zero residual jump under the 3.14.5 default adjustment, ITEM-6
collapses to "no code change; keep TEST-10 as the guard" recorded as an
`impl-wins` drift. Either way the no-teleport invariant is the gate.
**Basis:** codebase — the 3.14.5 `resizeItem` default already adjusts scroll for
above-viewport first-measurements (confirmed in virtual-core dist), so the manual
restore is potentially redundant; the guard makes the combination idempotent.

### DEC-7: What are the e2e "stability" tolerances (TEST-7/8/9)?
**Resolution:** TEST-7: per-scroll-step `scrollHeight` change ≤ 15% of the viewport
height (a corrected estimate can never move the total by more than a viewport's
worth if estimates are within range); cumulative drift bounded. TEST-9: a fixed
reference row's `boundingBox().y` moves ≤ 8px across an image load (matches the
`inline-csv` spec's 20px table tolerance, tighter for a single image). TEST-8:
cold-open cumulative `scrollHeight` correction over a full top→bottom scroll is at
least 2× the warm-reopen correction. These are asserted as inequalities, not exact
pixels, to stay deterministic across CI render timing.
**Basis:** convention — mirrors the tolerance/settle style of
`inline-csv-height-stability.spec.ts` (fixed thresholds + a settle wait).

### DEC-8: Does the module-level measured-height cache leak (ITEM-2)?
**Resolution:** Cap the cache with a simple LRU bound of 2000 entries (id×bucket) —
far above any single loaded window, small in memory (a number per entry). Eviction
is oldest-insertion-first. The cache is process-lifetime (not persisted to disk /
localStorage); a page reload starts cold, which is acceptable (the estimator DEC-1
still gives a good first pass). No conversation-scoped clearing is needed because
keys are message ids (globally unique) — cross-conversation reuse is a feature, not
a bug.
**Basis:** convention — bounded module caches (LRU) are the standard guard against
unbounded `Map` growth; message ids are UUIDs so no cross-conversation collision.

### DEC-9: Which UI surfaces / gallery states change (phase-8 state-matrix)?
**Resolution:** `ReservedImage`'s pre-load placeholder is a NEW visual state for an
inline assistant image. If the chat gallery cassette renders an assistant message
with an inline image, add a gallery cell for the reserved/placeholder state; if no
gallery surface renders an inline markdown image today, no state-matrix entry is
required (the estimator/cache/overscan changes are invisible geometry with no new
render state). The determination is made against `check:gallery-coverage` /
`check:state-matrix` output during phase 8; if it demands a cell, one is added for
the placeholder — budgeted here so phase 8 does not fail on it.
**Basis:** codebase — the `check:state-matrix` gate inside `npm run check` is the
authority on which states need coverage; only `ReservedImage` introduces a
candidate new state.
