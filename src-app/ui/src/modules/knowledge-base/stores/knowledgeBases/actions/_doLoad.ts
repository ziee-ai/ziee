import { ApiClient } from '@/api-client'
import type { KnowledgeBase } from '@/api-client/types'
import type { KnowledgeBasesGet, KnowledgeBasesSet } from '../state'

export default (_set: KnowledgeBasesSet, _get: KnowledgeBasesGet) => async () => {
  const list = await ApiClient.KnowledgeBase.list()
  return (list ?? []).map((kb: KnowledgeBase) => [kb.id, kb]) as [string, KnowledgeBase][]
}
