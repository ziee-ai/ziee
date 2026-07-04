import { ApiClient } from '@/api-client'
import type {
  Assistant,
  CreateAssistantFromHubRequest,
  HubAssistant,
} from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import {
  emitAssistantCreated,
  emitAssistantDeleted,
  emitAssistantTemplateCreated,
  emitAssistantTemplateDeleted,
} from '@/modules/assistant/events'

export const HubAssistants = defineStore('HubAssistants', {
  immer: true,
  state: {
    assistants: [] as HubAssistant[],
    version: null as string | null,
    loading: false,
    creating: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadAssistants = async (force = false) => {
      if (get().loading && !force) return
      set({ loading: true, error: null })
      try {
        // Load with user's locale
        const locale = 'en' // TODO: Get from user settings
        const assistants = await ApiClient.Hub.getAssistants({ lang: locale })
        const versionInfo = await ApiClient.Hub.getAssistantsVersion()
        set({ assistants, version: versionInfo.version, loading: false })
      } catch (error: any) {
        set({ error: error.message || 'Failed to load hub assistants', loading: false })
      }
    }
    return {
      loadAssistants,
      refreshFromGitHub: async () => {
        set({ loading: true, error: null })
        try {
          const result = await ApiClient.Hub.refreshAssistants()
          if (result.updated) await loadAssistants()
          set({ loading: false })
        } catch (error: any) {
          set({ error: error.message || 'Failed to refresh hub assistants', loading: false })
          throw error
        }
      },
      createFromHub: async (request: CreateAssistantFromHubRequest): Promise<Assistant> => {
        set({ creating: true, error: null })
        // Snapshot displaced ids BEFORE the call so the `replace_existing`
        // path can emit `assistant.deleted` for them after the new row exists.
        const displacedIds: string[] = request.replace_existing
          ? (get().assistants.find(a => a.name === request.hub_id)?.created_ids?.slice() ?? [])
          : []
        try {
          const response = await ApiClient.Hub.createAssistantFromHub(request)
          set(state => {
            const assistant = state.assistants.find(a => a.name === request.hub_id)
            if (assistant) {
              if (request.replace_existing) {
                assistant.created_ids = [response.hub_tracking.entity_id]
              } else {
                if (!assistant.created_ids) assistant.created_ids = []
                assistant.created_ids.push(response.hub_tracking.entity_id)
              }
            }
            state.creating = false
          })
          // Emit deletion events for displaced user installs so downstream
          // caches drop the stale rows.
          for (const oldId of displacedIds) {
            if (oldId !== response.hub_tracking.entity_id) {
              try {
                await emitAssistantDeleted(oldId)
              } catch (e) {
                console.warn('Failed to emit assistant.deleted:', e)
              }
            }
          }
          // Notify downstream caches that a new user assistant exists.
          try {
            await emitAssistantCreated(response.assistant)
          } catch (e) {
            console.warn('Failed to emit assistant.created:', e)
          }
          return response.assistant
        } catch (error: any) {
          set({ error: error.message || 'Failed to create assistant from hub', creating: false })
          throw error
        }
      },
      /** Install as a system-wide TEMPLATE (is_template=true, no owner). Backend
       *  requires `hub::assistants::create` + `assistant_templates::create`;
       *  the frontend gates the button on `AssistantsTemplateCreate`. */
      createTemplateFromHub: async (
        request: CreateAssistantFromHubRequest,
      ): Promise<Assistant> => {
        set({ creating: true, error: null })
        // Snapshot displaced ids BEFORE the call so `replace_existing` can emit
        // `assistant_template.deleted` after the new row exists.
        const displacedIds: string[] = request.replace_existing
          ? (get().assistants.find(a => a.name === request.hub_id)?.created_template_ids?.slice() ??
            [])
          : []
        try {
          const response = await ApiClient.Hub.createAssistantTemplateFromHub(request)
          set(state => {
            const assistant = state.assistants.find(a => a.name === request.hub_id)
            if (assistant) {
              if (request.replace_existing) {
                assistant.created_template_ids = [response.hub_tracking.entity_id]
              } else {
                if (!assistant.created_template_ids) assistant.created_template_ids = []
                assistant.created_template_ids.push(response.hub_tracking.entity_id)
              }
            }
            state.creating = false
          })
          for (const oldId of displacedIds) {
            if (oldId !== response.hub_tracking.entity_id) {
              try {
                await emitAssistantTemplateDeleted(oldId)
              } catch (e) {
                console.warn('Failed to emit assistant_template.deleted:', e)
              }
            }
          }
          try {
            await emitAssistantTemplateCreated(response.assistant)
          } catch (e) {
            console.warn('Failed to emit assistant_template.created:', e)
          }
          return response.assistant
        } catch (error: any) {
          set({
            error: error.message || 'Failed to create assistant template from hub',
            creating: false,
          })
          throw error
        }
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('assistant.deleted', event => {
      const { assistantId } = event.data
      set(state => {
        for (const assistant of state.assistants) {
          if (assistant.created_ids) {
            assistant.created_ids = assistant.created_ids.filter(id => id !== assistantId)
          }
        }
      })
    })
    // Symmetric to the user-assistant listener: keep the "Template installed"
    // tag off a hub card when its template is deleted elsewhere.
    on('assistant_template.deleted', event => {
      const { templateId } = event.data
      set(state => {
        for (const assistant of state.assistants) {
          if (assistant.created_template_ids) {
            assistant.created_template_ids = assistant.created_template_ids.filter(
              id => id !== templateId,
            )
          }
        }
      })
    })
    void actions.loadAssistants()
  },
})

export const useHubAssistantsStore = HubAssistants.store
