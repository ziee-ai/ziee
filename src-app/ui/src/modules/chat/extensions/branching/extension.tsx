import {
  createExtension,
  type ChatExtension,
} from '@/modules/chat/core/extensions'
import type { SSEEvent } from '@/modules/chat/core/extensions/types'
import type { SSEChatStreamStartedData } from '@/api-client/types'
import { createBranchingStore } from './Branching.store'
import { MessageActions } from './components/MessageActions'
import { BranchNavigator } from './components/BranchNavigator'
import type { Conversation } from '@/api-client/types'

/**
 * Branching Extension
 *
 * Enables message editing and response regeneration via conversation branches.
 *
 * - Edit (user message): pre-fills input with original text; next send
 *   creates a new branch from that message point.
 * - Regenerate (assistant message): finds the preceding user message,
 *   pre-fills it, and auto-sends to get a new response on a new branch.
 * - Branch navigator: shown in the message list footer when multiple
 *   branches exist, allowing the user to switch between them.
 */
const branchingExtension: ChatExtension = createExtension({
  name: 'branching',
  description: 'Edit messages and regenerate responses via conversation branches',
  priority: 50,

  store: {
    name: 'BranchingStore',
    createStore: createBranchingStore,
  },

  /**
   * Load branches when a conversation is opened.
   */
  onConversationLoad: async (conversation: Conversation) => {
    const { Stores } = await import('@/core/stores')
    await Stores.Chat.__state.BranchingStore.loadBranches(conversation.id)
  },

  /**
   * Inject create_branch_from_message_id into the send request if the user
   * clicked Edit or Regenerate.
   */
  composeRequestFields: async () => {
    const { Stores } = await import('@/core/stores')
    // getPendingBranchFromMessageId() is a function — the proxy returns it
    // directly without calling React hooks, so this is safe outside a component.
    const messageId =
      Stores.Chat.__state.BranchingStore.getPendingBranchFromMessageId()

    if (!messageId) return {}

    return { create_branch_from_message_id: messageId }
  },

  /**
   * After a message is sent, clear the pending branch state.
   */
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    Stores.Chat.__state.BranchingStore.setPendingBranchFromMessage(null)
    return {}
  },

  /**
   * After streaming completes:
   * 1. If a new branch was created during this stream, reload messages so
   *    the UI shows the correct message history for the new branch (not the
   *    old branch messages concatenated with the streamed response).
   * 2. Recompute fork points with the now-correct messages so the per-message
   *    < X/N > navigator renders at the right bubble.
   */
  afterStreamComplete: async () => {
    const { Stores } = await import('@/core/stores')
    const branchingStore = Stores.Chat.__state.BranchingStore

    if (branchingStore.getBranchChangedDuringStream()) {
      branchingStore.setBranchChangedDuringStream(false)

      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      const conversation = useChatStore.getState().conversation
      if (conversation) {
        // Reload messages for the new branch so the UI reflects the correct
        // history (new branch excludes messages after the fork point)
        await useChatStore.getState().loadMessages(conversation.id)
      }
    }

    // Always recompute fork points — messages are now up to date
    await branchingStore.computeForkPoints()
    return {}
  },

  /**
   * Intercept the SSE 'started' event to capture the branch_id the backend
   * assigned (which may be a newly created branch when create_branch_from_message_id
   * was used). Returns handled: false so Chat.store still updates the user
   * message ID as normal.
   */
  handleSSEEvent: async (event: SSEEvent) => {
    if (event.event_type === 'started') {
      const data = event.data as SSEChatStreamStartedData
      // Update conversation.active_branch_id in the Chat store so subsequent
      // sendMessage calls use the correct branch (handles branch created via
      // create_branch_from_message_id during edit/regenerate).
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      const currentBranchId = useChatStore.getState().conversation?.active_branch_id
      if (data.branch_id && data.branch_id !== currentBranchId) {
        useChatStore.setState(state => ({
          conversation: state.conversation
            ? { ...state.conversation, active_branch_id: data.branch_id }
            : null,
        }))

        // Flag that the branch changed so afterStreamComplete knows to reload messages
        const { Stores } = await import('@/core/stores')
        const branchingStore = Stores.Chat.__state.BranchingStore
        branchingStore.setBranchChangedDuringStream(true)

        // Capture the fork level for this branch before pendingBranchForkLevel is cleared
        branchingStore.captureBranchForkLevel(data.branch_id)

        // Reload branches for the navigator
        const conversation = useChatStore.getState().conversation
        if (conversation) {
          await branchingStore.loadBranches(conversation.id)
        }
      }
    }
    // Never consume the event — let Chat.store handle it too
    return { handled: false }
  },

  /**
   * Slot registrations
   */
  slots: {
    message_actions: { component: MessageActions, order: 10 },
    message_item_suffix: { component: BranchNavigator, order: 10 },
  },
})

export default branchingExtension
