import type { VirtualItem } from '@tanstack/react-virtual'

/** `@tanstack/virtual-core`'s item-key type (not exported from the package). */
type VirtualKey = number | string | bigint

/**
 * Width-bucketed, process-lifetime cache of MEASURED message-row heights
 * (message-scroll-perf ITEM-2, DEC-2/DEC-8).
 *
 * The virtualizer measures a row's true height only after it renders; on a
 * fresh mount every row starts at the (approximate) `estimateSize`, so the
 * first scroll-through pays an estimate→measured correction that moves the
 * scroll geometry. By persisting real measured heights across mounts (keyed by
 * message id) and seeding them into `useVirtualizer({ initialMeasurementsCache })`,
 * re-opening the same conversation starts rows at their true height → near-zero
 * first-scroll correction.
 *
 * Heights depend on the render width, so entries are bucketed on a coarse width
 * key: ordinary responsive jitter stays in one bucket, while a real layout
 * change (sidebar toggle, window resize) crosses buckets and correctly MISSES
 * stale-width heights rather than restoring wrong ones.
 *
 * The store is a bounded (LRU) module-level Map — process lifetime only, never
 * persisted to disk; a page reload starts cold (the estimator still gives a good
 * first pass). Message ids are UUIDs, so cross-conversation reuse is safe.
 */

/** ≈120px width granularity (DEC-2). */
export function widthBucket(width: number): number {
  return Math.max(0, Math.round(width / 120))
}

/** Max (id×bucket) entries retained before oldest-first eviction (DEC-8). */
const MAX_ENTRIES = 2000

// Insertion-ordered Map → oldest key is first; re-set on access to refresh LRU.
const store = new Map<string, number>()

function cacheKey(id: string, width: number): string {
  return `${widthBucket(width)}:${id}`
}

/** Measured height for a message id at the given width bucket, or undefined. */
export function getMeasuredHeight(id: string, width: number): number | undefined {
  const key = cacheKey(id, width)
  const size = store.get(key)
  if (size === undefined) return undefined
  // LRU refresh.
  store.delete(key)
  store.set(key, size)
  return size
}

/** Store a measured height; no-op when unchanged. Evicts oldest past the cap. */
export function setMeasuredHeight(id: string, width: number, size: number): void {
  if (!(size > 0)) return
  const key = cacheKey(id, width)
  // Delete-then-set on BOTH paths so the entry moves to the most-recent (last)
  // position — a Map.set on an EXISTING key keeps its original insertion order,
  // which would let a hot, frequently re-measured row be evicted as "oldest"
  // before genuinely cold entries.
  store.delete(key)
  store.set(key, size)
  while (store.size > MAX_ENTRIES) {
    const oldest = store.keys().next().value
    if (oldest === undefined) break
    store.delete(oldest)
  }
}

/**
 * Fold the virtualizer's own measured-size map (`instance.itemSizeCache`, keyed
 * by message id) into the persistent cache at the current width. Reads the
 * virtualizer's measurements — adds NO second observer (DEC-2). `itemSizeCache`
 * holds only REAL measured sizes (we seed it only with reals), so nothing
 * estimated is persisted.
 */
export function recordMeasurements(
  itemSizeCache: Map<VirtualKey, number>,
  width: number,
): void {
  itemSizeCache.forEach((size, key) => {
    if (typeof key === 'string') setMeasuredHeight(key, width, size)
  })
}

/**
 * Build the `initialMeasurementsCache` seed for the given ordered message ids at
 * the given width — ONE entry per id that has a real cached height at this
 * bucket (ids without a cached height are omitted so the virtualizer estimates
 * them and its first-measure scroll adjustment still fires). Only `key` + `size`
 * are functionally consumed (they seed `itemSizeCache`); `start`/`end` are
 * recomputed by the virtualizer in the same pass, so a cumulative layout here is
 * unnecessary.
 */
export function buildInitialMeasurementsCache(
  ids: string[],
  width: number,
): VirtualItem[] {
  const out: VirtualItem[] = []
  for (let index = 0; index < ids.length; index++) {
    const id = ids[index]
    const size = getMeasuredHeight(id, width)
    if (size === undefined) continue
    out.push({ key: id, index, start: 0, end: size, size, lane: 0 })
  }
  return out
}

/** Test-only: reset the module cache between unit tests. */
export function __clearMeasuredHeightCache(): void {
  store.clear()
}
