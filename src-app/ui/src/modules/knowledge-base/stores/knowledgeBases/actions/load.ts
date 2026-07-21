import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { KnowledgeBasesGet, KnowledgeBasesSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: KnowledgeBasesSet, get: KnowledgeBasesGet) => {
  const doLoad = doLoadFactory(set, get)
  return async (force = false) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    const state = get()
    if ((state.isInitialized && !force) || state.loading) return
    try {
      set({ loading: true, error: null })
      const rows = await doLoad()
      set({
        items: new Map(rows),
        isInitialized: true,
        loading: false,
      })
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to load knowledge bases',
        loading: false,
      })
      throw error
    }
  }
}
