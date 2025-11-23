import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import type { Conversation, MessageWithContent } from '@/api-client/types'

// Enable Map and Set support in Immer
enableMapSet()

interface ChatState {
  // Data
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>

  // Loading states
  loading: boolean
  sending: boolean
  isStreaming: boolean
  error: string | null

  // Streaming message assembly
  streamingMessage: MessageWithContent | null
  tempUserMessageId: string | null

  // Actions
  createConversation: (modelId: string, title?: string) => Promise<Conversation>
  loadConversation: (id: string) => Promise<void>
  loadMessages: (id: string) => Promise<void>
  sendMessage: (content: string, modelId: string) => Promise<void>
  updateConversation: (updates: { title?: string }) => Promise<void>
  clearError: () => void
  reset: () => void
}

export const useChatStore = create<ChatState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      // Initial state
      conversation: null,
      messages: new Map<string, MessageWithContent>(),
      loading: false,
      sending: false,
      isStreaming: false,
      error: null,
      streamingMessage: null,
      tempUserMessageId: null,

  // Create new conversation
  createConversation: async (modelId: string, title?: string) => {
    set({ loading: true, error: null })
    try {
      const conversation = await ApiClient.Conversation.create({
        model_id: modelId,
        title: title,
      })
      set({ conversation, loading: false })
      return conversation
    } catch (error: any) {
      set({
        error: error.message || 'Failed to create conversation',
        loading: false
      })
      throw error
    }
  },

  // Load conversation by ID
  loadConversation: async (id: string) => {
    set({ loading: true, error: null })
    try {
      const conversation = await ApiClient.Conversation.get({ id })
      set({ conversation, loading: false })
    } catch (error: any) {
      set({
        error: error.message || 'Failed to load conversation',
        loading: false
      })
    }
  },

  // Load messages for conversation
  loadMessages: async (id: string) => {
    set({ loading: true, error: null })
    try {
      const messagesArray = await ApiClient.Message.getHistory({ id })
      set({
        messages: new Map(messagesArray.map(msg => [msg.id, msg])),
        loading: false
      })
    } catch (error: any) {
      set({
        error: error.message || 'Failed to load messages',
        loading: false
      })
    }
  },

  // Send message with SSE streaming
  sendMessage: async (content: string, modelId: string) => {
    const { conversation } = get()

    if (!conversation) {
      set({ error: 'No active conversation' })
      return
    }

    set({ sending: true, isStreaming: true, error: null })

    // Create optimistic user message
    const tempUserMessage: MessageWithContent = {
      id: `temp-${Date.now()}`,
      role: 'user',
      contents: [{
        id: `temp-content-${Date.now()}`,
        message_id: `temp-${Date.now()}`,
        content_type: 'text',
        content: { text: content },
        sequence_order: 0,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      }],
      originated_from_id: '',
      edit_count: 0,
      created_at: new Date().toISOString(),
    }

    // Add optimistic user message and track its temp ID
    set(state => {
      state.messages.set(tempUserMessage.id, tempUserMessage)
      state.tempUserMessageId = tempUserMessage.id
    })

    try {
      await ApiClient.Message.sendStream(
        {
          id: conversation.id,
          content,
          model_id: modelId,
          branch_id: conversation.active_branch_id || '',
        },
        {
          SSE: {
            __init: (_data) => {
              console.log('Chat SSE initialized with abortController')
              set({ sending: false })
            },
            started: (data) => {
              // Handle started event - update optimistic user message with real IDs
              const state = get()
              if (data.user_message_id && data.user_content_id && state.tempUserMessageId) {
                const tempMessage = state.messages.get(state.tempUserMessageId)
                if (tempMessage) {
                  set(state => {
                    state.messages.delete(state.tempUserMessageId!)

                    // Update message with real IDs
                    const updatedMessage = {
                      ...tempMessage,
                      id: data.user_message_id!,
                      contents: tempMessage.contents.map(content => ({
                        ...content,
                        id: data.user_content_id!,
                        message_id: data.user_message_id!,
                      })),
                    }

                    state.messages.set(data.user_message_id!, updatedMessage)
                    state.tempUserMessageId = null
                  })
                }
              }
              console.log('Chat stream started:', {
                user_message_id: data.user_message_id,
                user_content_id: data.user_content_id,
                conversation_id: data.conversation_id,
                branch_id: data.branch_id,
              })
            },
            content: (data) => {
              const state = get()

              // Handle content chunks
              if (data.content && Array.isArray(data.content)) {
                for (const block of data.content) {
                  if (block.type === 'text_delta') {
                    // Initialize or update streaming message
                    if (!state.streamingMessage) {
                      // Create new assistant message with real ID from backend
                      const messageId = data.message_id || `streaming-${Date.now()}`
                      const newMessage: MessageWithContent = {
                        id: messageId,
                        role: 'assistant',
                        contents: [{
                          id: `${messageId}-content-0`,
                          message_id: messageId,
                          content_type: 'text',
                          content: { text: block.delta || '' },
                          sequence_order: 0,
                          created_at: new Date().toISOString(),
                          updated_at: new Date().toISOString(),
                        }],
                        originated_from_id: '',
                        edit_count: 0,
                        created_at: new Date().toISOString(),
                      }

                      set(state => {
                        state.streamingMessage = newMessage
                        state.messages.set(newMessage.id, newMessage)
                      })
                    } else {
                      // Append to existing message
                      const updatedMessage = {
                        ...state.streamingMessage,
                        contents: state.streamingMessage.contents.map(c => ({
                          ...c,
                          content: {
                            ...c.content,
                            text: (c.content.text || '') + (block.delta || '')
                          }
                        }))
                      }

                      set(state => {
                        state.streamingMessage = updatedMessage
                        if (state.streamingMessage) {
                          state.messages.set(state.streamingMessage.id, updatedMessage)
                        }
                      })
                    }
                  }
                }
              }
            },
            complete: (_data) => {
              // Streaming complete - reload messages to get final versions
              get().loadMessages(conversation.id)
              set({
                isStreaming: false,
                sending: false,
                streamingMessage: null
              })
            },
            error: (data) => {
              set({
                error: data.message || 'Stream error',
                isStreaming: false,
                sending: false,
                streamingMessage: null
              })
            },
            default: (event, data) => {
              console.log('Unknown chat SSE event:', event, data)
            }
          }
        }
      )
    } catch (error: any) {
      set({
        error: error.message || 'Failed to send message',
        sending: false,
        isStreaming: false,
        streamingMessage: null
      })
    }
  },

  // Update conversation properties (e.g., title)
  updateConversation: async (updates: { title?: string }) => {
    const { conversation } = get()
    if (!conversation) {
      set({ error: 'No active conversation' })
      return
    }

    try {
      await ApiClient.Conversation.update({
        id: conversation.id,
        ...updates,
      })

      set(state => {
        if (state.conversation) {
          state.conversation = {
            ...state.conversation,
            ...updates,
          }
        }
      })
    } catch (error: any) {
      set({
        error: error.message || 'Failed to update conversation',
      })
      throw error
    }
  },

  clearError: () => set({ error: null }),

  reset: () => set({
    conversation: null,
    messages: new Map<string, MessageWithContent>(),
    loading: false,
    sending: false,
    isStreaming: false,
    error: null,
    streamingMessage: null,
    tempUserMessageId: null,
  }),
    })),
  ),
)
