import { ApiClient } from '@/api-client'
import {
  type ApiEndpointParameters,
  Permissions,
  type Skill,
  type UpdateSkill,
  type ValidateSkillResponse,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * Skills store — lists the user's own + accessible system skills (each row
 * carries a `scope`), and exposes install-from-hub / import / validate / update
 * / delete. Mirrors the MCP store's lazy-init + sync-refetch shape.
 */
export const SkillStoreDef = defineStore('Skill', {
  immer: true,
  state: {
    skills: [] as Skill[],
    isInitialized: false,
    loading: false,
    creating: false,
    error: null as string | null,
    operationsLoading: {} as Record<string, boolean>,
  },
  actions: (set, get) => ({
    loadSkills: async () => {
      if (!hasPermissionNow(Permissions.SkillsRead)) return
      if (get().loading) return
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
        })
        const response = await ApiClient.Skill.list({})
        set(draft => {
          draft.skills = response.skills
          draft.isInitialized = true
          draft.loading = false
        })
      } catch (error) {
        console.error('Skills loading failed:', error)
        set(draft => {
          draft.loading = false
          draft.error = error instanceof Error ? error.message : 'Failed to load skills'
        })
      }
    },
    installFromHub: async (hubId: string): Promise<Skill> => {
      set(draft => {
        draft.creating = true
        draft.error = null
      })
      try {
        const response = await ApiClient.Hub.createSkillFromHub({ hub_id: hubId })
        set(draft => {
          draft.skills.push(response.skill)
          draft.creating = false
        })
        return response.skill
      } catch (error) {
        set(draft => {
          draft.creating = false
          draft.error = error instanceof Error ? error.message : 'Failed to install skill'
        })
        throw error
      }
    },
    importSkill: async (form: FormData): Promise<Skill> => {
      set(draft => {
        draft.creating = true
        draft.error = null
      })
      try {
        const skill = await ApiClient.Skill.import(
          form as ApiEndpointParameters['Skill.import'],
        )
        set(draft => {
          const idx = draft.skills.findIndex(s => s.id === skill.id)
          if (idx >= 0) draft.skills[idx] = skill
          else draft.skills.push(skill)
          draft.creating = false
        })
        return skill
      } catch (error) {
        set(draft => {
          draft.creating = false
          draft.error = error instanceof Error ? error.message : 'Failed to import skill'
        })
        throw error
      }
    },
    validateSkill: async (skillMd: string): Promise<ValidateSkillResponse> => {
      return await ApiClient.Skill.validate({ skill_md: skillMd })
    },
    updateSkill: async (id: string, data: UpdateSkill): Promise<Skill> => {
      set(draft => {
        draft.operationsLoading[id] = true
        draft.error = null
      })
      try {
        const updated = await ApiClient.Skill.update({ id, ...data })
        set(draft => {
          const idx = draft.skills.findIndex(s => s.id === id)
          if (idx >= 0) draft.skills[idx] = updated
          delete draft.operationsLoading[id]
        })
        return updated
      } catch (error) {
        set(draft => {
          delete draft.operationsLoading[id]
          draft.error = error instanceof Error ? error.message : 'Failed to update skill'
        })
        throw error
      }
    },
    deleteSkill: async (id: string): Promise<void> => {
      set(draft => {
        draft.operationsLoading[id] = true
        draft.error = null
      })
      try {
        await ApiClient.Skill.delete({ id })
        set(draft => {
          draft.skills = draft.skills.filter(s => s.id !== id)
          delete draft.operationsLoading[id]
        })
      } catch (error) {
        set(draft => {
          delete draft.operationsLoading[id]
          draft.error = error instanceof Error ? error.message : 'Failed to delete skill'
        })
        throw error
      }
    },
    getSkill: async (id: string): Promise<Skill> => {
      const skill = await ApiClient.Skill.get({ id })
      set(draft => {
        const idx = draft.skills.findIndex(s => s.id === id)
        if (idx >= 0) draft.skills[idx] = skill
      })
      return skill
    },
  }),
  init: ({ on, actions }) => {
    // Cross-device + local sync: the REST refetch is permission-gated internally
    // so a reconnect from a user without skills::read is a no-op.
    const reload = () => void actions.loadSkills()
    on('sync:skill', reload)
    on('sync:reconnect', reload)
    void actions.loadSkills()
  },
})

export const useSkillStore = SkillStoreDef.store
