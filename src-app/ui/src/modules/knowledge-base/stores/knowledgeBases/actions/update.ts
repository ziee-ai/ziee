import { ApiClient } from '@/api-client'
import { type KnowledgeBase, type UpdateKnowledgeBaseRequest } from '@/api-client/types'
import type { KnowledgeBasesGet, KnowledgeBasesSet } from '../state'

export default (set: KnowledgeBasesSet, _get: KnowledgeBasesGet) =>
  async (
    id: string,
    data: UpdateKnowledgeBaseRequest,
  ): Promise<KnowledgeBase> => {
    const kb = await ApiClient.KnowledgeBase.update({ id, ...data })
    set(draft => {
      draft.items.set(kb.id, kb)
    })
    return kb
  }
