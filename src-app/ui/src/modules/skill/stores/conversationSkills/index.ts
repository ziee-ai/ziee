import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { conversationSkillsState } from './state'
import type { Actions } from './actions.gen'
import type { ConversationSkillsState } from './state'

const ConversationSkillsDef = defineStore<ConversationSkillsState, Actions>('ConversationSkills', {
  immer: true,
  state: conversationSkillsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    // A remote skill change (install/remove, group-restriction edit) alters a
    // conversation's effective available set. Refetch every loaded conversation;
    // self-gate on skills::read so a user lacking it never 403s on reconnect.
    const reload = () => {
      if (!hasPermissionNow(Permissions.SkillsRead)) return
      for (const cid of Object.keys(get().available)) void actions.loadAvailable(cid)
    }
    on('sync:skill', reload)
    on('sync:reconnect', reload)
  },
})
// Re-export as `ConversationSkills` so gallery code using `.store.setState` continues to work.
registerLazyStore(ConversationSkillsDef)
export { ConversationSkillsDef as ConversationSkills }
export const useConversationSkillsStore = ConversationSkillsDef.store

/** Helper: which of the user's installed skills are currently hidden in this
 *  conversation (present in the install list but absent from the effective
 *  available list). */
export function deriveHiddenSkills(
  allSkills: import('@/api-client/types').Skill[],
  available: import('@/api-client/types').AvailableSkillEntry[] | undefined,
): import('@/api-client/types').Skill[] {
  if (!available) return []
  const availableIds = new Set(available.map(s => s.id))
  return allSkills.filter(s => s.enabled && !availableIds.has(s.id))
}
