import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  BatchReport,
  BibliographyEntry,
  CitationInput,
} from '@/api-client/types'
import { Stores } from '@/core/stores'

interface CitationsStore {
  entries: BibliographyEntry[]
  loading: boolean
  importing: boolean
  verifying: boolean
  error: string | null
  /** When set, the store scopes to a project's reference list. */
  projectId: string | null

  __init__: {
    __store__?: () => void
    entries: () => Promise<void>
  }
  __destroy__?: () => void

  load: (projectId?: string | null) => Promise<void>
  importItems: (
    items: CitationInput[],
    projectId?: string | null,
  ) => Promise<BatchReport>
  /** Re-resolve every stored entry and PERSIST the new status (the library's
   *  "Verify all"). Returns the per-item report; badges update via reload. */
  verifyAll: () => Promise<BatchReport>
  remove: (id: string) => Promise<void>
  exportLibrary: (
    format: string,
    style?: string,
    projectId?: string | null,
  ) => Promise<string>
  setProjectId: (id: string | null) => void
}

const loadEntries = async (
  set: (fn: (s: CitationsStore) => void) => void,
  get: () => CitationsStore,
  projectId?: string | null,
) => {
  const pid = projectId !== undefined ? projectId : get().projectId
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const resp = await ApiClient.Citations.list(
      pid ? { project_id: pid } : {},
    )
    set(s => {
      s.entries = resp.entries
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load citations'
      s.loading = false
    })
  }
}

export const useCitationsStore = create<CitationsStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      entries: [],
      loading: false,
      importing: false,
      verifying: false,
      error: null,
      projectId: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const reload = () => void loadEntries(set, get)
          // Notify-and-refetch: the backend emits `BibliographyEntry`
          // (owner-scoped) on add/import/delete/attach; refetch on reconnect too.
          eventBus.on('sync:bibliography_entry', reload, 'Citations')
          eventBus.on('sync:reconnect', reload, 'Citations')
        },
        entries: () => loadEntries(set, get),
      },
      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('Citations')
      },

      load: (projectId?: string | null) => loadEntries(set, get, projectId),

      importItems: async (items, projectId) => {
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
          await loadEntries(set, get)
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
      // then reload so the badges reflect the persisted statuses. Covers ALL
      // entries (incl. identifier-less, re-resolved by title) — the server picks
      // each entry's best identifier.
      verifyAll: async () => {
        set(s => {
          s.verifying = true
          s.error = null
        })
        try {
          const pid = get().projectId
          const report = await ApiClient.Citations.reverify(
            pid ? { project_id: pid } : {},
          )
          await loadEntries(set, get)
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

      remove: async id => {
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

      exportLibrary: async (format, style, projectId) => {
        const pid = projectId !== undefined ? projectId : get().projectId
        const resp = await ApiClient.Citations.export({
          format,
          ...(style ? { style } : {}),
          ...(pid ? { project_id: pid } : {}),
        })
        return resp.output
      },

      setProjectId: id =>
        set(s => {
          s.projectId = id
        }),
    })),
  ),
)
