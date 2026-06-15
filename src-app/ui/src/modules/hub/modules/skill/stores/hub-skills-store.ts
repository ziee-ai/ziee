import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { IndexItem } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import { useHubInstalledStore } from '@/modules/hub/stores/hub-installed-store'
import { useSkillStore } from '@/modules/skill/stores/Skill.store'
import { useSystemSkillStore } from '@/modules/skill/stores/SystemSkill.store'

/**
 * Hub-skills tab store. Skills have no category-specific catalog
 * endpoint (unlike models / assistants / mcp-servers), so the listing
 * is derived from the shared HubCatalog (`itemsByCategory('skill')`)
 * and install-tracking comes from the shared installed-store. Install
 * actions delegate to the skill module's user / system stores.
 */
interface HubSkillsState {
  installing: Record<string, boolean>
  error: string | null

  items: () => IndexItem[]
  /** Returns 'none' | 'user' | 'system' for a hub skill name. */
  installStateFor: (name: string) => 'none' | 'user' | 'system'
  installForMe: (hubId: string) => Promise<void>
  installForEveryone: (hubId: string) => Promise<void>
  installForGroups: (hubId: string, groups: string[]) => Promise<void>
  refresh: () => Promise<void>

  __init__: {
    __store__?: () => void
  }
}

export const useHubSkillsStore = create<HubSkillsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubSkillsState => ({
        installing: {},
        error: null,

        items: () => useHubCatalogStore.getState().itemsByCategory('skill'),

        installStateFor: (name: string) => {
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
          await get().installForGroups(hubId, [])
        },

        installForGroups: async (hubId: string, groups: string[]) => {
          set(draft => {
            draft.installing[hubId] = true
            draft.error = null
          })
          try {
            await useSystemSkillStore
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
            // Ensure the shared catalog + installed lists are loaded so
            // the tab has data on first render.
            void Stores.HubCatalog.loadCatalog()
            void Stores.HubInstalled.loadInstalled()
          },
        },
      }),
    ),
  ),
)
