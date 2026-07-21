import { ApiClient } from '@/api-client'
import { type CreateKnowledgeBaseRequest, type KnowledgeBase } from '@/api-client/types'
import type { KnowledgeBasesGet, KnowledgeBasesSet } from '../state'

export default (set: KnowledgeBasesSet, _get: KnowledgeBasesGet) =>
  async (data: CreateKnowledgeBaseRequest): Promise<KnowledgeBase> => {
    try {
      set({ creating: true, error: null })
      const kb = await ApiClient.KnowledgeBase.create(data)
      set(draft => {
        draft.items.set(kb.id, kb)
        draft.creating = false
      })
      return kb
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to create',
        creating: false,
      })
      throw error
    }
  }
