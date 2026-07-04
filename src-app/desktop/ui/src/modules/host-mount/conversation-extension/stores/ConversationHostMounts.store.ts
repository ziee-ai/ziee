// Per-conversation host-mounts store (desktop bundle).
//
// GET+PUT /api/host-mounts/conversation/{id}. Loaded on demand (popover open).

import { ApiClient } from '@/api-client'
import type { MountEntry } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import { type StoreProxy } from '@/core/stores'

interface ConversationHostMountsState {
  byConversation: Record<string, MountEntry[]>
  loading: boolean
  saving: boolean
  error: string | null
  loadMounts: (conversationId: string) => Promise<void>
  saveMounts: (conversationId: string, mounts: MountEntry[]) => Promise<void>
}

declare module '@/core/stores' {
  interface RegisteredStores {
    ConversationHostMounts: StoreProxy<ConversationHostMountsState>
  }
}

export const ConversationHostMounts = defineStore('ConversationHostMounts', {
  immer: true,
  state: {
    byConversation: {} as Record<string, MountEntry[]>,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => ({
    loadMounts: async (conversationId: string) => {
      try {
        set({ loading: true, error: null })
        const body = await ApiClient.HostMount.getConversationMounts({
          conversation_id: conversationId,
        })
        set(state => {
          state.byConversation[conversationId] = body.mounts
          state.loading = false
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load mounts',
          loading: false,
        })
      }
    },
    saveMounts: async (conversationId: string, mounts: MountEntry[]) => {
      try {
        set({ saving: true, error: null })
        const body = await ApiClient.HostMount.putConversationMounts({
          conversation_id: conversationId,
          mounts,
        })
        set(state => {
          state.byConversation[conversationId] = body.mounts
          state.saving = false
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to save mounts',
          saving: false,
        })
        throw error
      }
    },
  }),
})

export const useConversationHostMountsStore = ConversationHostMounts.store
