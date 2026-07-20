import type { MessageWithContent } from '@/api-client/types'
import { chatExtensionRegistry } from '@/modules/chat/extensions'
import { savePanelSnapshotForConversation } from '@/modules/chat/core/stores/Chat.store'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'
import type { ExtensionLifecycle } from '@/modules/chat/core/extensions/types'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  const extLifecycle = (): ExtensionLifecycle => get().extensionRuntime ?? chatExtensionRegistry
  return async () => {
      // Leaving for a new chat: stop receiving any conversation's tokens.
      void get().chatStreamClient?.setActiveConversation(null)
      const { conversation } = get()
      if (conversation) {
        get().saveConversationState(conversation.id)
        get().scheduleCacheClear(conversation.id)

        // Save outgoing conversation's panel tabs to localStorage before clearing
        const { rightPanel } = get()
        savePanelSnapshotForConversation(
          conversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )

        await extLifecycle().cleanup()
      }

      set(state => ({
        conversation: null,
        messages: new Map<string, MessageWithContent>(),
        loading: false,
        loadingConversationId: null,
        sending: false,
        isStreaming: false,
        finalizingTurn: false,
        error: null,
        hasMoreBefore: false,
        hasMoreAfter: false,
        loadingOlder: false,
        loadingNewer: false,
        streamingMessage: null,
        tempUserMessageId: null,
        streamingMessageId: null,
        branches: [],
        branchesLoading: false,
        pendingBranchFromMessageId: null,
        pendingBranchForkLevel: null,
        branchForkLevels: new Map(),
        branchChangedDuringStream: false,
        forkPoints: new Map(),
        editingMessage: null,
        rightPanel: {
          ...state.rightPanel,
          tabs: [],
          activeId: null,
          mobileDrawerOpen: false,
        },
      }))
    }
}
