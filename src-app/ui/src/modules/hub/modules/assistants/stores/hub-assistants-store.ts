import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  HubAssistant,
  Assistant,
  CreateAssistantFromHubRequest,
} from '@/api-client/types'
import {
  emitAssistantCreated,
  emitAssistantTemplateCreated,
} from '@/modules/assistant/events'
import { Stores } from '@/core/stores'

interface HubAssistantsState {
  assistants: HubAssistant[]
  version: string | null
  loading: boolean
  creating: boolean
  error: string | null

  // Actions
  loadAssistants: (force?: boolean) => Promise<void>
  refreshFromGitHub: () => Promise<void>
  createFromHub: (request: CreateAssistantFromHubRequest) => Promise<Assistant>
  /** Install as a system-wide TEMPLATE (is_template=true, no owner).
   *  Backend requires both `hub::assistants::create` and
   *  `assistant_templates::create` permissions; non-admin callers see
   *  a 403. The frontend gates the button on `AssistantsTemplateCreate`
   *  so the action is hidden when the user lacks the permission. */
  createTemplateFromHub: (
    request: CreateAssistantFromHubRequest,
  ) => Promise<Assistant>

  // Lazy initialization
  __init__: {
    assistants: () => Promise<void>
    __store__?: () => void
  }
  __destroy__?: () => void
}

export const useHubAssistantsStore = create<HubAssistantsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubAssistantsState => ({
        assistants: [],
        version: null,
        loading: false,
        creating: false,
        error: null,

        loadAssistants: async (force = false) => {
          const state = get()
          if (state.loading && !force) return

          set({ loading: true, error: null })
          try {
            // Load with user's locale
            const locale = 'en' // TODO: Get from user settings
            const assistants = await ApiClient.Hub.getAssistants({
              lang: locale,
            })
            const versionInfo = await ApiClient.Hub.getAssistantsVersion()

            set({
              assistants,
              version: versionInfo.version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to load hub assistants',
              loading: false,
            })
          }
        },

        refreshFromGitHub: async () => {
          set({ loading: true, error: null })
          try {
            // Call category-specific refresh endpoint
            const result = await ApiClient.Hub.refreshAssistants()

            // Reload if updated
            if (result.updated) {
              await get().loadAssistants()
            }

            set({ loading: false })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to refresh hub assistants',
              loading: false,
            })
            throw error
          }
        },

        createFromHub: async (
          request: CreateAssistantFromHubRequest,
        ): Promise<Assistant> => {
          set({ creating: true, error: null })
          try {
            const response = await ApiClient.Hub.createAssistantFromHub(request)

            // Update the hub assistant's created_ids directly from response
            set(state => {
              const assistant = state.assistants.find(
                a => a.id === request.hub_id,
              )
              if (assistant) {
                if (!assistant.created_ids) {
                  assistant.created_ids = []
                }
                assistant.created_ids.push(response.hub_tracking.entity_id)
              }
              state.creating = false
            })

            // Notify downstream caches (UserAssistants store, settings
            // pages) that a new user assistant exists. Without this,
            // navigating to /settings/assistants after install doesn't
            // surface the new row until manual refresh.
            try {
              await emitAssistantCreated(response.assistant)
            } catch (e) {
              console.warn('Failed to emit assistant.created:', e)
            }

            return response.assistant
          } catch (error: any) {
            set({
              error: error.message || 'Failed to create assistant from hub',
              creating: false,
            })
            throw error
          }
        },

        createTemplateFromHub: async (
          request: CreateAssistantFromHubRequest,
        ): Promise<Assistant> => {
          set({ creating: true, error: null })
          try {
            const response =
              await ApiClient.Hub.createAssistantTemplateFromHub(request)

            // Track the install on the hub assistant so the card can
            // surface a "Template installed" indicator + disable the
            // re-install button. Without this an admin clicking the
            // button twice silently creates a duplicate template
            // (backend also rejects with 409 as a safety net).
            set(state => {
              const assistant = state.assistants.find(
                a => a.id === request.hub_id,
              )
              if (assistant) {
                if (!assistant.created_template_ids) {
                  assistant.created_template_ids = []
                }
                assistant.created_template_ids.push(
                  response.hub_tracking.entity_id,
                )
              }
              state.creating = false
            })

            // Notify downstream caches (TemplateAssistants store, admin
            // template-list page) that a new template exists, so the
            // admin lands on /settings/assistant-templates with the
            // new row already visible instead of stale data.
            try {
              await emitAssistantTemplateCreated(response.assistant)
            } catch (e) {
              console.warn('Failed to emit assistant_template.created:', e)
            }

            return response.assistant
          } catch (error: any) {
            set({
              error:
                error.message || 'Failed to create assistant template from hub',
              creating: false,
            })
            throw error
          }
        },

        __init__: {
          __store__: () => {
            Stores.EventBus.on(
              'assistant.deleted',
              event => {
                const { assistantId } = event.data
                set(state => {
                  for (const assistant of state.assistants) {
                    if (assistant.created_ids) {
                      assistant.created_ids = assistant.created_ids.filter(
                        id => id !== assistantId,
                      )
                    }
                  }
                })
              },
              'HubAssistantsStore',
            )
            // Symmetric to the user-assistant listener above. Without
            // this, deleting a template (via the admin templates page)
            // leaves the "Template installed" tag + disabled button
            // stuck on the corresponding hub card until full reload.
            Stores.EventBus.on(
              'assistant_template.deleted',
              event => {
                const { templateId } = event.data
                set(state => {
                  for (const assistant of state.assistants) {
                    if (assistant.created_template_ids) {
                      assistant.created_template_ids =
                        assistant.created_template_ids.filter(
                          id => id !== templateId,
                        )
                    }
                  }
                })
              },
              'HubAssistantsStore',
            )
          },
          assistants: () => get().loadAssistants(),
        },

        // Unsubscribe from EventBus on store destroy so listener slots
        // don't accumulate per destroy/re-init cycle. (audit 09 B-9)
        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('HubAssistantsStore')
        },
      }),
    ),
  ),
)
