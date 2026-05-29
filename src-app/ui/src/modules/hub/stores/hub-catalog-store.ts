import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  Catalog,
  HubCategory,
  HubCatalogVersionResponse,
  HubReleaseInfo,
  IndexItem,
} from '@/api-client/types'
import { Stores } from '@/core/stores'
import { useHubModelsStore } from '@/modules/hub/modules/llm-models/stores/hub-models-store'
import { useHubAssistantsStore } from '@/modules/hub/modules/assistants/stores/hub-assistants-store'
import { useHubMcpServersStore } from '@/modules/hub/modules/mcp/stores/hub-mcp-servers-store'

/**
 * After the active catalog version changes (refresh / activate), the
 * per-category tab lists are stale — they were loaded against the old
 * `current/`. Re-pull all three so every tab reflects the new version.
 * The per-category endpoints serve from the rotated `current/` dir, so
 * a plain reload picks up the switch.
 */
async function reloadAllTabs(): Promise<void> {
  await Promise.allSettled([
    useHubModelsStore.getState().loadModels(),
    useHubAssistantsStore.getState().loadAssistants(),
    useHubMcpServersStore.getState().loadServers(),
  ])
}

/**
 * Result of a per-item compat check (see compatOf below).
 * `ok` = no min_ziee_version, or server >= min; `too_old` = surface in
 * the Incompatible(N) footer with install disabled.
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
  loadCatalog: () => Promise<void>
  loadVersion: () => Promise<void>
  refresh: () => Promise<void>
  loadReleases: () => Promise<void>
  activateVersion: (version: string | null) => Promise<void>
  itemsByCategory: (category: HubCategory) => IndexItem[]

  // Lazy initialization
  __init__: {
    catalog: () => Promise<void>
    serverVersion: () => Promise<void>
    hubVersion: () => Promise<void>
    counts: () => Promise<void>
  }
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

        loadCatalog: async () => {
          if (get().loading) return
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
            await get().loadCatalog()
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
            await ApiClient.Hub.activateVersion({ version: version ?? undefined })
            await get().loadCatalog()
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
          catalog: () => get().loadCatalog(),
          serverVersion: () => get().loadVersion(),
          hubVersion: () => get().loadVersion(),
          counts: () => get().loadVersion(),
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

export function compatOf(item: IndexItem, serverVersion: string | null): Compat {
  if (!item.min_ziee_version) return { status: 'ok' }
  if (!serverVersion) return { status: 'ok' } // not loaded yet — don't gate
  return semverCompare(serverVersion, item.min_ziee_version) >= 0
    ? { status: 'ok' }
    : { status: 'too_old', required: item.min_ziee_version }
}

/**
 * Partition a category's items into compatible + incompatible buckets.
 * Tabs render compatible as the main list and incompatible inside a
 * collapsed `<Collapse header="Incompatible (N)">` footer with install
 * disabled.
 */
export function partitionByCompat(
  items: IndexItem[],
  serverVersion: string | null,
): { compatible: IndexItem[]; incompatible: IndexItem[] } {
  const compatible: IndexItem[] = []
  const incompatible: IndexItem[] = []
  for (const it of items) {
    ;(compatibleStatus(it, serverVersion) ? compatible : incompatible).push(it)
  }
  return { compatible, incompatible }
}

function compatibleStatus(
  item: IndexItem,
  serverVersion: string | null,
): boolean {
  return compatOf(item, serverVersion).status === 'ok'
}

// Re-export Stores helper consumer side knows about; nothing here needs
// the runtime `Stores` object yet, but the import is kept for future
// use (e.g. listening to hub.catalog_refreshed events).
export const _StoresRef = Stores
