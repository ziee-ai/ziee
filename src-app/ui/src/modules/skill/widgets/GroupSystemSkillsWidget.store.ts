import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Skill } from '@/api-client/types'
import { ApiClient } from '@/api-client'

interface GroupSkills {
  groupId: string
  skills: Skill[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

interface GroupSystemSkillsWidgetState {
  // Map of groupId -> assigned system skills
  groupSkills: Map<string, GroupSkills>

  loadSkillsForGroup: (groupId: string, force?: boolean) => Promise<void>
  updateGroupSkills: (groupId: string, skillIds: string[]) => Promise<void>
}

/**
 * Group → assigned system skills, single-call per the LLM widget pattern
 * (`ApiClient.Group.getSystemSkills`), with 30s caching. No event
 * subscriptions: the drawer's save calls `updateGroupSkills` which stores the
 * returned set directly, so the widget refreshes without an events dir.
 */
export const useGroupSystemSkillsWidgetStore =
  create<GroupSystemSkillsWidgetState>()(
    subscribeWithSelector(
      immer((set, get): GroupSystemSkillsWidgetState => ({
        groupSkills: new Map(),

        loadSkillsForGroup: async (groupId, force = false): Promise<void> => {
          const existing = get().groupSkills.get(groupId)
          if (existing?.loading && !force) return
          if (
            !force &&
            existing?.lastFetched &&
            Date.now() - existing.lastFetched < 30000 &&
            !existing.error
          ) {
            return
          }

          set(state => {
            state.groupSkills.set(groupId, {
              groupId,
              skills: existing?.skills ?? [],
              loading: true,
              error: null,
              lastFetched: existing?.lastFetched ?? null,
            })
          })

          try {
            const response = await ApiClient.Group.getSystemSkills({
              group_id: groupId,
            })
            set(state => {
              state.groupSkills.set(groupId, {
                groupId,
                skills: response.skills,
                loading: false,
                error: null,
                lastFetched: Date.now(),
              })
            })
          } catch (error) {
            console.error(`Failed to load skills for group ${groupId}:`, error)
            set(state => {
              state.groupSkills.set(groupId, {
                groupId,
                skills: existing?.skills ?? [],
                loading: false,
                error:
                  error instanceof Error ? error.message : 'Failed to load skills',
                lastFetched: existing?.lastFetched ?? null,
              })
            })
          }
        },

        updateGroupSkills: async (groupId, skillIds): Promise<void> => {
          const response = await ApiClient.Group.updateSystemSkills({
            group_id: groupId,
            skill_ids: skillIds,
          })
          set(state => {
            state.groupSkills.set(groupId, {
              groupId,
              skills: response.skills,
              loading: false,
              error: null,
              lastFetched: Date.now(),
            })
          })
        },
      })),
    ),
  )
