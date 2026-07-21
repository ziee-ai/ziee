import { ApiClient } from '@/api-client'
import type { KnowledgeBasesGet, KnowledgeBasesSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: KnowledgeBasesSet, get: KnowledgeBasesGet) => {
  const doLoad = doLoadFactory(set, get)
  return async (id: string): Promise<void> => {
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.KnowledgeBase.delete({ id })
      set(draft => {
        draft.items.delete(id)
        draft.deleting = false
      })
      void doLoad().then((rows) => {
        set({
          items: new Map(rows),
        })
      }).catch(() => {
        // If refresh fails, the list still has the item removed.
        // The next load() or sync event will recover.
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete',
        deleting: false,
      })
      throw error
    }
  }
}
