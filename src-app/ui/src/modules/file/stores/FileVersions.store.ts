import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type { FileVersion } from '@/api-client/types'

enableMapSet()

/**
 * Per-file version history + restore.
 *
 * Kept separate from `File.store` so the (large) attachment/upload store stays
 * focused. Live-updates via the `sync:file` event (a restore / MCP edit /
 * sandbox version-back on this or another device).
 */
interface FileVersionsStore {
  versionsByFile: Map<string, FileVersion[]>
  versionsLoadingSet: Set<string>
  /** Cached text of a specific version, keyed `${fileId}:${version}`. */
  versionTextCache: Map<string, string>
  versionTextLoadingSet: Set<string>

  /** Render-safe: returns cached versions, triggering a background load. */
  getVersions: (fileId: string) => FileVersion[]
  loadVersions: (fileId: string) => Promise<void>
  /** Append a new head equal to `version` (no-op if it's already head). */
  restoreVersion: (fileId: string, version: number) => Promise<void>
  /** Render-safe: returns cached text for a non-head version, triggering a load. */
  getVersionText: (fileId: string, version: number) => string | null

  __init__: { __store__: () => void }
  __destroy__: () => void
}

export const useFileVersionsStore = create<FileVersionsStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      versionsByFile: new Map(),
      versionsLoadingSet: new Set(),
      versionTextCache: new Map(),
      versionTextLoadingSet: new Set(),

      getVersions: (fileId: string): FileVersion[] => {
        const cached = get().versionsByFile.get(fileId)
        if (!cached && !get().versionsLoadingSet.has(fileId)) {
          Promise.resolve().then(() => get().loadVersions(fileId))
        }
        return cached ?? []
      },

      loadVersions: async (fileId: string): Promise<void> => {
        set((s) => {
          const ls = new Set(s.versionsLoadingSet)
          ls.add(fileId)
          s.versionsLoadingSet = ls
        })
        try {
          const versions = await ApiClient.File.listVersions({ file_id: fileId })
          set((s) => {
            const m = new Map(s.versionsByFile)
            m.set(fileId, versions)
            s.versionsByFile = m
            const ls = new Set(s.versionsLoadingSet)
            ls.delete(fileId)
            s.versionsLoadingSet = ls
          })
        } catch (e) {
          set((s) => {
            // Cache an empty list so a render-triggered getVersions() doesn't
            // re-attempt the failed load on every frame. A `sync:file` /
            // `sync:reconnect` event re-runs loadVersions to recover.
            if (!s.versionsByFile.has(fileId)) {
              const m = new Map(s.versionsByFile)
              m.set(fileId, [])
              s.versionsByFile = m
            }
            const ls = new Set(s.versionsLoadingSet)
            ls.delete(fileId)
            s.versionsLoadingSet = ls
          })
          console.error('[FileVersions] failed to load versions', fileId, e)
        }
      },

      restoreVersion: async (fileId: string, version: number): Promise<void> => {
        await ApiClient.File.restore({ file_id: fileId, version })
        await get().loadVersions(fileId)
        // Refresh the head entity shown in panels / cards.
        try {
          await Stores.File.loadMessageFile(fileId)
        } catch {
          /* best-effort */
        }
      },

      getVersionText: (fileId: string, version: number): string | null => {
        const key = `${fileId}:${version}`
        const cached = get().versionTextCache.get(key)
        if (cached === undefined && !get().versionTextLoadingSet.has(key)) {
          Promise.resolve().then(async () => {
            set((s) => {
              const ls = new Set(s.versionTextLoadingSet)
              ls.add(key)
              s.versionTextLoadingSet = ls
            })
            try {
              const { getAuthToken } = await import('@/api-client/core')
              const token = getAuthToken()
              const res = await fetch(
                `/api/files/${fileId}/versions/${version}/text`,
                { headers: token ? { Authorization: `Bearer ${token}` } : {} },
              )
              const text = res.ok ? await res.text() : `[failed to load v${version}]`
              set((s) => {
                const m = new Map(s.versionTextCache)
                m.set(key, text)
                s.versionTextCache = m
                const ls = new Set(s.versionTextLoadingSet)
                ls.delete(key)
                s.versionTextLoadingSet = ls
              })
            } catch (e) {
              set((s) => {
                // Cache an error sentinel so getVersionText() doesn't retry on
                // every render (an uncached null would loop forever); also makes
                // the error distinguishable from the loading state (null).
                const m = new Map(s.versionTextCache)
                m.set(key, `[error loading v${version}]`)
                s.versionTextCache = m
                const ls = new Set(s.versionTextLoadingSet)
                ls.delete(key)
                s.versionTextLoadingSet = ls
              })
              console.error('[FileVersions] failed to load version text', key, e)
            }
          })
        }
        return cached ?? null
      },

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'FileVersions'
          // Drop cached version TEXT (including error sentinels from a prior
          // failed fetch) so a reopened/old-version view refetches fresh. Cheap
          // (small cache) so we clear it wholesale either way.
          const dropVersionText = () =>
            set((s) => {
              s.versionTextCache = new Map()
              s.versionTextLoadingSet = new Set()
            })
          // A specific file changed (restore / MCP edit / sandbox version-back).
          // Only REFETCH that file's version list (the expensive part) — not
          // every tracked file's.
          const onFileSync = (event: { data?: { id?: string } }) => {
            const fileId = event?.data?.id
            dropVersionText()
            if (fileId && get().versionsByFile.has(fileId)) {
              void get().loadVersions(fileId)
            } else if (!fileId) {
              // Defensive: a sync:file with no id → reload all tracked.
              Array.from(get().versionsByFile.keys()).forEach((fid) => void get().loadVersions(fid))
            }
          }
          // Reconnect: we may have missed events → reload EVERY tracked file.
          const onReconnect = () => {
            dropVersionText()
            Array.from(get().versionsByFile.keys()).forEach((fid) => void get().loadVersions(fid))
          }
          eventBus.on('sync:file', onFileSync, GROUP)
          eventBus.on('sync:reconnect', onReconnect, GROUP)
        },
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('FileVersions')
      },
    })),
  ),
)
