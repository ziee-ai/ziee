import { defineStore } from '@/core/store-kit'
import { Stores } from '@/core/stores'
import { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import { useHubInstalledStore } from '@/modules/hub/stores/hub-installed-store'
import { useSystemWorkflowStore } from '@/modules/workflow/stores/SystemWorkflow.store'
import { useWorkflowStore } from '@/modules/workflow/stores/Workflow.store'

/**
 * Hub-workflows tab store. Workflows have no category-specific catalog endpoint,
 * so the listing comes from the shared HubCatalog (`itemsByCategory('workflow')`)
 * and install-tracking from the shared installed-store. Install actions delegate
 * to the workflow module's user / system stores.
 */
export const HubWorkflows = defineStore('HubWorkflows', {
  immer: true,
  state: {
    installing: {} as Record<string, boolean>,
    error: null as string | null,
  },
  actions: set => {
    const installForGroups = async (hubId: string, groups: string[]) => {
      set(draft => {
        draft.installing[hubId] = true
        draft.error = null
      })
      try {
        await useSystemWorkflowStore.getState().installSystemFromHub(hubId, groups)
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
    }
    return {
      installForGroups,
      installStateFor: (name: string): 'none' | 'user' | 'system' => {
        const rows = useHubInstalledStore
          .getState()
          .items.filter(r => r.hub_id === name && r.hub_category === 'workflow')
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
        await installForGroups(hubId, [])
      },
      refresh: async () => {
        await useHubCatalogStore.getState().refresh()
        await useHubInstalledStore.getState().loadInstalled()
      },
    }
  },
  init: () => {
    void Stores.HubCatalog.loadCatalog()
    void Stores.HubInstalled.loadInstalled()
  },
})

export const useHubWorkflowsStore = HubWorkflows.store
