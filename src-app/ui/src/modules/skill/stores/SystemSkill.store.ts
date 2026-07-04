import { ApiClient } from '@/api-client'
import {
  type ApiEndpointParameters,
  Permissions,
  type Skill,
  type UpdateSkill,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * Admin store for system-scope skills + their group assignments. Mirrors
 * SystemMcpServer.store. The `groups` map caches assigned group ids per skill.
 */
export const SystemSkill = defineStore('SystemSkill', {
  immer: true,
  state: {
    systemSkills: [] as Skill[],
    isInitialized: false,
    loading: false,
    creating: false,
    error: null as string | null,
    // Per-skill assigned group ids (lazy-loaded by the assignment card).
    groups: {} as Record<string, { groupIds: string[]; loading: boolean }>,
  },
  actions: (set, get) => ({
    loadSystemSkills: async () => {
      if (!hasPermissionNow(Permissions.SkillsManageSystem)) return
      if (get().loading) return
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
        })
        const response = await ApiClient.SkillSystem.list({})
        set(draft => {
          draft.systemSkills = response.skills
          draft.isInitialized = true
          draft.loading = false
        })
      } catch (error) {
        set(draft => {
          draft.loading = false
          draft.error =
            error instanceof Error ? error.message : 'Failed to load system skills'
        })
      }
    },
    installSystemFromHub: async (hubId: string, groups?: string[]): Promise<Skill> => {
      set(draft => {
        draft.creating = true
        draft.error = null
      })
      try {
        const response = await ApiClient.Hub.createSystemSkillFromHub({
          hub_id: hubId,
          ...(groups && groups.length > 0 ? { groups } : {}),
        })
        set(draft => {
          draft.systemSkills.push(response.skill)
          draft.creating = false
        })
        return response.skill
      } catch (error) {
        set(draft => {
          draft.creating = false
          draft.error =
            error instanceof Error ? error.message : 'Failed to install system skill'
        })
        throw error
      }
    },
    importSystemSkill: async (form: FormData): Promise<Skill> => {
      set(draft => {
        draft.creating = true
        draft.error = null
      })
      try {
        const skill = await ApiClient.Skill.import(
          form as ApiEndpointParameters['Skill.import'],
        )
        set(draft => {
          const idx = draft.systemSkills.findIndex(s => s.id === skill.id)
          if (idx >= 0) draft.systemSkills[idx] = skill
          else draft.systemSkills.push(skill)
          draft.creating = false
        })
        return skill
      } catch (error) {
        set(draft => {
          draft.creating = false
          draft.error =
            error instanceof Error ? error.message : 'Failed to import system skill'
        })
        throw error
      }
    },
    updateSystemSkill: async (id: string, data: UpdateSkill): Promise<Skill> => {
      const updated = await ApiClient.SkillSystem.update({ id, ...data })
      set(draft => {
        const idx = draft.systemSkills.findIndex(s => s.id === id)
        if (idx >= 0) draft.systemSkills[idx] = updated
      })
      return updated
    },
    deleteSystemSkill: async (id: string): Promise<void> => {
      await ApiClient.SkillSystem.delete({ id })
      set(draft => {
        draft.systemSkills = draft.systemSkills.filter(s => s.id !== id)
      })
    },
    loadGroups: async (skillId: string) => {
      set(draft => {
        draft.groups[skillId] = {
          groupIds: draft.groups[skillId]?.groupIds ?? [],
          loading: true,
        }
      })
      try {
        const groupIds = await ApiClient.SkillSystem.getGroups({ id: skillId })
        set(draft => {
          draft.groups[skillId] = { groupIds, loading: false }
        })
      } catch (error) {
        set(draft => {
          draft.groups[skillId] = {
            groupIds: draft.groups[skillId]?.groupIds ?? [],
            loading: false,
          }
          draft.error =
            error instanceof Error ? error.message : 'Failed to load skill groups'
        })
      }
    },
    setGroups: async (skillId: string, groupIds: string[]) => {
      await ApiClient.SkillSystem.setGroups({ id: skillId, group_ids: groupIds })
      set(draft => {
        draft.groups[skillId] = { groupIds, loading: false }
      })
    },
  }),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadSystemSkills()
    on('sync:skill_system', reload)
    on('sync:reconnect', reload)
    void actions.loadSystemSkills()
  },
})

export const useSystemSkillStore = SystemSkill.store
