import { ApiClient } from '@/api-client'
import type {
  BatchReport,
  BibliographyEntry,
  CitationInput,
} from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

export const Citations = defineStore('Citations', {
  immer: true,
  state: {
    entries: [] as BibliographyEntry[],
    loading: false,
    importing: false,
    verifying: false,
    error: null as string | null,
    /** When set, the store scopes to a project's reference list. */
    projectId: null as string | null,
  },
  actions: (set, get) => {
    const loadEntries = async (projectId?: string | null) => {
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users without `citations::use` (the endpoint would 403).
      if (!hasPermissionNow(Permissions.CitationsUse)) return
      const pid = projectId !== undefined ? projectId : get().projectId
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const resp = await ApiClient.Citations.list(pid ? { project_id: pid } : {})
        set(s => {
          s.entries = resp.entries
          s.loading = false
        })
      } catch (error) {
        set(s => {
          s.error = error instanceof Error ? error.message : 'Failed to load citations'
          s.loading = false
        })
      }
    }
    return {
      loadEntries,
      load: (projectId?: string | null) => loadEntries(projectId),
      importItems: async (items: CitationInput[], projectId?: string | null) => {
        set(s => {
          s.importing = true
          s.error = null
        })
        try {
          const pid = projectId !== undefined ? projectId : get().projectId
          const report = await ApiClient.Citations.import({
            items,
            ...(pid ? { project_id: pid } : {}),
          })
          await loadEntries()
          set(s => {
            s.importing = false
          })
          return report
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Import failed'
            s.importing = false
          })
          throw error
        }
      },
      // Re-resolve every stored entry server-side and PERSIST the new status;
      // then reload so badges reflect persisted statuses. Covers ALL entries
      // (incl. identifier-less, re-resolved by title).
      verifyAll: async (): Promise<BatchReport> => {
        set(s => {
          s.verifying = true
          s.error = null
        })
        try {
          const pid = get().projectId
          const report = await ApiClient.Citations.reverify(pid ? { project_id: pid } : {})
          await loadEntries()
          set(s => {
            s.verifying = false
          })
          return report
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Verify failed'
            s.verifying = false
          })
          throw error
        }
      },
      remove: async (id: string) => {
        try {
          await ApiClient.Citations.delete({ id })
          set(s => {
            s.entries = s.entries.filter(e => e.id !== id)
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Delete failed'
          })
          throw error
        }
      },
      exportLibrary: async (format: string, style?: string, projectId?: string | null) => {
        const pid = projectId !== undefined ? projectId : get().projectId
        const resp = await ApiClient.Citations.export({
          format,
          ...(style ? { style } : {}),
          ...(pid ? { project_id: pid } : {}),
        })
        return resp.output
      },
      setProjectId: (id: string | null) =>
        set(s => {
          s.projectId = id
        }),
    }
  },
  init: ({ on, actions }) => {
    // Notify-and-refetch: the backend emits `BibliographyEntry` (owner-scoped)
    // on add/import/delete/attach; refetch on reconnect too.
    const reload = () => void actions.loadEntries()
    on('sync:bibliography_entry', reload)
    on('sync:reconnect', reload)
    void actions.loadEntries()
  },
})

export const useCitationsStore = Citations.store
