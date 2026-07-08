# message-scroll-perf — FIX_ROUND-1

## Fixes applied to the original LEDGER (round-0) findings

All 15 confirmed findings from the first blind audit were fixed (commit
"FIX_ROUND-1 — resolve all confirmed audit findings"):

- **HIGH security** (`imageSrcPolicy.ts`) — dropped the `startsWith('/')` fast-path
  entirely; `classifyImageSrc` now resolves `new URL(src, origin)` and compares
  origins, blocking `//host`, backslash `/\host`, `\\host`, `data:`, opaque
  schemes, and malformed URLs. New unit cases added.
- **HIGH perf** (`estimateMessageHeight.ts`) — memoized per `(message, width
  bucket)` in a WeakMap and capped the table/image/code marker scans to a
  4096-char prefix → O(1) on repeat, bounded on first compute.
- **HIGH perf** (`MessageList.tsx`) — content width now read from a `widthRef`
  updated by a ResizeObserver (useLayoutEffect); `estimateSize` reads the ref, so
  no `clientWidth` reflow on the measurement hot path.
- **MEDIUM perf** (`MessageList.tsx`) — the `onChange` measured-height write-back
  is debounced (single trailing 400ms timer), replacing the per-measurement
  O(n²) full-cache fold.
- **MEDIUM correctness / MEDIUM concurrency** (`MessageList.tsx`) — the seed and
  the unmount flush use the tracked width (correct bucket); the unmount flush no
  longer reads a detached `clientWidth` (0 → bucket 0 garbage).
- **LOW state** (`measuredHeightCache.ts`) — `setMeasuredHeight` now delete-before-set
  on the changed-value path (LRU recency correct).
- **LOW** (`MessageList.tsx`) — seed memo now depends on the width bucket too.
- **HIGH/MEDIUM tests** — fixed the `a-29`→`a-14` window (30 msgs = 15 pairs);
  replaced the cache-wiping full-reload warm-reopen with a real client-side
  remount (New Chat push + `goBack()`); made the image test use a 360px image
  (non-trivial jump vs the 240px reserve); added a settle poller; renamed the
  memo test to an honest runtime-error smoke with a narrowed error filter; and
  reworded the self-referential estimate sanity-bound test.

## Blind re-audit round (round 1) — NEW findings

A full 4-angle-group blind round on the fixed diff CONFIRMED the security + perf
fixes hold (no bypass / no remaining reflow) and surfaced these NEW confirmed
findings:

- **MEDIUM perf** (`MessageList.tsx`) — streaming replaces the messages Map every
  token → `messagesArray` identity churns → `initialMeasurementsCache` memo
  rebuilds O(window)/token, yet virtual-core consumes the seed only once at mount
  (wasted work + impure LRU mutation during render).
- **LOW perf** (`measuredHeightCache.ts`) — `getMeasuredHeight`'s LRU delete+set
  runs during render (via `buildInitialMeasurementsCache` in the memo), compounding
  the per-token rebuild.
- **LOW security** (`ReservedImage.tsx`) — no `onError` handler → a broken/404
  same-origin image keeps its 240px reservation forever (phantom gap).
- **MEDIUM tests** (`message-scroll-perf.spec.ts`) — warm-start ratio `>0.8` does
  not isolate a working seed from the estimator (which can also exceed 0.8).
- **LOW tests** (`message-scroll-perf.spec.ts`) — `settledScrollHeight` returns on
  the first two-equal reads → can return a not-yet-final height across an async
  Shiki plateau.
- **LOW correctness** (`MessageList.tsx`) — the `widthBucketState`→seed-rebuild is
  ineffective for mid-session resize (virtual-core re-reads the seed only while
  its measurementsCache is empty).
- **LOW correctness** (`MessageList.tsx`) — a fresh mount with `count>0` at first
  render builds the seed at the fallback width (latent: the store clears messages
  on switch so mounts are normally `count=0`).
- **LOW patterns** (`estimateMessageHeight.ts`) — default `width=768` disagreed
  with MessageList's 864 fallback bucket.

**New confirmed findings:** 8
