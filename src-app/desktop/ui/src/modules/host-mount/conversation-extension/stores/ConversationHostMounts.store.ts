// Per-conversation host-mounts store (desktop bundle).
//
// GET+PUT /api/host-mounts/conversation/{id}. Loaded on demand (when the
// header control's popover opens) rather than via a subscription, since the
// control is a transient header decoration.

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

import { ApiClient } from '@/api-client'
import type { MountEntry } from '@/api-client/types'
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

export const useConversationHostMountsStore = create<ConversationHostMountsState>()(
  subscribeWithSelector(
    immer((set): ConversationHostMountsState => ({
      byConversation: {},
      loading: false,
      saving: false,
      error: null,

      loadMounts: async (conversationId) => {
        try {
          set({ loading: true, error: null })
          const body = await ApiClient.HostMount.getConversationMounts({
            conversation_id: conversationId,
          })
          set((state) => {
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

      saveMounts: async (conversationId, mounts) => {
        try {
          set({ saving: true, error: null })
          const body = await ApiClient.HostMount.putConversationMounts({
            conversation_id: conversationId,
            mounts,
          })
          set((state) => {
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
    })),
  ),
)
