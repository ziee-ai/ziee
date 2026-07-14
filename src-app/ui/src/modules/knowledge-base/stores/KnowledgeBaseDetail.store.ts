import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import {
  type KnowledgeBase,
  type KnowledgeBaseDocument,
  type KnowledgeBaseSearchResponse,
  type KnowledgeBaseUsage,
  Permissions,
  type RetrievalInfo,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import type { FileUploadProgress } from '@/modules/file/stores/File.store'

enableMapSet()

/** 100 MiB — mirrors the file module's per-file cap (ProjectFilesManagePanel). */
const MAX_FILE_SIZE = 100 * 1024 * 1024

/** Default documents-per-page. Numbered pagination (discrete pages via
 *  `ListPagination`, like the users/memories settings pages) — NOT infinite
 *  scroll — so only a small page loads at a time (a KB holds up to 2000). */
const DOC_DEFAULT_PAGE_SIZE = 10

export const KnowledgeBaseDetail = defineStore('KnowledgeBaseDetail', {
  immer: true,
  state: {
    kb: null as KnowledgeBase | null,
    documents: [] as KnowledgeBaseDocument[],
    loading: false,
    documentsLoading: false,
    /** 1-based current page + page size for the documents `ListPagination`. */
    documentsPage: 1,
    documentsPageSize: DOC_DEFAULT_PAGE_SIZE,
    uploading: false,
    /** Per-file upload progress, keyed by a synthetic local id — mirrors
     *  ProjectFiles so each uploading file shows its own FileCard progress row. */
    uploadingFiles: new Map<string, FileUploadProgress>(),
    /** Multi-select for bulk remove (mirrors ProjectFiles). */
    selectedFileIds: new Set<string>(),
    error: null as string | null,
    /** Deployment retrieval mode (for the detail-page mode line). */
    retrievalInfo: null as RetrievalInfo | null,
    /** Conversations + projects this KB is attached to ("Used in"). */
    usage: null as KnowledgeBaseUsage | null,
    /** Direct "test retrieval" search box state. */
    searching: false,
    searchResults: null as KnowledgeBaseSearchResponse | null,
  },
  actions: (set, get) => ({
    load: async (id: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
          // New KB — drop any stale progress/selection from the previous one.
          draft.uploadingFiles.clear()
          draft.selectedFileIds.clear()
        })
        const kb = await ApiClient.KnowledgeBase.get({ id })
        set({ kb, loading: false })
        await Stores.KnowledgeBaseDetail.loadDocuments(id)
        void Stores.KnowledgeBaseDetail.loadRetrievalInfo()
        void Stores.KnowledgeBaseDetail.loadUsage(id)
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load',
          loading: false,
        })
      }
    },

    /** Load a specific 1-based page of documents (server-side; REPLACES the
     *  current page — numbered pagination, mirroring the memories/users
     *  settings pages via `ListPagination`). Total = `kb.document_count`. */
    loadDocumentsPage: async (id: string, page: number, pageSize: number) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        set({ documentsLoading: true })
        const offset = Math.max(0, (page - 1) * pageSize)
        const docs = await ApiClient.KnowledgeBase.listDocuments({
          id,
          limit: pageSize,
          offset,
        })
        set({
          documents: docs ?? [],
          documentsLoading: false,
          documentsPage: page,
          documentsPageSize: pageSize,
        })
      } catch {
        set({ documentsLoading: false })
      }
    },

    /** Load page 1 at the current page size (initial load / after upload). */
    loadDocuments: async (id: string) => {
      await Stores.KnowledgeBaseDetail.loadDocumentsPage(
        id,
        1,
        get().documentsPageSize,
      )
    },

    /** Reload the CURRENT page (so live sync status updates reach its rows)
     *  without changing the page; steps back one page if the current page
     *  emptied (e.g. the last item on the last page was removed). */
    refreshLoadedDocuments: async (id: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      const { documentsPage: page, documentsPageSize: size } = get()
      try {
        const offset = Math.max(0, (page - 1) * size)
        const docs = await ApiClient.KnowledgeBase.listDocuments({
          id,
          limit: size,
          offset,
        })
        if ((docs?.length ?? 0) === 0 && page > 1) {
          await Stores.KnowledgeBaseDetail.loadDocumentsPage(id, page - 1, size)
          return
        }
        set({ documents: docs ?? [] })
      } catch {
        /* transient */
      }
    },

    /** Refresh the KB header (document_count + indexing_summary). */
    refreshKb: async (id: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        const kb = await ApiClient.KnowledgeBase.get({ id })
        set({ kb })
      } catch {
        /* transient */
      }
    },

    /** Deployment retrieval mode (hybrid+rerank / hybrid / keyword-only). */
    loadRetrievalInfo: async () => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        set({ retrievalInfo: await ApiClient.KnowledgeBase.retrievalInfo() })
      } catch {
        /* transient */
      }
    },

    /** Where this KB is attached ("Used in" card). */
    loadUsage: async (id: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        set({ usage: await ApiClient.KnowledgeBase.usage({ id }) })
      } catch {
        /* transient */
      }
    },

    /** Direct KB search — the detail-page "test retrieval" box. */
    searchKb: async (id: string, query: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      const q = query.trim()
      if (!q) {
        set({ searchResults: null })
        return
      }
      set({ searching: true })
      try {
        const res = await ApiClient.KnowledgeBase.search({ id, query: q })
        set({ searchResults: res, searching: false })
      } catch (error) {
        set({
          searching: false,
          error: error instanceof Error ? error.message : 'Search failed',
        })
      }
    },

    clearSearch: () => set({ searchResults: null }),

    /**
     * Upload files to the store (with per-file progress), then attach the
     * successfully-uploaded ones to the KB (no combined endpoint). Mirrors
     * ProjectFiles.uploadAndAttachFiles — each file streams its own progress
     * into `uploadingFiles` so the panel can render a FileCard progress row.
     * Returns how many were newly attached vs skipped as dups.
     */
    uploadAndAttach: async (id: string, files: File[]) => {
      set({ uploading: true })
      const fileIds: string[] = []
      try {
        await Promise.all(
          files.map(async file => {
            const uploadId = `up_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
            set(draft => {
              draft.uploadingFiles.set(uploadId, {
                id: uploadId,
                filename: file.name,
                size: file.size,
                progress: 0,
                status: 'pending',
              })
            })
            try {
              if (file.size > MAX_FILE_SIZE) {
                throw new Error(`${file.name} exceeds the per-file size cap`)
              }
              set(draft => {
                const entry = draft.uploadingFiles.get(uploadId)
                if (entry) entry.status = 'uploading'
              })
              const formData = new FormData()
              formData.append('file', file)
              const uploaded = await ApiClient.File.upload(
                formData as unknown as FormData,
                {
                  fileUploadProgress: {
                    onProgress: progress => {
                      set(draft => {
                        const entry = draft.uploadingFiles.get(uploadId)
                        if (entry) entry.progress = progress
                      })
                    },
                  },
                },
              )
              fileIds.push(uploaded.id)
              set(draft => {
                draft.uploadingFiles.delete(uploadId)
              })
            } catch (error) {
              set(draft => {
                const entry = draft.uploadingFiles.get(uploadId)
                if (entry) {
                  entry.status = 'error'
                  entry.error =
                    error instanceof Error ? error.message : 'Upload failed'
                }
              })
            }
          }),
        )

        if (fileIds.length === 0) {
          return { attached: 0, skipped_duplicates: 0 }
        }
        const result = await ApiClient.KnowledgeBase.attachDocuments({
          id,
          file_ids: fileIds,
        })
        await Stores.KnowledgeBaseDetail.loadDocuments(id)
        await Stores.KnowledgeBaseDetail.refreshKb(id)
        return result
      } finally {
        set({ uploading: false })
      }
    },

    dismissUploadingFile: (uploadId: string) => {
      set(draft => {
        draft.uploadingFiles.delete(uploadId)
      })
    },

    toggleSelection: (fileId: string) => {
      set(draft => {
        if (draft.selectedFileIds.has(fileId)) {
          draft.selectedFileIds.delete(fileId)
        } else {
          draft.selectedFileIds.add(fileId)
        }
      })
    },

    deselectAll: () => {
      set(draft => {
        draft.selectedFileIds.clear()
      })
    },

    removeDocument: async (id: string, fileId: string) => {
      await ApiClient.KnowledgeBase.removeDocument({ id, file_id: fileId })
      set(draft => {
        draft.selectedFileIds.delete(fileId)
      })
      await Stores.KnowledgeBaseDetail.refreshKb(id)
      // Reload the current page so pagination + the row set stay correct.
      await Stores.KnowledgeBaseDetail.refreshLoadedDocuments(id)
    },

    /** Remove every selected document from the KB (join rows only). */
    batchRemove: async (id: string) => {
      const ids = Array.from(get().selectedFileIds)
      if (ids.length === 0) return
      for (const fileId of ids) {
        try {
          await ApiClient.KnowledgeBase.removeDocument({ id, file_id: fileId })
        } catch {
          /* per-item failure surfaced by the caller's toast; keep going */
        }
      }
      set(draft => {
        draft.selectedFileIds.clear()
      })
      await Stores.KnowledgeBaseDetail.refreshKb(id)
      // Reload the current page so pagination + the row set stay correct.
      await Stores.KnowledgeBaseDetail.refreshLoadedDocuments(id)
    },

    reindexDocument: async (id: string, fileId: string) => {
      await ApiClient.KnowledgeBase.reindexDocument({ id, file_id: fileId })
    },

    /** Re-index every retryable (failed / no_text) document currently loaded. */
    retryAllFailed: async (id: string) => {
      const failed = get().documents.filter(
        d => d.index_status === 'failed' || d.index_status === 'no_text',
      )
      for (const d of failed) {
        try {
          await ApiClient.KnowledgeBase.reindexDocument({ id, file_id: d.file_id })
        } catch {
          /* per-item; keep going */
        }
      }
    },

    reset: () =>
      set(draft => {
        draft.kb = null
        draft.documents = []
        draft.error = null
        draft.documentsPage = 1
        draft.usage = null
        draft.searchResults = null
        draft.uploadingFiles.clear()
        draft.selectedFileIds.clear()
      }),
  }),
  init: ({ on, get, actions }) => {
    // Live per-document index status + external file deletes.
    const refreshOpen = () => {
      const id = get().kb?.id
      if (!id) return
      // Refresh the loaded window (not a page-1 reset) so live status updates
      // reach every loaded row without collapsing the user's paging.
      void actions.refreshLoadedDocuments(id)
      void actions.refreshKb(id)
    }
    // Usage (conversations/projects a KB is attached to) changes on attach/detach,
    // NOT while documents index — refresh it only on the KB entity + reconnect,
    // not on the per-document index-state stream (which fires per doc at scale).
    on('sync:knowledge_base', () => {
      const id = get().kb?.id
      if (id) void actions.loadUsage(id)
    })
    on('sync:file_index_state', refreshOpen)
    on('sync:knowledge_base_document', refreshOpen)
    on('sync:file', refreshOpen)
    on('sync:reconnect', refreshOpen)
  },
})

export const useKnowledgeBaseDetailStore = KnowledgeBaseDetail.store
