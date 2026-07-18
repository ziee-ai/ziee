import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { type FileVersion, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'

enableMapSet()

/**
 * Per-file version history + restore. Kept separate from `File.store` so the
 * (large) attachment/upload store stays focused. Live-updates via `sync:file`
 * (a restore / MCP edit / sandbox version-back on this or another device).
 */
export const FileVersions = defineStore('FileVersions', {
  immer: true,
  state: {
    versionsByFile: new Map<string, FileVersion[]>(),
    versionsLoadingSet: new Set<string>(),
    /** Cached text of a specific version, keyed `${fileId}:${version}`. */
    versionTextCache: new Map<string, string>(),
    versionTextLoadingSet: new Set<string>(),
  },
  actions: (set, get) => {
    const loadVersions = async (fileId: string): Promise<void> => {
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users without `files::read` (the endpoint would 403).
      if (!hasPermissionNow(Permissions.FilesRead)) return
      set(s => {
        const ls = new Set(s.versionsLoadingSet)
        ls.add(fileId)
        s.versionsLoadingSet = ls
      })
      try {
        const versions = await ApiClient.File.listVersions({ file_id: fileId })
        set(s => {
          const m = new Map(s.versionsByFile)
          m.set(fileId, versions)
          s.versionsByFile = m
          const ls = new Set(s.versionsLoadingSet)
          ls.delete(fileId)
          s.versionsLoadingSet = ls
        })
      } catch (e) {
        set(s => {
          // Cache an empty list so a render-triggered getVersions() doesn't
          // re-attempt on every frame. A sync event re-runs loadVersions.
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
    }
    return {
      loadVersions,
      /** Render-safe: returns cached versions, triggering a background load. */
      getVersions: (fileId: string): FileVersion[] => {
        const cached = get().versionsByFile.get(fileId)
        if (!cached && !get().versionsLoadingSet.has(fileId)) {
          Promise.resolve().then(() => loadVersions(fileId))
        }
        return cached ?? []
      },
      /** Append a new head equal to `version` (no-op if it's already head). */
      restoreVersion: async (fileId: string, version: number): Promise<void> => {
        await ApiClient.File.restore({ file_id: fileId, version })
        await loadVersions(fileId)
        // Refresh the head entity shown in panels / cards.
        try {
          await Stores.File.loadMessageFile(fileId)
        } catch {
          /* best-effort */
        }
      },
      /**
       * Save `content` as a new head version — the user side of co-editing a
       * deliverable. Byte-identical content is a server-side no-op. Refreshes the
       * version list + head entity so the panel/version-bar reflect the save.
       */
      appendVersion: async (fileId: string, content: string): Promise<void> => {
        await ApiClient.File.appendVersion({ file_id: fileId, content })
        await loadVersions(fileId)
        try {
          await Stores.File.loadMessageFile(fileId)
        } catch {
          /* best-effort */
        }
      },
      /** Render-safe: returns cached text for a non-head version, triggering a load. */
      getVersionText: (fileId: string, version: number): string | null => {
        const key = `${fileId}:${version}`
        const cached = get().versionTextCache.get(key)
        if (cached === undefined && !get().versionTextLoadingSet.has(key)) {
          Promise.resolve().then(async () => {
            set(s => {
              const ls = new Set(s.versionTextLoadingSet)
              ls.add(key)
              s.versionTextLoadingSet = ls
            })
            try {
              const res = await ApiClient.File.textVersion({
                file_id: fileId,
                version: String(version),
              })
              // The api-client returns a string for text/* responses (the
              // typed `Blob` is nominal); guard like File.store's text loaders
              // so calling `.text()` on a plain string doesn't throw.
              const text =
                typeof res === 'string' ? res : await (res as Blob).text()
              set(s => {
                const m = new Map(s.versionTextCache)
                m.set(key, text)
                s.versionTextCache = m
                const ls = new Set(s.versionTextLoadingSet)
                ls.delete(key)
                s.versionTextLoadingSet = ls
              })
            } catch (e) {
              set(s => {
                // Cache an error sentinel so getVersionText() doesn't retry on
                // every render; also distinguishes error from loading (null).
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
    }
  },
  init: ({ on, get, set, actions }) => {
    // Drop cached version TEXT (incl. error sentinels) so a reopened/old-version
    // view refetches fresh. Cheap (small cache).
    const dropVersionText = () =>
      set(s => {
        s.versionTextCache = new Map()
        s.versionTextLoadingSet = new Set()
      })
    // A specific file changed (restore / MCP edit / sandbox version-back). Only
    // REFETCH that file's version list — not every tracked file's.
    on('sync:file', (event: { data?: { id?: string } }) => {
      const fileId = event?.data?.id
      dropVersionText()
      if (fileId && get().versionsByFile.has(fileId)) {
        void actions.loadVersions(fileId)
      } else if (!fileId) {
        // Defensive: a sync:file with no id → reload all tracked.
        Array.from(get().versionsByFile.keys()).forEach(fid => void actions.loadVersions(fid))
      }
    })
    // Reconnect: we may have missed events → reload EVERY tracked file.
    on('sync:reconnect', () => {
      dropVersionText()
      Array.from(get().versionsByFile.keys()).forEach(fid => void actions.loadVersions(fid))
    })
  },
})

export const useFileVersionsStore = FileVersions.store
