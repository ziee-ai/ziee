import type { DeliverablesGet, DeliverablesSet } from '../state'
import loadFactory from './load'

export default (set: DeliverablesSet, get: DeliverablesGet) => {
  const load = loadFactory(set, get)
  /** Render-safe: cached list, triggering a background load on first call.
   *  Note: lazy-store dispatch wraps sync returns in Promise, so callers
   *  should handle both the cached read (instant) and the Promise. */
  return async (conversationId: string): Promise<import('@/api-client/types').File[]> => {
    const cached = get().byConversation.get(conversationId)
    if (!cached && !get().loadingSet.has(conversationId)) {
      Promise.resolve().then(() => load(conversationId))
    }
    // If already cached, return it immediately via sync resolve.
    // The Promise wrapper is from the lazy-store dispatch; the value
    // resolves instantly when there's no pending I/O.
    return cached ?? []
  }
}
