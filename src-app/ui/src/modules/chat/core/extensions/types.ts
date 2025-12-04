import type {
  Conversation,
  MessageWithContent,
  MessageContent,
  MessageContentData,
  SSEChatStreamEvent,
} from '@/api-client/types'

/**
 * Base Chat state type for SSE event handlers
 * This is a subset of the full ChatState to avoid circular imports
 */
export interface ChatStateForSSE {
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>
  streamingMessage: MessageWithContent | null
  isStreaming: boolean
}

/**
 * SSE Event Type Registry
 * This is an alias for the auto-generated SSEChatStreamEvent type from the API client
 * All SSE event types are defined in the OpenAPI spec and auto-generated
 */
export type SSEEventTypeRegistry = SSEChatStreamEvent

/**
 * Available slots for UI injection
 * Extensions can register components to render in these slots
 *
 * Type is auto-extracted from CHAT_SLOTS object keys
 */
export const CHAT_SLOTS = {
  /** Above message list */
  message_list_header: {
    description: 'Rendered above the message list',
    component: 'MessageList',
  },
  /** Below message list */
  message_list_footer: {
    description: 'Rendered below the message list',
    component: 'MessageList',
  },
  /** Before each message */
  message_item_prefix: {
    description: 'Rendered before each message',
    component: 'ChatMessage',
  },
  /** After each message */
  message_item_suffix: {
    description: 'Rendered after each message',
    component: 'ChatMessage',
  },
  /** Before input textarea */
  input_area_prefix: {
    description: 'Rendered before input textarea',
    component: 'ChatInput',
  },
  /** Main text input textarea */
  text_input: {
    description: 'Main text input textarea',
    component: 'ChatInput',
  },
  /** After input textarea */
  input_area_suffix: {
    description: 'Rendered after input textarea',
    component: 'ChatInput',
  },
  /** Additional toolbar buttons */
  toolbar_actions: {
    description: 'Additional toolbar buttons',
    component: 'ChatInput',
  },
  /** Message-level actions (edit, copy, etc.) */
  message_actions: {
    description: 'Message-level actions',
    component: 'ChatMessage',
  },
} as const

/**
 * Chat slot name type
 * Auto-extracted from CHAT_SLOTS object keys
 */
export type ChatSlotName = keyof typeof CHAT_SLOTS

/**
 * SSE event data structure
 * Type-safe based on SSEEventTypeRegistry (auto-generated from OpenAPI spec)
 */
export type SSEEvent<
  K extends keyof SSEEventTypeRegistry = keyof SSEEventTypeRegistry,
> = {
  [P in K]: {
    event_type: P
    data: SSEEventTypeRegistry[P]
  }
}[K]

/**
 * Generic SSE event for unknown event types
 * Used in default/fallback handlers
 */
export interface GenericSSEEvent {
  event_type: string
  data: unknown
}

/**
 * Helper type to get event data by event type
 */
export type SSEEventData<K extends keyof SSEEventTypeRegistry> =
  SSEEventTypeRegistry[K]

/**
 * Extension slot component props
 * Props passed to slot renderer components
 */
export interface SlotRendererProps {
  slot: ChatSlotName
}

/**
 * Slot registration with order
 * Allows extensions to specify render order independent of extension priority
 */
export interface SlotRegistration {
  /** React component to render */
  component: React.ComponentType
  /** Render order (lower = renders first, default: 100) */
  order?: number
}

/**
 * Request composition fields
 * Extensions can add custom fields to chat requests
 */
export interface ExtensionRequestFields {
  [key: string]: unknown
}

/**
 * Content renderer props
 * For extensions that want to render custom content types
 */
export interface ContentRendererProps {
  content: MessageContent
  isUser: boolean
}

/**
 * Extension lifecycle return types
 */
export interface BeforeSendResult {
  /** Set to true to cancel the send operation */
  cancel?: boolean
  /** Error/warning message to display to user if operation is cancelled */
  errorMessage?: string
  /** Extension names whose cancellations should be discarded */
  discardCancel?: string[]
}

export interface AfterStreamCompleteResult {
  /** Custom actions to perform */
  actions?: (() => void | Promise<void>)[]
}

export interface OnMessageSentResult {
  /** Custom actions to perform */
  actions?: (() => void | Promise<void>)[]
}

export interface OnStreamStartResult {
  /** Custom actions to perform */
  actions?: (() => void | Promise<void>)[]
}

export interface OnStreamErrorResult {
  /** Custom actions to perform */
  actions?: (() => void | Promise<void>)[]
}

export interface HandleSSEEventResult {
  /** Set to true if event was handled and should not propagate */
  handled?: boolean
  /** Custom UI updates */
  uiUpdates?: (() => void)[]
}

/**
 * Type-safe SSE event handler registry
 * Maps event types to handlers with correctly typed data parameters
 * Handlers receive (data, get, set) where get and set are from Chat store
 *
 * @example
 * ```typescript
 * sseEventHandlers: {
 *   titleUpdated: (data, get, set) => {
 *     // data is automatically typed as SSEChatStreamTitleUpdatedData
 *     // get() returns current Chat store state
 *     // set() updates Chat store state (returns new state, no mutation)
 *     const conversation = get().conversation
 *     set(state => ({ conversation: { ...state.conversation, title: data.title } }))
 *   },
 *   mcpToolStart: (data, get, set) => {
 *     // data is automatically typed as SSEChatStreamMcpToolStartData
 *     console.log(data.tool_name)
 *   }
 * }
 * ```
 */
export type SSEEventHandlers = {
  [K in keyof SSEEventTypeRegistry]?: (
    data: SSEEventTypeRegistry[K],
    get: () => ChatStateForSSE,
    set: (partial: Partial<ChatStateForSSE> | ((state: ChatStateForSSE) => Partial<ChatStateForSSE>)) => void,
  ) => void | Promise<void>
}

/**
 * Extension slice creator function type
 * Creates a slice that integrates with the Chat store
 *
 * Note: Extension state is automatically cached by the Chat store using whole-store snapshots.
 * Extensions only need to define their state (T) and actions (A).
 */
export type ExtensionSliceCreator<T, A = Record<string, any>> = (
  set: any,
  get: any,
) => T & A

/**
 * Extract content type string literal from MessageContentData union
 * Extracts the 'type' discriminator from each variant
 */
type ExtractContentType<T> = T extends { type: infer U } ? U : never

/**
 * All possible content type strings
 * Result: 'text' | 'thinking' | 'image' | 'file_attachment' | 'tool_use' | 'tool_result'
 */
type ContentTypeString = ExtractContentType<MessageContentData>

/**
 * Helper type to narrow MessageContent to a specific content data type
 * Replaces the generic MessageContentData with a specific variant
 */
export type MessageContentTyped<TData extends MessageContentData> = Omit<MessageContent, 'content'> & {
  content: TData
}

/**
 * Type-safe streaming delta processor for a specific content type
 * The content parameter is automatically typed based on the content type string
 * This ensures compile-time safety when accessing content properties
 */
type StreamingDeltaProcessor<T extends ContentTypeString> = (
  content: MessageContentTyped<Extract<MessageContentData, { type: T }>>,
  delta: string,
) => MessageContent | Promise<MessageContent>

/**
 * Type-safe streaming delta processor registry
 * Maps content types to handlers with correctly typed content parameters
 * More efficient than processStreamingDelta (O(1) lookup instead of O(n) loop)
 *
 * @example
 * ```typescript
 * streamingDeltaProcessors: {
 *   text: (content, delta) => {
 *     // content.content is automatically typed as MessageContentDataText
 *     // No casting needed!
 *     return {
 *       ...content,
 *       content: { ...content.content, text: content.content.text + delta }
 *     }
 *   },
 *   thinking: (content, delta) => {
 *     // content.content is automatically typed as MessageContentDataThinking
 *     // No casting needed!
 *     return {
 *       ...content,
 *       content: { ...content.content, thinking: content.content.thinking + delta }
 *     }
 *   }
 * } satisfies StreamingDeltaProcessors
 * ```
 */
export type StreamingDeltaProcessors = {
  [K in ContentTypeString]?: StreamingDeltaProcessor<K>
}

/**
 * Type-safe streaming content provider
 * Creates initial content blocks for a specific content type during streaming
 * Automatically types the return value based on content type
 *
 * @template T - The content type string (e.g., 'text', 'thinking', 'tool_use')
 * @param delta - Optional initial delta text from the stream
 * @returns New MessageContent for this content block, or null if not handled
 */
type StreamingContentProvider<T extends ContentTypeString> = (
  delta?: string,
) => MessageContentTyped<Extract<MessageContentData, { type: T }>> | null | Promise<MessageContentTyped<Extract<MessageContentData, { type: T }>> | null>

/**
 * Type-safe streaming content provider registry
 * Maps content types to factory functions with correctly typed return values
 * More efficient than provideStreamingContent (O(1) lookup instead of O(n) loop)
 *
 * @example
 * ```typescript
 * streamingContentProviders: {
 *   text: (delta) => {
 *     // Return value is automatically typed as MessageContent with text content
 *     return {
 *       id: crypto.randomUUID(),
 *       message_id: '',
 *       content_type: 'text',
 *       content: { type: 'text', text: delta || '' },
 *       sequence_order: 0,
 *       created_at: new Date().toISOString(),
 *       updated_at: new Date().toISOString(),
 *     }
 *   },
 *   thinking: (delta) => {
 *     // Return value is automatically typed as MessageContent with thinking content
 *     return {
 *       id: crypto.randomUUID(),
 *       message_id: '',
 *       content_type: 'thinking',
 *       content: { type: 'thinking', thinking: delta || '', metadata: null },
 *       sequence_order: 0,
 *       created_at: new Date().toISOString(),
 *       updated_at: new Date().toISOString(),
 *     }
 *   }
 * } satisfies StreamingContentProviders
 * ```
 */
export type StreamingContentProviders = {
  [K in ContentTypeString]?: StreamingContentProvider<K>
}

/**
 * Main extension interface
 * All chat extensions must implement this interface
 */
export interface ChatExtension {
  /** Unique extension identifier */
  readonly name: string

  /** Extension description for debugging */
  readonly description?: string

  /** Extension execution priority (lower = earlier, default: 100) */
  readonly priority?: number

  /**
   * Store configuration for stateful extensions
   * Groups store name and factory function together
   * If defined, both name and createStore must be provided
   *
   * The store will be accessible via Stores.Chat.{store.name}
   * Provides full reactivity, lifecycle management, and reference counting
   *
   * @example
   * ```typescript
   * store: {
   *   name: 'McpStore',  // Store key for registration
   *   createStore: () => createExtensionStore<MyStore>((set, get) => ({
   *     // State
   *     selectedId: null,
   *
   *     // Actions
   *     selectId: (id) => set(state => { state.selectedId = id })
   *   }))
   * }
   * ```
   */
  readonly store?: {
    /** Store key for registration (e.g., "McpStore", "FileStore") */
    name: string
    /** Factory function to create the store instance */
    createStore: () => import('@/core/stores').StoreProxy<any>
  }

  /**
   * Initialize extension
   * Called once when extension is registered
   * Extensions should access Stores.Chat for conversation data
   */
  initialize?: () => void | Promise<void>

  /**
   * Called when a conversation is loaded or switched
   * Extensions can use this to sync state with conversation data
   *
   * @param conversation - The loaded conversation
   */
  onConversationLoad?: (conversation: import('@/api-client/types').Conversation) => void | Promise<void>

  /**
   * Called before a message is sent
   * Can modify message, add request fields, or cancel send
   * Extensions should access their own stores for data (e.g., TextStore for text)
   */
  beforeSendMessage?: () => BeforeSendResult | Promise<BeforeSendResult>

  /**
   * Called after message is successfully sent (before streaming starts)
   * Useful for clearing state, logging, etc.
   * Extensions should access Stores.Chat for conversation data
   */
  onMessageSent?: () => OnMessageSentResult | Promise<OnMessageSentResult>

  /**
   * Called when streaming starts
   * Extensions should access Stores.Chat for conversation data
   */
  onStreamStart?: () => OnStreamStartResult | Promise<OnStreamStartResult>

  /**
   * Called when streaming encounters an error
   * Extensions should access Stores.Chat for conversation data
   */
  onStreamError?: (error: Error) => OnStreamErrorResult | Promise<OnStreamErrorResult>

  /**
   * Called after stream completes successfully
   * Can perform cleanup, analytics, etc.
   * Extensions should access Stores.Chat for conversation data
   */
  afterStreamComplete?: (
    message: MessageWithContent,
  ) => AfterStreamCompleteResult | Promise<AfterStreamCompleteResult>

  /**
   * Handle SSE events (DEPRECATED - use sseEventHandlers instead)
   * Return handled: true to prevent other extensions from processing
   * Extensions should access Stores.Chat for conversation data
   * @deprecated Use sseEventHandlers for type-safe event handling
   */
  handleSSEEvent?: (
    event: SSEEvent,
  ) => HandleSSEEventResult | Promise<HandleSSEEventResult>

  /**
   * Type-safe SSE event handler registry
   * Register handlers for specific event types with auto-typed data parameters
   * More efficient than handleSSEEvent (O(1) lookup instead of O(n) loop)
   *
   * @example
   * ```typescript
   * sseEventHandlers: {
   *   titleUpdated: (data) => {
   *     // data is automatically typed as SSEChatStreamTitleUpdatedData
   *     return { handled: true, uiUpdates: [...] }
   *   }
   * }
   * ```
   */
  sseEventHandlers?: SSEEventHandlers

  /**
   * Content type components registry
   * Map of content types to React components
   * Extensions register components for specific content types they can render
   * More efficient than single ContentRenderer with runtime filtering
   * Can access reactive stores directly via Stores.Chat
   *
   * @example
   * ```typescript
   * contentTypes: {
   *   'text': MyTextRenderer,
   *   'file_attachment': MyFileRenderer,
   *   'mcp_tool_call': MyToolCallRenderer,
   * }
   * ```
   */
  contentTypes?: Record<string, React.ComponentType<ContentRendererProps>>

  /**
   * Slot components registry
   * Map of slot names to React components with optional order
   * Extensions register components for specific slots they want to render in
   * Order property controls render position (lower = renders first, default: 100)
   * Can access reactive stores directly via Stores.Chat
   *
   * @example
   * ```typescript
   * slots: {
   *   'toolbar_actions': { component: MyToolbarComponent, order: 10 },
   *   'message_list_header': { component: MyHeaderComponent, order: 20 },
   * }
   * ```
   */
  slots?: Partial<Record<ChatSlotName, SlotRegistration>>

  /**
   * Compose request fields
   * Add custom fields to chat requests
   * Extensions should access Stores.Chat for conversation data
   */
  composeRequestFields?: () =>
    | ExtensionRequestFields
    | Promise<ExtensionRequestFields>

  /**
   * Provide user message content
   * Called when creating a user message to allow extensions to contribute content
   *
   * @param text - The primary text content from user input
   * @param composedRequest - The composed request with all extension fields
   * @returns Array of MessageContentData to add to user message (or empty array)
   *
   * @example
   * ```typescript
   * // Text extension provides text content
   * provideUserContent: async (text, composedRequest) => {
   *   if (!text) return []
   *   return [{ type: 'text', text }]
   * }
   *
   * // File extension provides file attachments
   * provideUserContent: async (text, composedRequest) => {
   *   const fileIds = composedRequest.file_ids || []
   *   return fileIds.map(id => ({ type: 'file_attachment', file_id: id, ... }))
   * }
   * ```
   */
  provideUserContent?: (
    text: string,
    composedRequest: any,
  ) => MessageContent[] | Promise<MessageContent[]>

  /**
   * Streaming content provider registry (PREFERRED over provideStreamingContent)
   * Maps content types to factory functions that create initial content blocks
   * Provides O(1) lookup and compile-time type safety
   *
   * This is the recommended approach for providing streaming content blocks.
   * Use this instead of provideStreamingContent for better performance and type safety.
   *
   * @example
   * ```typescript
   * streamingContentProviders: {
   *   text: (delta) => ({
   *     id: crypto.randomUUID(),
   *     message_id: '',
   *     content_type: 'text',
   *     content: { type: 'text', text: delta || '' },
   *     sequence_order: 0,
   *     created_at: new Date().toISOString(),
   *     updated_at: new Date().toISOString(),
   *   }),
   *   thinking: (delta) => ({
   *     id: crypto.randomUUID(),
   *     message_id: '',
   *     content_type: 'thinking',
   *     content: { type: 'thinking', thinking: delta || '', metadata: null },
   *     sequence_order: 0,
   *     created_at: new Date().toISOString(),
   *     updated_at: new Date().toISOString(),
   *   })
   * } satisfies StreamingContentProviders
   * ```
   */
  streamingContentProviders?: StreamingContentProviders

  /**
   * Provide streaming content (DEPRECATED - use streamingContentProviders instead)
   * Called when a new streaming content block starts (delta with new index)
   *
   * @deprecated Use streamingContentProviders for type-safe, performant content creation
   * @param contentType - The content type from streaming delta
   * @param delta - Optional initial delta text
   * @returns New MessageContentData for this content block, or null if not handled
   *
   * @example
   * ```typescript
   * // DEPRECATED - prefer streamingContentProviders
   * provideStreamingContent: async (contentType, delta) => {
   *   if (contentType === 'text') {
   *     return { type: 'text', text: delta || '' }
   *   }
   *   return null
   * }
   * ```
   */
  provideStreamingContent?: (
    contentType: string,
    delta?: string,
  ) => MessageContent | null | Promise<MessageContent | null>

  /**
   * Streaming delta processor registry (PREFERRED over processStreamingDelta)
   * Maps content types to type-safe delta processors
   * Provides O(1) lookup and compile-time type safety
   *
   * This is the recommended approach for handling streaming deltas.
   * Use this instead of processStreamingDelta for better performance and type safety.
   *
   * @example
   * ```typescript
   * streamingDeltaProcessors: {
   *   text: (content, delta) => ({
   *     ...content,
   *     content: { ...content.content, text: content.content.text + delta }
   *   }),
   *   thinking: (content, delta) => ({
   *     ...content,
   *     content: { ...content.content, thinking: content.content.thinking + delta }
   *   })
   * }
   * ```
   */
  streamingDeltaProcessors?: StreamingDeltaProcessors

  /**
   * Process streaming delta (DEPRECATED - use streamingDeltaProcessors instead)
   * Called for each delta during streaming to accumulate content
   *
   * @deprecated Use streamingDeltaProcessors for type-safe, performant delta processing
   * @param content - The existing MessageContentData
   * @param delta - The delta text to append
   * @returns Updated MessageContentData with delta applied
   *
   * @example
   * ```typescript
   * // DEPRECATED - prefer streamingDeltaProcessors
   * processStreamingDelta: async (content, delta) => {
   *   if (content.content.type === 'text') {
   *     return {
   *       ...content,
   *       content: { ...content.content, text: content.content.text + delta }
   *     }
   *   }
   *   return content
   * }
   * ```
   */
  processStreamingDelta?: (
    content: MessageContent,
    delta: string,
  ) => MessageContent | Promise<MessageContent>

  /**
   * Cleanup extension
   * Called when extension is unregistered or chat is unmounted
   * Extensions should access Stores.Chat for conversation data
   */
  cleanup?: () => void | Promise<void>
}

/**
 * Extension registration options
 */
export interface ExtensionRegistrationOptions {
  /** Whether to enable this extension by default */
  enabled?: boolean
  /** Extension-specific configuration */
  config?: Record<string, unknown>
}
