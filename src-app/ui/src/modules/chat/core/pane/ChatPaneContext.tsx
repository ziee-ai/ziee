import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useMemo,
} from 'react'
import type { StoreApi } from 'zustand'
import {
  Chat,
  ChatPaneStore,
} from '@/modules/chat/core/stores/Chat.store'
import { registerPane, unregisterPane } from '@/modules/chat/core/stores/chatBridge'
import { PaneApiContext } from '@/modules/chat/core/pane/paneApiContext'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'

/** The store instance a pane subtree resolves via `useChatPane()`. */
type PaneStore = ReturnType<typeof ChatPaneStore.use>

export interface ChatPaneHandle {
  paneId: string
  /** The conversation this pane is pointed at (null = a new/empty pane). */
  conversationId: string | null
  /** This pane's own chat store instance (reactive reads + actions + `.$` + `.__api__`). */
  store: PaneStore
}

const ChatPaneContext = createContext<ChatPaneHandle | null>(null)

/**
 * Provides one pane's own `ChatPaneStore` instance to its subtree (ITEM-3).
 *
 * - Instantiates a per-pane store (`ChatPaneStore.use`) — its own EventBus group,
 *   its own conversation/messages/streaming/window state.
 * - Registers it in the `paneRegistry` so the `Stores.Chat` bridge forwards to it
 *   while it is the focused pane; deregisters on unmount.
 * - Loads (and reloads on change) the pane's conversation — `.use()` is
 *   ref-frozen so the conversationId prop can't drive a re-init on its own
 *   (Round-2 finding), hence the explicit effect.
 *
 * Pane-scoped components call `useChatPane()` to read/act on THEIR pane rather
 * than `Stores.Chat` (which is the focused pane). The single-pane path does not
 * mount a provider — it runs on the primary pane via the bridge — so this is
 * additive and only engaged once `SplitChatView` renders panes.
 */
export function ChatPaneProvider({
  paneId,
  conversationId,
  children,
}: {
  paneId: string
  conversationId: string | null
  children: ReactNode
}) {
  const store = ChatPaneStore.use()

  // Seed this pane's extension stores (e.g. the composer `TextStore`) BEFORE the
  // subtree renders. The store's own `init` also injects them, but `init` runs
  // post-mount — too late for a child (TextInput) that reads `pane.store.TextStore`
  // on its FIRST render. Idempotent (skips already-present stores); runs in a
  // render-phase useMemo so it completes before children render.
  useMemo(() => {
    chatExtensionRegistry.injectExtensionStores(
      store.__api__.getState() as unknown as Record<string, unknown>,
    )
    return null
  }, [store])

  useEffect(() => {
    registerPane({
      paneId,
      api: store.__api__ as StoreApi<ReturnType<typeof Chat.store.getState>>,
    })
    return () => unregisterPane(paneId)
  }, [paneId, store])

  useEffect(() => {
    // Skip when the instance already holds this conversation — e.g. a new-chat
    // pane that just CREATED it and adopted it into its SplitView slot (avoid a
    // reload that would interrupt the in-flight first stream). Otherwise load.
    if (conversationId && store.$.conversation?.id !== conversationId) {
      void store.loadConversation(conversationId)
    }
    // ref-frozen instance: re-run the imperative load when the pane's
    // conversation changes (DEC-14 in-pane switch).
  }, [conversationId, store])

  const handle = useMemo<ChatPaneHandle>(
    () => ({ paneId, conversationId, store }),
    [paneId, conversationId, store],
  )

  return (
    <ChatPaneContext.Provider value={handle}>
      {/* Expose this pane's raw StoreApi to the `Stores.Chat` bridge so every
          reactive `Stores.Chat.<field>` read inside this subtree resolves to
          THIS pane (see paneApiContext / chatBridge). */}
      <PaneApiContext.Provider
        value={store.__api__ as StoreApi<ReturnType<typeof Chat.store.getState>>}
      >
        {children}
      </PaneApiContext.Provider>
    </ChatPaneContext.Provider>
  )
}

/** Resolve the current pane's store handle. Throws outside a `ChatPaneProvider`. */
export function useChatPane(): ChatPaneHandle {
  const ctx = useContext(ChatPaneContext)
  if (!ctx) {
    throw new Error('useChatPane() must be used within a <ChatPaneProvider>')
  }
  return ctx
}

/** Non-throwing variant for components that render in BOTH single-pane (no
 *  provider → null) and split (provider) contexts. */
export function useChatPaneOrNull(): ChatPaneHandle | null {
  return useContext(ChatPaneContext)
}
