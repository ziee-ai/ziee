import { ApiClient } from '@/api-client'
import type { DeliverablesGet, DeliverablesSet } from '../state'
import loadFactory from './load'

export default (set: DeliverablesSet, get: DeliverablesGet) => {
  const load = loadFactory(set, get)
  return async (conversationId: string, fileId: string, pinned = true): Promise<void> => {
    await ApiClient.File.pinDeliverable({
      id: conversationId,
      file_id: fileId,
      pinned,
    })
    await load(conversationId)
  }
}
