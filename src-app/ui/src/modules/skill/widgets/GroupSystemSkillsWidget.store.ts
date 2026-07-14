import type { Skill } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'

interface GroupSkills {
  groupId: string
  skills: Skill[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

/** Group → assigned system skills (single-call, 30s cache; no event subs). */
export const GroupSystemSkillsWidget = defineStore('GroupSystemSkillsWidget', {
  immer: true,
  state: { groupSkills: new Map<string, GroupSkills>() },
  actions: (set, get) => ({
    loadSkillsForGroup: async (groupId: string, force = false) => {
      const existing = get().groupSkills.get(groupId)
      if (existing?.loading && !force) return
      if (!force && existing?.lastFetched && Date.now() - existing.lastFetched < 30000 && !existing.error) {
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
        const response = await ApiClient.Group.getSystemSkills({ group_id: groupId })
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
            error: error instanceof Error ? error.message : 'Failed to load skills',
            lastFetched: existing?.lastFetched ?? null,
          })
        })
      }
    },
    updateGroupSkills: async (groupId: string, skillIds: string[]) => {
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
  }),
})

export const useGroupSystemSkillsWidgetStore = GroupSystemSkillsWidget.store
