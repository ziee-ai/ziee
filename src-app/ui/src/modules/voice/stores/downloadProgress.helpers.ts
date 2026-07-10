/**
 * Pure helpers for `VoiceDownloadProgress.store.ts`, extracted so they can be
 * unit-tested without the store's runtime `@/api-client/types` (enum) import.
 * The store's permission self-gating (`loadActive` returns early unless
 * `VoiceAdminRead`) and the SSE wiring are covered by the voice admin e2e specs;
 * these two pieces are the race/clamp logic worth pinning deterministically.
 */

/** Percent for a progress bar, clamped to 0..100; undefined when total unknown. */
export function percentOf(received: number, total: number | undefined): number | undefined {
  if (!total || total === 0) return undefined
  return Math.min(100, Math.max(0, (received / total) * 100))
}

/**
 * Synchronously claim an SSE subscription slot for `key`. Returns true when THIS
 * call is the first to claim it (proceed to subscribe), false when the key is
 * already claimed (dedupe — do nothing). The claim is written before the caller
 * awaits the real `AbortController`, so a rapid second call is deduped even
 * though the controller arrives later — closing the two-callers-both-pass race.
 */
export function claimSubscription(
  aborts: Map<string, AbortController>,
  key: string,
): boolean {
  if (aborts.has(key)) return false
  aborts.set(key, new AbortController())
  return true
}
