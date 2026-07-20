import type { StoreSet } from '@ziee/framework/store-kit'
import type { Conversation, MessageWithContent, Branch } from '@/api-client/types'
import type { ChatStreamClient } from '@/modules/chat/core/stream/ChatStreamClient'
import type { RightPanelTab, ChatStateSnapshot } from './index'

export const chatInitialState = {
    conversation: null as Conversation | null,
    messages: new Map<string, MessageWithContent>(),
    loading: false,
    loadingConversationId: null as string | null,
    sending: false,
    isStreaming: false,
    error: null as string | null,
    // HTTP status of the last failed conversation load (404 gone / 403 no-access),
    // so a split pane can move itself out of the workspace when its conversation
    // is deleted or access is revoked (ITEM-29). Null on success / transient error.
    lastLoadErrorStatus: null as number | null,
    lastTurnInterrupted: false,
    finalizingTurn: false,
    hasMoreBefore: false,
    hasMoreAfter: false,
    loadingOlder: false,
    loadingNewer: false,
    streamingMessage: null as MessageWithContent | null,
    tempUserMessageId: null as string | null,
    streamingAbortController: null as AbortController | null,
    streamingMessageId: null as string | null,
    // This instance's own chat-token stream client (ITEM-6). Created in `init`
    // so actions can scope it via `setActiveConversation`; null before init.
    chatStreamClient: null as ChatStreamClient | null,
    // This pane's extension runtime (ITEM-34). Attached by `ChatPaneProvider` on
    // mount so lifecycle/hooks bind to THIS pane's store + its own `initialized`
    // flag. Null on the single-pane primary store → falls back to the global
    // `chatExtensionRegistry` (which binds to the singleton = correct).
    extensionRuntime: null as import(
      '@/modules/chat/core/extensions/types'
    ).ExtensionLifecycle | null,
    // This pane's stable id (ITEM-32/37), attached by ChatPaneProvider. Scopes
    // the composer buffer (per-pane files) + the new-chat sentinel keys (model /
    // assistant / MCP) so two new-chat panes don't share one selection. Null on
    // the single-pane primary → the shared/global key (byte-identical).
    paneId: null as string | null,
    conversationStateCache: new Map<string, ChatStateSnapshot>(),
    cacheClearTimers: new Map<string, NodeJS.Timeout>(),
    // Branch initial state
    branches: [] as Branch[],
    branchesLoading: false,
    pendingBranchFromMessageId: null as string | null,
    pendingBranchForkLevel: null as 'user' | 'assistant' | null,
    branchForkLevels: new Map<string, 'user' | 'assistant'>(),
    branchChangedDuringStream: false,
    forkPoints: new Map<string, string[]>(),
    editingMessage: null as MessageWithContent | null,
    // Right panel initial state
    rightPanel: {
      panelWidth: 440,
      tabs: [] as RightPanelTab[],
      activeId: null as string | null,
      mobileDrawerOpen: false,
    },
}

export type ChatInitialState = typeof chatInitialState
export type ChatSet = StoreSet<ChatInitialState>
