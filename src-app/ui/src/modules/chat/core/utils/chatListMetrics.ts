/**
 * DEV-only correction-metrics surface for the virtualized conversation list
 * (chats-page-virtualization ITEM-6). Mirrors MessageList's `__MSGLIST_METRICS__`.
 *
 * A "correction" is a non-sync virtualizer size recompute (a row re-measured →
 * total-size changed), which is exactly the scrollbar-thumb "jank" signal. The
 * no-jank e2e resets this, scrolls, pauses, and asserts the count settles to ~0.
 *
 * Kept in a plain `.ts` module (not the `.tsx` component) so it is unit-testable
 * under the `node:test` loader (which strips `.ts` types but cannot execute JSX).
 */
export interface ChatListMetrics {
  corrections: number
  reset: () => void
  totalSize: () => number
}

declare global {
  interface Window {
    __CHATLIST_METRICS__?: ChatListMetrics
  }
}

/**
 * Build a LIVE metrics view over a mutable counter + a total-size getter.
 * `corrections` reads THROUGH the counter (not a snapshot) so the counter can be
 * incremented on each virtualizer recorrection and observed later.
 */
export function makeChatListMetrics(
  counter: { corrections: number },
  totalSize: () => number,
): ChatListMetrics {
  return {
    get corrections() {
      return counter.corrections
    },
    reset: () => {
      counter.corrections = 0
    },
    totalSize,
  }
}
