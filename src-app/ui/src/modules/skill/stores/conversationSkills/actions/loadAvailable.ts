import type { ConversationSkillsGet, ConversationSkillsSet } from '../state'
import doLoadAvailableFactory from './_doLoadAvailable'

export default (set: ConversationSkillsSet, get: ConversationSkillsGet) => {
  const doLoadAvailable = doLoadAvailableFactory(set, get)
  return async (conversationId: string) => doLoadAvailable(conversationId)
}
