import { ApiClient } from '@/api-client'
import { finalizeTailWindow, resumeOrFreshPlaceholder, toOrderedMap } from '@/modules/chat/core/stores/messageWindow'
import { chatExtensionRegistry } from '@/modules/chat/extensions'
import type { MessageContent, MessageWithContent } from '@/api-client/types'
import type { SSEEvent } from '@/modules/chat/core/extensions/types'
import { MESSAGE_PAGE_SIZE } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string, event: any) => {
      const type = event?.type

      // SPLIT-VIEW ISOLATION (audit HIGH — both correctness + security angles):
      // every pane's store subscribes to the shared `chat:token` bus, so pane B
      // receives pane A's frames too. Only `started`/`content` guarded on the
      // conversation id; `complete` (unconditional streaming-state reset),
      // `error` (mismatch path cleared streaming + cache), and the raw-extension
      // tail (`titleUpdated`/`mcp*` → this store's extensions) did NOT — so a
      // sibling pane finishing / erroring / renaming corrupted THIS pane. One
      // guard up front makes a frame for a conversation this store does not have
      // open a true no-op. Single-pane: the client is subscription-scoped to the
      // open conversation, so this never trips (the per-branch re-checks below
      // still handle a switch that happens mid-await).
      if (get().conversation?.id !== conversationId) return

      // Mark the OPEN conversation as streaming on started/content. Critical for
      // a RECEIVING device (one watching a generation another device started) —
      // it never went through `sendMessage`, so without this its "generating"
      // affordance never shows AND the reconnect/`reloadOpen` `isStreaming`
      // guard wouldn't protect its live buffer from a refetch. Also capture the
      // assistant message id (from content frames) so a receiver can stop too.
      if (
        (type === 'started' || type === 'content') &&
        get().conversation?.id === conversationId
      ) {
        if (event?.message_id && !get().streamingMessageId) {
          set({ isStreaming: true, streamingMessageId: event.message_id })
        } else {
          set({ isStreaming: true })
        }
      }

      if (type === 'started') {
        // Drop a straggler that lands just after a switch: everything below
        // MUTATES the open conversation (branch id, temp-swap, extension stream
        // state), so applying an off-screen frame would corrupt the open view.
        if (get().conversation?.id !== conversationId) return

        await chatExtensionRegistry.onStreamStart()

        // Detect branch change (e.g. edit/regenerate created a new branch).
        const currentBranchId = get().conversation?.active_branch_id
        if (event.branch_id && event.branch_id !== currentBranchId) {
          set(state => ({
            conversation: state.conversation
              ? { ...state.conversation, active_branch_id: event.branch_id }
              : null,
            branchChangedDuringStream: true,
          }))
          await get().captureBranchForkLevel(event.branch_id)
          const conversation = get().conversation
          if (conversation) await get().loadBranches(conversation.id)
        }

        const sseEvent: SSEEvent = { event_type: 'started', data: event }
        const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
        if (handled) return

        const state = get()
        if (event.user_message_id && state.tempUserMessageId) {
          // This device sent the message: reconcile the optimistic temp id.
          // (Idempotent: the POST response may have already done this swap.)
          const tempMessage = state.messages.get(state.tempUserMessageId)
          if (tempMessage) {
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.delete(state.tempUserMessageId!)
              newMessages.set(event.user_message_id, {
                ...tempMessage,
                id: event.user_message_id,
                contents: tempMessage.contents.map(content => ({
                  ...content,
                  message_id: event.user_message_id,
                })),
              })
              return { messages: newMessages, tempUserMessageId: null }
            })
          }
        } else if (
          event.user_message_id &&
          conversationId === get().conversation?.id &&
          !get().messages.has(event.user_message_id)
        ) {
          // Receiving device (never had a temp): another device sent this
          // message. Merge the tail so the user bubble renders before the
          // assistant tokens fill in, without discarding loaded older pages.
          // Covers a catch-up replay too.
          await get().reconcileTail(conversationId)
        }
        return
      }

      if (type === 'content') {
        // Drop a straggler before any side-effect (extension dispatch included),
        // so an off-screen frame can't drive extension state for a conversation
        // we've already switched away from.
        if (get().conversation?.id !== conversationId) return

        const data = event
        const sseEvent: SSEEvent = { event_type: 'content', data }
        const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
        if (handled) return

        const state = get()
        if (data.content && Array.isArray(data.content)) {
          if (!state.streamingMessage && data.content.length > 0) {
            const placeholderId = data.message_id || `streaming-${Date.now()}`
            set(state => {
              const newMessages = new Map(state.messages)
              // A tool-approval RESUME continues the SAME assistant message id.
              // If a message with this id already exists (with accumulated
              // text/tool_use content), REUSE it as the streaming buffer rather
              // than overwriting it with an empty placeholder — otherwise the
              // message renders empty for a beat (ChatMessage bails to null on
              // zero blocks) and the bubble VANISHES then reappears (the
              // resume-chain flicker). A genuinely-new turn has no existing row
              // → a fresh empty placeholder, unchanged from before.
              const placeholder = resumeOrFreshPlaceholder(
                newMessages.get(placeholderId),
                {
                  id: placeholderId,
                  role: 'assistant',
                  contents: [],
                  originated_from_id: '',
                  edit_count: 0,
                  created_at: new Date().toISOString(),
                },
              )
              newMessages.set(placeholder.id, placeholder)
              return { streamingMessage: placeholder, messages: newMessages }
            })
          }

          for (const block of data.content) {
            if (block.type === 'text_delta') {
              const currentState = get()
              const hasTextContent =
                currentState.streamingMessage?.contents.some(
                  c =>
                    c.content_type === 'text' ||
                    (c.content as any)?.type === 'text',
                ) ?? false

              if (!currentState.streamingMessage || !hasTextContent) {
                const messageId =
                  currentState.streamingMessage?.id ||
                  data.message_id ||
                  `streaming-${Date.now()}`
                const initialContent =
                  await chatExtensionRegistry.provideStreamingContent(
                    'text',
                    block.delta,
                  )
                if (initialContent) {
                  const baseMessage = currentState.streamingMessage ?? {
                    id: messageId,
                    role: 'assistant' as const,
                    contents: [],
                    originated_from_id: '',
                    edit_count: 0,
                    created_at: new Date().toISOString(),
                  }
                  const newContent = {
                    ...initialContent,
                    id: `${messageId}-content-${baseMessage.contents.length}`,
                    message_id: messageId,
                    sequence_order: baseMessage.contents.length,
                  }
                  const newMessage: MessageWithContent = {
                    ...baseMessage,
                    id: messageId,
                    contents: [...baseMessage.contents, newContent],
                  }
                  set(state => {
                    const newMessages = new Map(state.messages)
                    newMessages.set(newMessage.id, newMessage)
                    return {
                      streamingMessage: newMessage,
                      messages: newMessages,
                    }
                  })
                }
              } else {
                const delta = block.delta || ''
                const incomingMessageId = data.message_id
                set(currentState => {
                  if (!currentState.streamingMessage) return {}
                  const stableId = currentState.streamingMessage.id
                  const dbId = incomingMessageId || stableId
                  const existingContents =
                    currentState.streamingMessage.contents
                  const lastBlock =
                    existingContents[existingContents.length - 1]
                  const lastIsText =
                    !!lastBlock &&
                    (lastBlock.content_type === 'text' ||
                      (lastBlock.content as any)?.type === 'text')

                  let updatedContents: MessageContent[]
                  if (lastIsText) {
                    const currentText = (lastBlock.content as any)?.text || ''
                    updatedContents = [...existingContents]
                    updatedContents[existingContents.length - 1] = {
                      ...lastBlock,
                      content: {
                        ...lastBlock.content,
                        text: currentText + delta,
                      } as any,
                    }
                  } else {
                    const now = new Date().toISOString()
                    updatedContents = [
                      ...existingContents,
                      {
                        id: `${stableId}-content-${existingContents.length}`,
                        message_id: dbId,
                        content_type: 'text',
                        content: { type: 'text', text: delta } as any,
                        sequence_order: existingContents.length,
                        created_at: now,
                        updated_at: now,
                      },
                    ]
                  }

                  const updatedMessage: MessageWithContent = {
                    ...currentState.streamingMessage,
                    contents: updatedContents.map(c => ({
                      ...c,
                      message_id: dbId,
                    })),
                  }
                  const newMessages = new Map(currentState.messages)
                  newMessages.set(stableId, updatedMessage)
                  return {
                    streamingMessage: updatedMessage,
                    messages: newMessages,
                  }
                })
              }
            }
          }
        }
        return
      }

      if (type === 'complete') {
        const sseEvent: SSEEvent = { event_type: 'complete', data: event }
        const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
        if (handled) return

        const { streamingMessage } = get()
        const isOnOriginalConversation =
          get().conversation?.id === conversationId

        // A user-cancelled turn arrives as a `complete` frame with
        // finish_reason "cancelled" (start_generation) — that's an interrupted
        // partial, not a genuine empty completion, so flag it to suppress the
        // empty-completion notice on the (possibly reasoning-only) partial.
        const cancelled = event.finish_reason === 'cancelled'

        // A BACKGROUND conversation completing: tear down its live buffer and
        // drop the streaming row (nothing on-screen to keep continuous), and
        // never touch the on-screen `lastTurnInterrupted` (a single global
        // signal). Unchanged from the original teardown.
        if (!isOnOriginalConversation) {
          set(state => {
            const newMessages = new Map(state.messages)
            if (state.streamingMessage) {
              newMessages.delete(state.streamingMessage.id)
            }
            return {
              isStreaming: false,
              sending: false,
              streamingMessage: null,
              streamingAbortController: null,
              streamingMessageId: null,
              messages: newMessages,
            }
          })
          get().clearConversationCache(conversationId)
          return
        }

        // ── On-screen finalize: keep the streamed row, swap in persisted ────
        // Clear the streaming CONTROL flags SYNCHRONOUSLY (as the original did),
        // but do NOT delete the streamed assistant row from `messages`, and set
        // `finalizingTurn` so the empty-completion notice stays suppressed until
        // the persisted tail is merged in. The row is therefore continuously
        // visible (no disappear/reappear + no false notice) WITHOUT holding
        // `isStreaming` true across the awaited `getHistory` — so a conversation
        // switch or a `saveConversationState` snapshot during that await can
        // never observe a mid-stream state (which would strand the row as
        // "generating" or clobber a background stream). Only the transient
        // `finalizingTurn` is cleared AFTER the await, in the persisted swap.
        const streamingRowId = streamingMessage?.id ?? null
        // Capture BEFORE clearing: an edit/regenerate created a NEW branch during
        // this stream, so the loaded window still holds the old branch's prefix —
        // a merge would be inconsistent. Snap to the new branch's tail instead.
        const branchChanged = get().branchChangedDuringStream
        set({
          isStreaming: false,
          sending: false,
          streamingMessage: null,
          streamingAbortController: null,
          streamingMessageId: null,
          branchChangedDuringStream: false,
          finalizingTurn: true,
          lastTurnInterrupted: cancelled,
        })

        // Per-pane (ITEM v2): the completion runs in the OWNING pane's own store,
        // so thread this pane's id to the extension hooks (the async-hook focus-race
        // fix). The switched-away/background case already returned via the early bail
        // above (`!isOnOriginalConversation`), so no extra guard is needed here.
        if (streamingMessage) {
          await chatExtensionRegistry.afterStreamComplete(streamingMessage, get().paneId)
        }
        const conversation = get().conversation

        if (conversation) {
          try {
            const page = await ApiClient.Message.getHistory({
              id: conversation.id,
              limit: MESSAGE_PAGE_SIZE,
            })
            if (get().conversation?.id === conversation.id) {
              set(state => {
                // Snap-to-tail when the branch changed or the window is anchored
                // mid-conversation (a merge would splice the tail after a gap);
                // else merge so loaded older pages stay and the finalized turn
                // replaces the streaming row IN PLACE (no empty frame). The
                // sidebar message_count self-heals via the backend `Conversation`
                // sync, so no optimistic count emit here.
                const snapToTail = branchChanged || state.hasMoreAfter
                if (snapToTail) {
                  return {
                    finalizingTurn: false,
                    messages: toOrderedMap(page.messages),
                    hasMoreBefore: page.has_more_before,
                    hasMoreAfter: page.has_more_after,
                    loadingOlder: false,
                    loadingNewer: false,
                  }
                }
                return {
                  finalizingTurn: false,
                  messages: finalizeTailWindow(
                    state.messages,
                    page.messages,
                    streamingRowId,
                  ),
                  hasMoreAfter: false,
                }
              })
            }
            // else — switched away mid-fetch: do NOT touch finalizingTurn. The
            // switch/reset that changed the conversation already cleared it
            // (loadConversation cleanup / reset), and whatever conversation is now
            // on-screen may own its OWN finalize; clearing it here would clobber
            // that newer suppression.
          } catch (error: any) {
            // getHistory failed: the streamed row is still in `messages`, so it
            // stays visible; clear the finalize flag + surface the error only if
            // we're still on-screen (else the switch already cleared it).
            if (get().conversation?.id === conversation.id) {
              set({
                finalizingTurn: false,
                error: get().error || error.message || 'Failed to refresh messages',
              })
            }
          }
        } else {
          set({ finalizingTurn: false })
        }
        await get().computeForkPoints()
        return
      }

      if (type === 'error') {
        const streamError = new Error(event.message || 'Stream error')
        await chatExtensionRegistry.onStreamError(streamError, get().paneId)
        const sseEvent: SSEEvent = { event_type: 'error', data: event }
        await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)

        if (get().conversation?.id !== conversationId) {
          set({
            isStreaming: false,
            sending: false,
            streamingMessage: null,
            streamingAbortController: null,
            streamingMessageId: null,
            finalizingTurn: false,
          })
          get().clearConversationCache(conversationId)
          return
        }

        const state = get()
        if (state.tempUserMessageId) {
          set(state => {
            const newMessages = new Map(state.messages)
            newMessages.delete(state.tempUserMessageId!)
            return {
              messages: newMessages,
              tempUserMessageId: null,
              error: event.message || 'Stream error',
              isStreaming: false,
              sending: false,
              streamingMessage: null,
              streamingAbortController: null,
              streamingMessageId: null,
              finalizingTurn: false,
              lastTurnInterrupted: true,
            }
          })
        } else {
          set({
            error: event.message || 'Stream error',
            isStreaming: false,
            sending: false,
            streamingMessage: null,
            streamingAbortController: null,
            streamingMessageId: null,
            finalizingTurn: false,
            lastTurnInterrupted: true,
          })
        }
        return
      }

      // Extension events (titleUpdated, mcpToolStart/Complete/Progress,
      // mcpApprovalRequired, mcpElicitationRequired, artifactCreated, …) —
      // route through the extension registry exactly as the old inline
      // `default` SSE handler did. The backend forwards these onto the
      // chat-token stream alongside content frames.
      const sseEvent: SSEEvent = { event_type: type, data: event }
      await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
    }
}
