import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { Stores } from '@/core/stores'
import { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import { useHubInstalledStore } from '@/modules/hub/stores/hub-installed-store'
import { useSystemWorkflowStore } from '@/modules/workflow/stores/SystemWorkflow.store'
import { useWorkflowStore } from '@/modules/workflow/stores/Workflow.store'

/**
 * Hub-workflows tab store. Workflows have no category-specific catalog
 * endpoint, so the listing comes from the shared HubCatalog
 * (`itemsByCategory('workflow')`) and install-tracking from the shared
 * installed-store. Install actions delegate to the workflow module's
 * user / system stores.
 */
interface HubWorkflowsState {
  installing: Record<string, boolean>
  error: string | null

  installStateFor: (name: string) => 'none' | 'user' | 'system'
  installForMe: (hubId: string) => Promise<void>
  installForEveryone: (hubId: string) => Promise<void>
  installForGroups: (hubId: string, groups: string[]) => Promise<void>
  refresh: () => Promise<void>

  __init__: {
    __store__?: () => void
  }
}

export const useHubWorkflowsStore = create<HubWorkflowsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubWorkflowsState => ({
        installing: {},
        error: null,

        installStateFor: (name: string) => {
          const rows = useHubInstalledStore
            .getState()
            .items.filter(
              r => r.hub_id === name && r.hub_category === 'workflow',
            )
          if (rows.some(r => r.is_system)) return 'system'
          if (rows.length > 0) return 'user'
          return 'none'
        },

        installForMe: async (hubId: string) => {
          set(draft => {
            draft.installing[hubId] = true
            draft.error = null
          })
          try {
            await useWorkflowStore.getState().installFromHub(hubId)
            await useHubInstalledStore.getState().loadInstalled()
          } catch (e) {
            set(draft => {
              draft.error = e instanceof Error ? e.message : 'Install failed'
            })
            throw e
          } finally {
            set(draft => {
              delete draft.installing[hubId]
            })
          }
        },

        installForEveryone: async (hubId: string) => {
          await get().installForGroups(hubId, [])
        },

        installForGroups: async (hubId: string, groups: string[]) => {
          set(draft => {
            draft.installing[hubId] = true
            draft.error = null
          })
          try {
            await useSystemWorkflowStore
              .getState()
              .installSystemFromHub(hubId, groups)
            await useHubInstalledStore.getState().loadInstalled()
          } catch (e) {
            set(draft => {
              draft.error = e instanceof Error ? e.message : 'Install failed'
            })
            throw e
          } finally {
            set(draft => {
              delete draft.installing[hubId]
            })
          }
        },

        refresh: async () => {
          await useHubCatalogStore.getState().refresh()
          await useHubInstalledStore.getState().loadInstalled()
        },

        __init__: {
          __store__: () => {
            void Stores.HubCatalog.loadCatalog()
            void Stores.HubInstalled.loadInstalled()
          },
        },
      }),
    ),
  ),
)
