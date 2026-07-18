import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import { useHubInstalledStore } from '@/modules/hub/stores/hub-installed-store'
import { useSkillStore } from '@/modules/skill/stores/Skill.store'
import { useSystemSkillStore } from '@/modules/skill/stores/SystemSkill.store'

/**
 * Hub-skills tab store. Skills have no category-specific catalog endpoint, so
 * the listing is derived from the shared HubCatalog (`itemsByCategory('skill')`)
 * and install-tracking from the shared installed-store. Install actions delegate
 * to the skill module's user / system stores.
 */
export const HubSkills = defineStore('HubSkills', {
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
        await useSystemSkillStore.getState().installSystemFromHub(hubId, groups)
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
      /** Returns 'none' | 'user' | 'system' for a hub skill name. */
      installStateFor: (name: string): 'none' | 'user' | 'system' => {
        const rows = useHubInstalledStore
          .getState()
          .items.filter(r => r.hub_id === name && r.hub_category === 'skill')
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
          await useSkillStore.getState().installFromHub(hubId)
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
    // Ensure the shared catalog + installed lists are loaded so the tab has data.
    void Stores.HubCatalog.loadCatalog()
    void Stores.HubInstalled.loadInstalled()
  },
})

export const useHubSkillsStore = HubSkills.store
