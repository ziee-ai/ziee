import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import type { Conversation, MessageWithContent } from '@/api-client/types'

interface ChatState {
  // Data
  conversation: Conversation | null
  messages: MessageWithContent[]

  // Loading states
  loading: boolean
  sending: boolean
  isStreaming: boolean
  error: string | null

  // Streaming message assembly
  streamingMessage: MessageWithContent | null

  // Actions
  loadConversation: (id: string) => Promise<void>
  loadMessages: (id: string) => Promise<void>
  sendMessage: (content: string, modelId: string) => Promise<void>
  clearError: () => void
  reset: () => void
}

export const useChatStore = create<ChatState>((set, get) => ({
  // Initial state
  conversation: null,
  messages: [],
  loading: false,
  sending: false,
  isStreaming: false,
  error: null,
  streamingMessage: null,

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
      const messages = await ApiClient.Message.getHistory({ id })
      set({ messages, loading: false })
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

    // Add optimistic user message
    set(state => ({
      messages: [...state.messages, tempUserMessage]
    }))

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
            content: (data) => {
              const state = get()

              // Handle content chunks
              if (data.content && Array.isArray(data.content)) {
                for (const block of data.content) {
                  if (block.type === 'text_delta') {
                    // Initialize or update streaming message
                    if (!state.streamingMessage) {
                      // Create new assistant message
                      const newMessage: MessageWithContent = {
                        id: `streaming-${Date.now()}`,
                        role: 'assistant',
                        contents: [{
                          id: `streaming-content-${Date.now()}`,
                          message_id: `streaming-${Date.now()}`,
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

                      set({
                        streamingMessage: newMessage,
                        messages: [...state.messages, newMessage]
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

                      set(state => ({
                        streamingMessage: updatedMessage,
                        messages: state.messages.map(m =>
                          m.id === state.streamingMessage?.id ? updatedMessage : m
                        )
                      }))
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

  clearError: () => set({ error: null }),

  reset: () => set({
    conversation: null,
    messages: [],
    loading: false,
    sending: false,
    isStreaming: false,
    error: null,
    streamingMessage: null,
  }),
}))
