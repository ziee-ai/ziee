import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  type ApiEndpointParameters,
  Permissions,
  type Skill,
  type UpdateSkill,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * Admin store for system-scope skills + their group assignments.
 * Mirrors SystemMcpServer.store. The `groups` map caches the assigned
 * group ids per skill so the assignment card can render without
 * re-fetching on every render.
 */
interface SystemSkillState {
  systemSkills: Skill[]
  isInitialized: boolean
  loading: boolean
  creating: boolean
  error: string | null
  // Per-skill assigned group ids (lazy-loaded by the assignment card).
  groups: Record<string, { groupIds: string[]; loading: boolean }>

  __init__: {
    __store__?: () => void
    systemSkills: () => Promise<void>
  }
  __destroy__?: () => void

  loadSystemSkills: () => Promise<void>
  installSystemFromHub: (hubId: string, groups?: string[]) => Promise<Skill>
  importSystemSkill: (form: FormData) => Promise<Skill>
  updateSystemSkill: (id: string, data: UpdateSkill) => Promise<Skill>
  deleteSystemSkill: (id: string) => Promise<void>
  loadGroups: (skillId: string) => Promise<void>
  setGroups: (skillId: string, groupIds: string[]) => Promise<void>
}

export const useSystemSkillStore = create<SystemSkillState>()(
  subscribeWithSelector(
    immer(
      (set, get): SystemSkillState => ({
        systemSkills: [],
        isInitialized: false,
        loading: false,
        creating: false,
        error: null,
        groups: {},

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'SystemSkillStore'
            const reload = () => void get().loadSystemSkills()
            eventBus.on('sync:skill_system', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
          systemSkills: () => get().loadSystemSkills(),
        },

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
                error instanceof Error
                  ? error.message
                  : 'Failed to load system skills'
            })
          }
        },

        installSystemFromHub: async (
          hubId: string,
          groups?: string[],
        ): Promise<Skill> => {
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
                error instanceof Error
                  ? error.message
                  : 'Failed to install system skill'
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
            // FormData carries the multipart fields (incl. scope=system)
            // the endpoint reads; narrow to the endpoint's param type
            // instead of defeating type-checking with `any`.
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
                error instanceof Error
                  ? error.message
                  : 'Failed to import system skill'
            })
            throw error
          }
        },

        updateSystemSkill: async (
          id: string,
          data: UpdateSkill,
        ): Promise<Skill> => {
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
            const groupIds = await ApiClient.SkillSystem.getGroups({
              id: skillId,
            })
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
                error instanceof Error
                  ? error.message
                  : 'Failed to load skill groups'
            })
          }
        },

        setGroups: async (skillId: string, groupIds: string[]) => {
          await ApiClient.SkillSystem.setGroups({
            id: skillId,
            group_ids: groupIds,
          })
          set(draft => {
            draft.groups[skillId] = { groupIds, loading: false }
          })
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('SystemSkillStore')
        },
      }),
    ),
  ),
)
