import { ApiClient } from '@/api-client'
import type {
  Catalog,
  HubCatalogVersionResponse,
  HubCategory,
  IndexItem,
} from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { useHubAssistantsStore } from '@/modules/hub/modules/assistants/stores/hub-assistants-store'
import { useHubModelsStore } from '@/modules/hub/modules/llm-models/stores/hub-models-store'
import { useHubMcpServersStore } from '@/modules/hub/modules/mcp/stores/hub-mcp-servers-store'

/**
 * After the active catalog version changes (refresh / activate), the per-category
 * tab lists are stale. Re-pull all three (forced) so every tab reflects the new
 * version. Self-gated: the three tab reads don't self-gate, and `hub::models::read`
 * is admin-only, so a non-admin reconnect would 403 without this.
 */
export async function reloadAllTabs(): Promise<void> {
  if (
    !hasPermissionNow({
      allOf: [
        Permissions.HubModelsRead,
        Permissions.HubAssistantsRead,
        Permissions.HubMCPServersRead,
      ],
    })
  ) {
    return
  }
  await Promise.allSettled([
    useHubModelsStore.getState().loadModels(true),
    useHubAssistantsStore.getState().loadAssistants(true),
    useHubMcpServersStore.getState().loadServers(true),
  ])
}

/**
 * Result of a per-item compat check. `ok` = no min_ziee_version, or server >=
 * min; `too_old` = the item requires a newer ziee server and is hidden.
 */
export type Compat = { status: 'ok' } | { status: 'too_old'; required: string }

export const HubCatalog = defineStore('HubCatalog', {
  immer: true,
  state: {
    catalog: null as Catalog | null,
    serverVersion: null as string | null,
    hubVersion: null as string | null,
    counts: null as HubCatalogVersionResponse['counts'] | null,
    loading: false,
    refreshing: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadCatalog = async (force = false) => {
      // `force` lets refresh bypass the in-flight guard.
      if (get().loading && !force) return
      // GET /hub/index requires hub::catalog::read — gate on the real endpoint
      // perm; short-circuit for non-catalog-readers (the catalog degrades).
      if (!hasPermissionNow(Permissions.HubCatalogRead)) return
      set({ loading: true, error: null })
      try {
        const catalog = await ApiClient.Hub.getCatalog()
        set({ catalog, hubVersion: catalog.hub_version, loading: false })
      } catch (error: any) {
        set({ error: error?.message || 'Failed to load hub catalog', loading: false })
      }
    }
    const loadVersion = async () => {
      // GET /hub/version requires hub::catalog::read — skip for non-readers.
      if (!hasPermissionNow(Permissions.HubCatalogRead)) return
      try {
        const v = await ApiClient.Hub.getCatalogVersion()
        set({ serverVersion: v.server_version, hubVersion: v.hub_version, counts: v.counts })
      } catch (error: any) {
        set({ error: error?.message || 'Failed to load hub version' })
      }
    }
    return {
      loadCatalog,
      loadVersion,
      refresh: async () => {
        set({ refreshing: true, error: null })
        try {
          const outcome = await ApiClient.Hub.refreshCatalog()
          // Re-pull even when nothing advanced so generated_at + counts stay fresh.
          await loadCatalog(true)
          await loadVersion()
          await reloadAllTabs()
          set({ refreshing: false })
          return outcome as unknown as void
        } catch (error: any) {
          set({ error: error?.message || 'Failed to refresh hub catalog', refreshing: false })
          throw error
        }
      },
      itemsByCategory: (category: HubCategory): IndexItem[] => {
        const catalog = get().catalog
        if (!catalog) return []
        return catalog.items.filter(it => it.category === category)
      },
    }
  },
  init: ({ on, actions }) => {
    // Under Hub v2 the catalog version isn't pinnable, so a hub_settings change
    // (or reconnect) just refetches the version marker + reloads every tab.
    const handleHubSettingsChange = () => {
      void actions.loadVersion()
      void reloadAllTabs()
    }
    on('sync:hub_settings', handleHubSettingsChange)
    on('sync:reconnect', handleHubSettingsChange)
    void actions.loadCatalog()
    void actions.loadVersion()
  },
})

export const useHubCatalogStore = HubCatalog.store

// -------- helpers consumable by per-tab components --------

/**
 * Lightweight semver compare (-1/0/1). Pre-release suffix treated as "older than
 * the same X.Y.Z without the suffix". Both sides garbled → 0 (compatible).
 */
function semverCompare(a: string, b: string): number {
  const parse = (s: string): [number[], string | null] => {
    const dash = s.indexOf('-')
    const core = dash >= 0 ? s.slice(0, dash) : s
    const pre = dash >= 0 ? s.slice(dash + 1) : null
    const parts = core.split('.').map(p => Number(p))
    if (parts.some(p => Number.isNaN(p))) return [[], null]
    return [parts, pre]
  }
  const [pa, prea] = parse(a)
  const [pb, preb] = parse(b)
  if (pa.length === 0 || pb.length === 0) return 0
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? 0
    const y = pb[i] ?? 0
    if (x < y) return -1
    if (x > y) return 1
  }
  // Equal cores → pre-release is OLDER than no-pre-release.
  if (prea && !preb) return -1
  if (!prea && preb) return 1
  return 0
}

/**
 * Compat check for one catalog item against the running server. Returns `ok`
 * when the server version hasn't loaded yet so nothing flickers hidden.
 */
export function compatOf(item: IndexItem, serverVersion: string | null): Compat {
  if (!item.min_ziee_version) return { status: 'ok' }
  if (!serverVersion) return { status: 'ok' } // not loaded yet — don't gate
  return semverCompare(serverVersion, item.min_ziee_version) >= 0
    ? { status: 'ok' }
    : { status: 'too_old', required: item.min_ziee_version }
}
