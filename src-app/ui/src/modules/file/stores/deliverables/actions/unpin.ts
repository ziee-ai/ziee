import { ApiClient } from '@/api-client'
import type { DeliverablesGet, DeliverablesSet } from '../state'
import loadFactory from './load'

export default (set: DeliverablesSet, get: DeliverablesGet) => {
  const load = loadFactory(set, get)
  return async (conversationId: string, fileId: string): Promise<void> => {
    await ApiClient.File.unpinDeliverable({
      id: conversationId,
      file_id: fileId,
    })
    await load(conversationId)
  }
}
