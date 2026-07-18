import { ApiClient } from '@/api-client'
import { type AvailableSkillEntry, Permissions, type Skill } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Per-conversation skill opt-out store (Path B). Every installed skill is
 * available by default; this surface lists which skills are currently VISIBLE
 * in a conversation and lets the user hide/unhide individual ones. `available`
 * is the effective set (after group restrictions + the conversation's own
 * overrides) — a skill absent from it is opted-out (see `deriveHiddenSkills`).
 */
export const ConversationSkills = defineStore('ConversationSkills', {
  immer: true,
  state: {
    // Keyed by conversation id.
    available: {} as Record<string, AvailableSkillEntry[]>,
    loading: {} as Record<string, boolean>,
    error: null as string | null,
  },
  actions: set => {
    const loadAvailable = async (conversationId: string) => {
      set(draft => {
        draft.loading[conversationId] = true
        draft.error = null
      })
      try {
        const response = await ApiClient.Skill.listAvailable({
          conversation_id: conversationId,
        })
        set(draft => {
          draft.available[conversationId] = response.skills
          draft.loading[conversationId] = false
        })
      } catch (error) {
        set(draft => {
          draft.loading[conversationId] = false
          draft.error =
            error instanceof Error ? error.message : 'Failed to load available skills'
        })
      }
    }
    return {
      loadAvailable,
      hide: async (skillId: string, conversationId: string) => {
        try {
          await ApiClient.Skill.hideInConversation({
            id: skillId,
            conversation_id: conversationId,
          })
          await loadAvailable(conversationId)
        } catch (error) {
          set(draft => {
            draft.error = error instanceof Error ? error.message : 'Failed to hide skill'
          })
          throw error
        }
      },
      unhide: async (skillId: string, conversationId: string) => {
        try {
          await ApiClient.Skill.unhideInConversation({
            id: skillId,
            conversation_id: conversationId,
          })
          await loadAvailable(conversationId)
        } catch (error) {
          set(draft => {
            draft.error =
              error instanceof Error ? error.message : 'Failed to unhide skill'
          })
          throw error
        }
      },
    }
  },
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

export const useConversationSkillsStore = ConversationSkills.store

/** Helper: which of the user's installed skills are currently hidden in this
 *  conversation (present in the install list but absent from the effective
 *  available list). */
export function deriveHiddenSkills(
  allSkills: Skill[],
  available: AvailableSkillEntry[] | undefined,
): Skill[] {
  if (!available) return []
  const availableIds = new Set(available.map(s => s.id))
  return allSkills.filter(s => s.enabled && !availableIds.has(s.id))
}
