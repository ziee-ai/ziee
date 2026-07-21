import { ApiClient } from '@/api-client'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'
import type { MessageWithContent } from '@/api-client/types'

import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'
import type { ExtensionLifecycle } from '@/modules/chat/core/extensions/types'
import { EventBus } from '@ziee/framework/stores'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  const extLifecycle = (): ExtensionLifecycle => get().extensionRuntime ?? chatExtensionRegistry
  return async () => {
      let { conversation } = get()

      const beforeResult = await chatExtensionRegistry.beforeSendMessage()

      if (beforeResult.cancel) {
        console.log('[Chat.store] Message send cancelled by extension')
        throw new Error(
          beforeResult.errorMessage || 'Message send was cancelled',
        )
      }

      // Collect all request fields from extensions. Pass THIS pane's
      // conversation id so per-conversation composer selections (e.g. model)
      // resolve to the sending pane (ITEM-5); null = a new-chat pane.
      const allRequestFields = await chatExtensionRegistry.composeRequestFields({
        conversationId: get().conversation?.id ?? null,
        paneId: get().paneId,
      })

      // Inject branching fields directly (moved from branching extension)
      const pendingBranchFromMessageId = get().pendingBranchFromMessageId
      if (pendingBranchFromMessageId) {
        allRequestFields.create_branch_from_message_id =
          pendingBranchFromMessageId
        allRequestFields.fork_level = get().pendingBranchForkLevel ?? 'user'
      }

      if (!conversation) {
        // Deferred emission: extensions get to mutate the freshly
        // created conversation BEFORE subscribers see the event.
        // The `afterCreateConversation` hook can return a replacement
        // shape; chat adopts it and emits the post-hook conversation.
        conversation = await get().createConversation(
          undefined,
          allRequestFields.model_id as string | undefined,
          /* emitCreated */ false,
        )
        const afterHook =
          await chatExtensionRegistry.afterCreateConversation(conversation)
        if (afterHook !== conversation) {
          conversation = afterHook
          set({ conversation })
        }
        await EventBus.emit({
          type: 'conversation.created',
          data: { conversation },
        })
        await extLifecycle().initialize()
        await chatExtensionRegistry.onConversationLoad(conversation)
      }

      set({
        sending: true,
        isStreaming: true,
        error: null,
        lastTurnInterrupted: false,
        finalizingTurn: false,
      })

      // If the window is anchored MID-conversation (after an around=/find/
      // deep-link jump, so `hasMoreAfter` is true), the loaded slice does not
      // abut the real tail. Snap to the tail first so the new turn's optimistic
      // bubble appends at the actual end instead of after a gap of unloaded
      // messages (reconciled again on `complete`, but this fixes the optimistic
      // render order too).
      if (get().hasMoreAfter) {
        await get().loadMessages(conversation.id)
      }

      const userContents = await chatExtensionRegistry.provideUserContent(
        (allRequestFields.content as string) || '',
        allRequestFields,
        get().paneId,
      )

      const tempUserMessage: MessageWithContent = {
        id: `temp-${Date.now()}`,
        role: 'user',
        contents: userContents,
        originated_from_id: '',
        edit_count: 0,
        created_at: new Date().toISOString(),
      }

      set(state => {
        const newMessages = new Map(state.messages)
        newMessages.set(tempUserMessage.id, tempUserMessage)
        return {
          messages: newMessages,
          tempUserMessageId: tempUserMessage.id,
        }
      })

      try {
        // Subscribe this device's token stream to the (possibly just-created)
        // conversation BEFORE kicking off generation, so it receives all of its
        // own tokens. Idempotent/deduped for an already-open conversation.
        await get().chatStreamClient?.setActiveConversation(conversation.id)

        // Fire-and-forget: the assistant reply streams over the chat-token
        // stream (applied by `applyStreamFrame` via the `chat:token` router),
        // not this response.
        const { user_message_id, assistant_message_id } =
          await ApiClient.Message.send({
            id: conversation.id,
            branch_id: conversation.active_branch_id || '',
            ...allRequestFields,
          } as any)

        // Remember the assistant message so the stop button can address it.
        set({ streamingMessageId: assistant_message_id })

        // Reconcile the optimistic temp user message to its real id. The
        // `started` frame may also do this swap; both are idempotent.
        if (user_message_id && get().tempUserMessageId) {
          const tempId = get().tempUserMessageId!
          const tempMessage = get().messages.get(tempId)
          if (tempMessage) {
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.delete(tempId)
              newMessages.set(user_message_id, {
                ...tempMessage,
                id: user_message_id,
                contents: tempMessage.contents.map(c => ({
                  ...c,
                  message_id: user_message_id,
                })),
              })
              return { messages: newMessages, tempUserMessageId: null }
            })
          }
        }

        await chatExtensionRegistry.onMessageSent(get().paneId)
        await get().clearPendingBranch()
        set({ sending: false })
      } catch (error: any) {
        const isAborted = error instanceof Error && error.name === 'AbortError'

        if (!isAborted) {
          await chatExtensionRegistry.onStreamError(
            error instanceof Error
              ? error
              : new Error(error.message || 'Failed to send message'),
            get().paneId,
          )
        }

        const state = get()
        const baseUpdate = {
          error: isAborted ? null : error.message || 'Failed to send message',
          sending: false,
          isStreaming: false,
          streamingMessage: null,
          streamingAbortController: null,
          streamingMessageId: null,
          finalizingTurn: false,
          // Aborted (user cancel) or a transport error — either way the turn's
          // partial is not a genuine empty completion.
          lastTurnInterrupted: true,
        }

        if (state.tempUserMessageId) {
          set(state => {
            const newMessages = new Map(state.messages)
            newMessages.delete(state.tempUserMessageId!)
            return {
              messages: newMessages,
              tempUserMessageId: null,
              ...baseUpdate,
            }
          })
        } else {
          set(baseUpdate)
        }

        if (isAborted) {
          const conversation = get().conversation
          if (conversation) {
            await get().loadMessages(conversation.id)
          }
        }
      }
    }
}
