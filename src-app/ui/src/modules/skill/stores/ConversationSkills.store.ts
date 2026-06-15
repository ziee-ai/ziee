import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { AvailableSkillEntry, Skill } from '@/api-client/types'

/**
 * Per-conversation skill opt-out store (Path B). Every installed skill
 * is available by default; this surface lists which skills are
 * currently VISIBLE in a conversation and lets the user hide/unhide
 * individual ones. The "available" listing is the effective set (after
 * group restrictions + the conversation's own overrides), so a skill
 * present in `available` is visible and a skill absent from it is
 * opted-out (derived via `deriveHiddenSkills`).
 */
interface ConversationSkillsState {
  // Keyed by conversation id.
  available: Record<string, AvailableSkillEntry[]>
  loading: Record<string, boolean>
  error: string | null

  loadAvailable: (conversationId: string) => Promise<void>
  hide: (skillId: string, conversationId: string) => Promise<void>
  unhide: (skillId: string, conversationId: string) => Promise<void>
}

export const useConversationSkillsStore = create<ConversationSkillsState>()(
  subscribeWithSelector(
    immer(
      (set, get): ConversationSkillsState => ({
        available: {},
        loading: {},
        error: null,

        loadAvailable: async (conversationId: string) => {
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
                error instanceof Error
                  ? error.message
                  : 'Failed to load available skills'
            })
          }
        },

        hide: async (skillId: string, conversationId: string) => {
          try {
            await ApiClient.Skill.hideInConversation({
              id: skillId,
              conversation_id: conversationId,
            })
            // Refetch the effective listing so the UI reflects the change.
            await get().loadAvailable(conversationId)
          } catch (error) {
            set(draft => {
              draft.error =
                error instanceof Error ? error.message : 'Failed to hide skill'
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
            await get().loadAvailable(conversationId)
          } catch (error) {
            set(draft => {
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to unhide skill'
            })
            throw error
          }
        },
      }),
    ),
  ),
)

/** Helper: which of the user's installed skills are currently hidden in
 *  this conversation (i.e. present in the install list but absent from
 *  the effective available list). */
export function deriveHiddenSkills(
  allSkills: Skill[],
  available: AvailableSkillEntry[] | undefined,
): Skill[] {
  if (!available) return []
  const availableIds = new Set(available.map(s => s.id))
  return allSkills.filter(s => s.enabled && !availableIds.has(s.id))
}
