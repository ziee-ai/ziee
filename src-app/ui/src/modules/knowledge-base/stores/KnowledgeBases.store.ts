import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import {
  type CreateKnowledgeBaseRequest,
  type KnowledgeBase,
  Permissions,
  type UpdateKnowledgeBaseRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

enableMapSet()

export const KnowledgeBases = defineStore('KnowledgeBases', {
  immer: true,
  state: {
    items: new Map<string, KnowledgeBase>(),
    isInitialized: false,
    loading: false,
    creating: false,
    deleting: false,
    error: null as string | null,
  },
  actions: (set, get) => ({
    load: async (force = false) => {
      if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
      const state = get()
      if ((state.isInitialized && !force) || state.loading) return
      try {
        set({ loading: true, error: null })
        const list = await ApiClient.KnowledgeBase.list()
        set({
          items: new Map((list ?? []).map((kb: KnowledgeBase) => [kb.id, kb])),
          isInitialized: true,
          loading: false,
        })
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to load knowledge bases',
          loading: false,
        })
        throw error
      }
    },

    create: async (data: CreateKnowledgeBaseRequest): Promise<KnowledgeBase> => {
      try {
        set({ creating: true, error: null })
        const kb = await ApiClient.KnowledgeBase.create(data)
        set(draft => {
          draft.items.set(kb.id, kb)
          draft.creating = false
        })
        return kb
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to create',
          creating: false,
        })
        throw error
      }
    },

    update: async (
      id: string,
      data: UpdateKnowledgeBaseRequest,
    ): Promise<KnowledgeBase> => {
      const kb = await ApiClient.KnowledgeBase.update({ id, ...data })
      set(draft => {
        draft.items.set(kb.id, kb)
      })
      return kb
    },

    remove: async (id: string): Promise<void> => {
      if (get().deleting) return
      try {
        set({ deleting: true, error: null })
        await ApiClient.KnowledgeBase.delete({ id })
        set(draft => {
          draft.items.delete(id)
          draft.deleting = false
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to delete',
          deleting: false,
        })
        throw error
      }
    },
  }),
  init: ({ on, actions }) => {
    // Cross-device sync — `load` self-gates on the permission.
    const reload = () => void actions.load(true)
    on('sync:knowledge_base', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const useKnowledgeBasesStore = KnowledgeBases.store
