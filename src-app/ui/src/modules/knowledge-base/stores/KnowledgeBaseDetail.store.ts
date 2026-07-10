import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import {
  type KnowledgeBase,
  type KnowledgeBaseDocument,
  Permissions,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import { Stores } from '@/core/stores'

enableMapSet()

export const KnowledgeBaseDetail = defineStore('KnowledgeBaseDetail', {
  immer: true,
  state: {
    kb: null as KnowledgeBase | null,
    documents: [] as KnowledgeBaseDocument[],
    loading: false,
    documentsLoading: false,
    uploading: false,
    error: null as string | null,
  },
  actions: set => ({
    load: async (id: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        set({ loading: true, error: null })
        const kb = await ApiClient.KnowledgeBase.get({ id })
        set({ kb, loading: false })
        await Stores.KnowledgeBaseDetail.loadDocuments(id)
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load',
          loading: false,
        })
      }
    },

    loadDocuments: async (id: string) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      try {
        set({ documentsLoading: true })
        const docs = await ApiClient.KnowledgeBase.listDocuments({ id })
        set({ documents: docs ?? [], documentsLoading: false })
      } catch {
        set({ documentsLoading: false })
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

    /** Upload files to the store, then attach them to the KB (no combined
     *  endpoint). Returns how many were newly attached vs skipped as dups. */
    uploadAndAttach: async (id: string, files: File[]) => {
      set({ uploading: true })
      try {
        const fileIds: string[] = []
        for (const file of files) {
          const formData = new FormData()
          formData.append('file', file)
          const uploaded = await ApiClient.File.upload(
            formData as unknown as FormData,
          )
          fileIds.push(uploaded.id)
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

    removeDocument: async (id: string, fileId: string) => {
      await ApiClient.KnowledgeBase.removeDocument({ id, file_id: fileId })
      set(draft => {
        draft.documents = draft.documents.filter(d => d.file_id !== fileId)
      })
      await Stores.KnowledgeBaseDetail.refreshKb(id)
    },

    reindexDocument: async (id: string, fileId: string) => {
      await ApiClient.KnowledgeBase.reindexDocument({ id, file_id: fileId })
    },

    reset: () => set({ kb: null, documents: [], error: null }),
  }),
  init: ({ on, get, actions }) => {
    // Live per-document index status + external file deletes.
    const refreshOpen = () => {
      const id = get().kb?.id
      if (!id) return
      void actions.loadDocuments(id)
      void actions.refreshKb(id)
    }
    on('sync:file_index_state', refreshOpen)
    on('sync:knowledge_base_document', refreshOpen)
    on('sync:file', refreshOpen)
    on('sync:reconnect', refreshOpen)
  },
})

export const useKnowledgeBaseDetailStore = KnowledgeBaseDetail.store
