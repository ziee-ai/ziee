import { ApiClient } from '@/api-client'
import {
  type AgentAdminSettings as AgentAdminSettingsRow,
  type LlmModel,
  Permissions,
  type UpdateAgentAdminSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { emitAgentAdminSettingsUpdated } from '@/modules/agent/events'

// Candidate model row for the reviewer-model picker. The reviewer is a cheap
// chat model that risk-classifies approval-needing tool calls, so any
// chat-capable model qualifies.
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

// Widened patch type. The backend uses `Option<Option<T>>` for the model id +
// policy fields — tri-state (absent = leave, null = clear, value = set). The TS
// codegen strips the `null` arm, so widen it at the boundary; JSON.stringify
// writes null-vs-absent correctly and the backend's deserialize_nullable_field
// honors both.
export type AgentAdminUpdatePatch = Omit<
  UpdateAgentAdminSettingsRequest,
  'reviewer_model_id' | 'reviewer_policy'
> & {
  reviewer_model_id?: string | null
  reviewer_policy?: string | null
}

const toRow = (m: LlmModel): CandidateModelRow => ({
  id: m.id,
  name: m.name,
  display_name: m.display_name,
  provider_id: m.provider_id,
})

/**
 * Deployment-wide agent policy (singleton row via `/api/agent/settings`). Read
 * on first mount, PATCH the diff on save. Mirrors the code-sandbox resource-
 * limits store: a plain singleton GET/PUT with sync-driven refetch. Also loads
 * a capped list of candidate models for the reviewer-model picker.
 */
export const AgentAdminSettings = defineStore('AgentAdminSettings', {
  immer: true,
  state: {
    settings: null as AgentAdminSettingsRow | null,
    availableModels: [] as CandidateModelRow[],
    loading: false,
    saving: false,
    loadingModels: false,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const row = await ApiClient.AgentAdmin.get()
        set(s => {
          s.settings = row
          s.loading = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to load agent settings'
          s.loading = false
        })
      }
    }
    const loadCandidateModels = async () => {
      set(s => {
        s.loadingModels = true
      })
      try {
        const body = await ApiClient.LlmModel.list({ page: 1, perPage: 200 })
        set(s => {
          s.availableModels = body.models.map(toRow)
          s.loadingModels = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to load models'
          s.loadingModels = false
        })
      }
    }
    return {
      load,
      loadCandidateModels,
      update: async (
        patch: AgentAdminUpdatePatch,
      ): Promise<AgentAdminSettingsRow> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: codegen loses the `null` arm; JSON.stringify writes null vs
          // absent correctly and the backend honors both (tri-state clear).
          const row = await ApiClient.AgentAdmin.update(
            patch as UpdateAgentAdminSettingsRequest,
          )
          set(s => {
            s.settings = row
            s.saving = false
          })
          try {
            await emitAgentAdminSettingsUpdated(row)
          } catch (eventError) {
            console.error(
              'Failed to emit agent admin settings updated event:',
              eventError,
            )
          }
          return row
        } catch (e: any) {
          set(s => {
            s.error = e?.message ?? 'Failed to save agent settings'
            s.saving = false
          })
          throw e
        }
      },
    }
  },
  // Singleton row. Refetch on a remote change or SSE reconnect. Self-gate the
  // refetch (no-403 reconnect rule): sync:reconnect fires for every store
  // regardless of audience, so a user without agent-settings read must not
  // refetch. The perm MUST equal the GET's read-perm.
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.AgentSettingsRead)) return
      void actions.load()
    }
    on('sync:agent_admin_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.AgentSettingsRead)) {
      void actions.load()
      void actions.loadCandidateModels()
    }
  },
})

export const useAgentAdminSettingsStore = AgentAdminSettings.store
