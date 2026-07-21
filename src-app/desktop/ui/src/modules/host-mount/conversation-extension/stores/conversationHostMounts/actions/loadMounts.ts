import type { ConversationHostMountsGet, ConversationHostMountsSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: ConversationHostMountsSet, _get: ConversationHostMountsGet) => {
  const doLoad = doLoadFactory(set)
  return async (conversationId: string) => doLoad(conversationId)
}
