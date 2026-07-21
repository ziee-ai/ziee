import type { MountEntry } from '@/api-client/types'
import type { ConversationHostMountsGet, ConversationHostMountsSet } from '../state'
import doSaveFactory from './_doSave'

export default (set: ConversationHostMountsSet, _get: ConversationHostMountsGet) => {
  const doSave = doSaveFactory(set)
  return async (conversationId: string, mounts: MountEntry[]) => doSave(conversationId, mounts)
}
