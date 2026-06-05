import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  Catalog,
  HubCatalogVersionResponse,
  HubCategory,
  HubReleaseInfo,
  IndexItem,
} from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { useHubAssistantsStore } from '@/modules/hub/modules/assistants/stores/hub-assistants-store'
import { useHubModelsStore } from '@/modules/hub/modules/llm-models/stores/hub-models-store'
import { useHubMcpServersStore } from '@/modules/hub/modules/mcp/stores/hub-mcp-servers-store'

/**
 * After the active catalog version changes (refresh / activate), the
 * per-category tab lists are stale — they were loaded against the old
 * `current/`. Re-pull all three so every tab reflects the new version.
 * Forced (true) so an in-flight first-load doesn't make the reload
 * early-return on the `loading` guard and leave a tab showing the old
 * version. The per-category endpoints serve from the rotated `current/`
 * dir, so a reload picks up the switch.
 */
export async function reloadAllTabs(): Promise<void> {
  // Self-gate: this refetch calls loadModels + loadAssistants + loadServers,
  // none of which self-gate. `hub::models::read` is admin-only (migration 37
  // removed it from the Users group), so a non-admin reconnect would 403 on
  // `GET /hub/models` without this guard. (NOT `hub::catalog::read` — that's
  // the admin catalog-settings perm, unrelated to the three tab reads here.)
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
 * Result of a per-item compat check (see compatOf below).
 * `ok` = no min_ziee_version, or server >= min; `too_old` = the item
 * requires a newer ziee server and is hidden from the tab + rejected
 * by the install endpoint.
 */
export type Compat = { status: 'ok' } | { status: 'too_old'; required: string }

interface HubCatalogState {
  catalog: Catalog | null
  serverVersion: string | null
  hubVersion: string | null
  counts: HubCatalogVersionResponse['counts'] | null
  loading: boolean
  refreshing: boolean
  error: string | null

  // Admin version picker (lazy — only loaded when an admin opens the
  // dropdown via loadReleases()).
  releases: HubReleaseInfo[]
  pinnedVersion: string | null
  activeVersion: string | null
  releasesLoading: boolean
  activating: boolean

  // Actions
  loadCatalog: (force?: boolean) => Promise<void>
  loadVersion: () => Promise<void>
  refresh: () => Promise<void>
  loadReleases: () => Promise<void>
  activateVersion: (version: string | null) => Promise<void>
  itemsByCategory: (category: HubCategory) => IndexItem[]

  // Lazy initialization
  __init__: {
    __store__?: () => void
    catalog: () => Promise<void>
    serverVersion: () => Promise<void>
    hubVersion: () => Promise<void>
    counts: () => Promise<void>
  }

  __destroy__?: () => void
}

export const useHubCatalogStore = create<HubCatalogState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubCatalogState => ({
        catalog: null,
        serverVersion: null,
        hubVersion: null,
        counts: null,
        loading: false,
        refreshing: false,
        error: null,
        releases: [],
        pinnedVersion: null,
        activeVersion: null,
        releasesLoading: false,
        activating: false,

        loadCatalog: async (force = false) => {
          // `force` lets refresh/activate bypass the in-flight guard so
          // a concurrent first-load doesn't make the post-switch reload
          // a no-op (which would leave the UI on the old catalog).
          if (get().loading && !force) return
          set({ loading: true, error: null })
          try {
            const catalog = await ApiClient.Hub.getCatalog()
            set({
              catalog,
              hubVersion: catalog.hub_version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error?.message || 'Failed to load hub catalog',
              loading: false,
            })
          }
        },

        loadVersion: async () => {
          try {
            const v = await ApiClient.Hub.getCatalogVersion()
            set({
              serverVersion: v.server_version,
              hubVersion: v.hub_version,
              counts: v.counts,
            })
          } catch (error: any) {
            set({ error: error?.message || 'Failed to load hub version' })
          }
        },

        refresh: async () => {
          set({ refreshing: true, error: null })
          try {
            const outcome = await ApiClient.Hub.refreshCatalog()
            // Even when nothing advanced, re-pull the catalog so the
            // generated_at timestamp + counts (server might have added
            // a sidecar after staging an air-gapped update) stay
            // fresh in the UI.
            await get().loadCatalog(true)
            await get().loadVersion()
            await reloadAllTabs()
            set({ refreshing: false })
            return outcome as unknown as void
          } catch (error: any) {
            set({
              error: error?.message || 'Failed to refresh hub catalog',
              refreshing: false,
            })
            throw error
          }
        },

        loadReleases: async () => {
          set({ releasesLoading: true, error: null })
          try {
            const resp = await ApiClient.Hub.getReleases()
            set({
              releases: resp.releases,
              activeVersion: resp.active_version ?? null,
              pinnedVersion: resp.pinned_version ?? null,
              releasesLoading: false,
            })
          } catch (error: any) {
            set({
              error: error?.message || 'Failed to list hub versions',
              releasesLoading: false,
            })
          }
        },

        // Activate a specific version server-wide (admin). `null` clears
        // the pin and tracks latest. On success, re-pulls catalog +
        // version + releases so the UI reflects the new active version.
        activateVersion: async (version: string | null) => {
          set({ activating: true, error: null })
          try {
            await ApiClient.Hub.activateVersion({
              version: version ?? undefined,
            })
            await get().loadCatalog(true)
            await get().loadVersion()
            await get().loadReleases()
            await reloadAllTabs()
            set({ activating: false })
          } catch (error: any) {
            set({
              error: error?.message || 'Failed to activate hub version',
              activating: false,
            })
            throw error
          }
        },

        itemsByCategory: (category: HubCategory) => {
          const catalog = get().catalog
          if (!catalog) return []
          return catalog.items.filter(it => it.category === category)
        },

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'HubCatalogStore'

            // The hub catalog version was pinned/refreshed (singleton; event
            // id is nil). Reload every hub category tab so stale per-category
            // lists pick up the new catalog. `reloadAllTabs` self-gates on the
            // perms it needs, so a non-admin reconnect won't 403.
            eventBus.on('sync:hub_settings', () => void reloadAllTabs(), GROUP)
            eventBus.on('sync:reconnect', () => void reloadAllTabs(), GROUP)
          },
          catalog: () => get().loadCatalog(),
          serverVersion: () => get().loadVersion(),
          hubVersion: () => get().loadVersion(),
          counts: () => get().loadVersion(),
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('HubCatalogStore')
        },
      }),
    ),
  ),
)

// -------- helpers consumable by per-tab components --------

/**
 * Lightweight semver compare. Returns -1/0/1 like a typical comparator.
 * Pre-release suffix (`-alpha`, `-rc.1`) treated as "older than the
 * same X.Y.Z without the suffix" — i.e. `0.5.0-alpha < 0.5.0`. Both
 * sides being garbled returns 0 (treat as compatible). Sufficient for
 * the compat() check; not a full semver implementation.
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
  // Both pre-release or neither: don't try to order pre-release tags.
  return 0
}

/**
 * Compat check for one catalog item against the running server. Items
 * with no `min_ziee_version`, or whose requirement the server meets,
 * are `ok`; otherwise `too_old`. Tabs hide `too_old` items entirely
 * (and the install endpoint rejects them server-side). Returns `ok`
 * when the server version hasn't loaded yet so nothing flickers hidden.
 */
export function compatOf(
  item: IndexItem,
  serverVersion: string | null,
): Compat {
  if (!item.min_ziee_version) return { status: 'ok' }
  if (!serverVersion) return { status: 'ok' } // not loaded yet — don't gate
  return semverCompare(serverVersion, item.min_ziee_version) >= 0
    ? { status: 'ok' }
    : { status: 'too_old', required: item.min_ziee_version }
}
