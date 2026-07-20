import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import { ApiClient } from '@/api-client'
import { chatExtensionRegistry } from '@/modules/chat/extensions'

import { loadAllPanelSnapshots, touchPanelSnapshot, savePanelSnapshotForConversation, rehydrateTabs } from '@/modules/chat/core/stores/Chat.store'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'
import type { ExtensionLifecycle } from '@/modules/chat/core/extensions/types'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  const extLifecycle = (): ExtensionLifecycle => get().extensionRuntime ?? chatExtensionRegistry
  return async (id: string) => {
      // Scope this device's token stream to the conversation being opened, so
      // it receives (only) this conversation's live assistant tokens — and a
      // catch-up replay if it is mid-generation. Deduped for a no-op repeat.
      void get().chatStreamClient?.setActiveConversation(id)

      const currentConversation = get().conversation
      const loadingId = get().loadingConversationId

      if (currentConversation && currentConversation.id === id) {
        console.log(`[Chat.store] Conversation ${id} already loaded, skipping`)
        return
      }

      if (loadingId === id) {
        console.log(
          `[Chat.store] Conversation ${id} is already loading, skipping`,
        )
        return
      }

      if (currentConversation && currentConversation.id !== id) {
        console.log(
          `[Chat.store] Switching from ${currentConversation.id} to ${id} - saving current state`,
        )
        get().saveConversationState(currentConversation.id)
        get().scheduleCacheClear(currentConversation.id)

        // Save outgoing conversation's panel tabs to localStorage, then clear panel
        const { rightPanel } = get()
        savePanelSnapshotForConversation(
          currentConversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )
        set(state => ({
          rightPanel: {
            ...state.rightPanel,
            tabs: [],
            activeId: null,
            mobileDrawerOpen: false,
          },
        }))

        await extLifecycle().cleanup()
        // Capture the OUTGOING message ids BEFORE clearing `messages` (ITEM-38):
        // reading them after the `set({ messages: new Map() })` below yielded []
        // — a no-op that leaked the outgoing conversation's collapse state.
        const outgoingMessageIds = Array.from(get().messages.keys())
        // Clear messages on switch so consumers never momentarily see the
        // OUTGOING conversation's messages under the new conversation id.
        // (Outgoing state was already saved via saveConversationState above;
        // the cache-hit/miss paths below repopulate from cache or the API.)
        // Without this, ConversationPage's first-load scroll latches against
        // the stale Map and the new conversation gets an animated
        // scroll-through that defeats inline-file lazy-loading.
        set({
          isStreaming: false,
          sending: false,
          streamingMessage: null,
          tempUserMessageId: null,
          streamingAbortController: null,
          streamingMessageId: null,
          lastTurnInterrupted: false,
          finalizingTurn: false,
          messages: new Map(),
          hasMoreBefore: false,
          hasMoreAfter: false,
          loadingOlder: false,
          loadingNewer: false,
        })
        // Drop the outgoing conversation's ephemeral per-row view state
        // (show-more collapse) so the incoming conversation starts clean. Scoped
        // to THIS store's own (now-captured) message ids so a split pane switching
        // conversation never clears another pane's entries (ITEM-21/38).
        useMessageViewStateStore
          .getState()
          .resetViewState(outgoingMessageIds)
      }

      get().cancelCacheClear(id)

      const cacheHit = await get().loadConversationState(id)
      if (cacheHit) {
        console.log(`[Chat.store] Cache hit for conversation: ${id}`)
        await extLifecycle().initialize()

        const { conversation } = get()
        if (conversation) {
          await chatExtensionRegistry.onConversationLoad(conversation)
          await get().loadBranches(id)
        }

        // Restore panel tabs from localStorage (after initialize() so registry is populated)
        const panelSnapshot = loadAllPanelSnapshots()[id]
        if (panelSnapshot) {
          const tabs = rehydrateTabs(panelSnapshot.tabs)
          if (tabs.length > 0) {
            set(state => ({
              rightPanel: {
                ...state.rightPanel,
                tabs,
                activeId: panelSnapshot.activeId,
              },
            }))
          }
          // Bump lastAccessedAt so the snapshot isn't evicted just because
          // the user keeps revisiting without modifying the panel.
          touchPanelSnapshot(id)
        }
        return
      }

      console.log(`[Chat.store] Cache miss for conversation: ${id}`)
      set({ loading: true, loadingConversationId: id, error: null, lastLoadErrorStatus: null })
      try {
        const conversation = await ApiClient.Conversation.get({ id })
        // Stale-result guard: if the user navigated away during the
        // await (loadingConversationId changed), drop this response.
        // Prevents the A→B→A race where A's slow response overwrites
        // B's freshly-loaded conversation. (audit 04 HIGH-1 mitigation)
        if (get().loadingConversationId !== id) {
          console.log(`[Chat.store] Stale response for ${id}, dropping`)
          return
        }
        set({ conversation, loading: false, loadingConversationId: null })

        await get().loadMessages(id)
        if (get().conversation?.id !== id) return
        await get().loadBranches(id)
        if (get().conversation?.id !== id) return

        await extLifecycle().initialize()
        await chatExtensionRegistry.onConversationLoad(conversation)

        // Restore panel tabs from localStorage (after initialize() so registry is populated)
        const panelSnapshot = loadAllPanelSnapshots()[id]
        if (panelSnapshot) {
          const tabs = rehydrateTabs(panelSnapshot.tabs)
          if (tabs.length > 0) {
            set(state => ({
              rightPanel: {
                ...state.rightPanel,
                tabs,
                activeId: panelSnapshot.activeId,
              },
            }))
          }
          touchPanelSnapshot(id)
        }
      } catch (error: any) {
        // Only surface error if we're still on this conversation; an
        // abort from navigation is not a user-facing error.
        if (get().loadingConversationId === id) {
          set({
            error: error.message || 'Failed to load conversation',
            loading: false,
            loadingConversationId: null,
            lastLoadErrorStatus:
              typeof error?.status === 'number' ? error.status : null,
          })
        }
      }
    }
}
