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
  /** Message-level footer — extension content rendered AFTER all content
   *  blocks but still inside the same MessageContext.Provider. No extension
   *  registers here today: tool-returned files render inline at their
   *  `tool_result` block (file extension's `tool_result` content renderer),
   *  not aggregated into a footer. Kept as a generic extension point. */
  message_footer: {
    description: 'Message-level footer rendered after content blocks',
    component: 'ChatMessage',
  },
  /** Model selector rendered in the toolbar, left of the Send button */
  toolbar_model: {
    description: 'Model selector rendered in the toolbar, left of the Send button',
    component: 'ChatInput',
  },
  /** Items shown inside the + dropdown menu (attach files, MCP, etc.) */
  toolbar_plus_items: {
    description: 'Items shown inside the + dropdown menu',
    component: 'ChatInput',
  },
  /** Active selection status row below the toolbar (MCP servers, assistant, etc.) */
  toolbar_status: {
    description: 'Active selection chips shown below the toolbar',
    component: 'ChatInput',
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

/**
 * Reactive "send is blocked" signal returned by an extension's
 * `useSendBlocker` hook. ChatInput's Send button disables when ANY
 * extension reports a blocker. `reason` is a short machine-readable
 * tag (e.g. 'uploading', 'awaiting-approval') so callers can render a
 * tooltip or status badge if desired.
 *
 * For click-time defensive cancellation (in case the disable race
 * losts), implement `beforeSendMessage` to return `{ cancel: true }`
 * with the same condition — the two are paired.
 */
export interface SendBlocker {
  reason: string
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
   * Called after chat auto-creates a conversation in sendMessage,
   * BEFORE the message stream starts. Extensions can append work
   * (record cross-cutting attribution, attach defaults, etc.) that
   * needs to land before the LLM begins.
   *
   * Return the updated conversation if your work mutated server-side
   * state; chat adopts the return value as the canonical local conv
   * so the subsequent `conversation.created` event + store state
   * carry the post-hook shape. Return void to keep the conversation
   * as-is.
   *
   * Extensions run sequentially in priority order; each receives the
   * latest accumulated conversation shape.
   *
   * @param conversation - The freshly-created conversation
   * @returns Updated conversation, or void to keep the input as-is
   */
  afterCreateConversation?: (
    conversation: import('@/api-client/types').Conversation,
  ) => Promise<import('@/api-client/types').Conversation | void> | import('@/api-client/types').Conversation | void

  /**
   * Return a URL string for the given conversation, or undefined to
   * let chat fall through to its default `/chat/{conversation.id}`.
   * First registered extension to return a non-undefined string
   * wins. Lets chat's conversation-list surfaces (ConversationCard,
   * recent widget, history page) route per-conversation links
   * without knowing about other modules.
   */
  conversationHref?: (
    conversation: import('@/api-client/types').Conversation,
  ) => string | undefined

  /**
   * Contribute menu items + overlay JSX to any conversation
   * dropdown menu (sidebar `RecentConversationsWidget` 3-dot menu,
   * future card-level menus, etc.). Implemented as a React hook so
   * the contribution can hold its own state (modal open, popconfirm
   * visible, async lookups) via useState/useEffect — flat item
   * descriptors aren't enough for extensions that need modals.
   *
   * Returns:
   *   - `items`: antd MenuProps['items'] entries appended to the
   *     menu. Each item's onClick can toggle local state defined in
   *     this hook to drive `overlays`.
   *   - `overlays`: optional JSX mounted alongside the dropdown
   *     trigger (modals, popconfirms, etc.).
   *
   * The aggregator on the chat side calls every registered hook in
   * priority order on every render of a conversation row. Standard
   * React-hook rules apply: same order on every render.
   */
  useConversationMenu?: (
    conversation: import('@/api-client/types').Conversation,
  ) => {
    items: import('antd').MenuProps['items']
    overlays?: import('react').ReactNode
    /**
     * Set true while any overlay (popconfirm, sub-popover) is
     * showing so the dropdown stays open even when the user moves
     * the mouse to interact with the overlay (which renders in a
     * body-level portal and would otherwise trigger antd's
     * outside-click close). Modals don't need this — they cover
     * the dropdown anyway.
     */
    keepMenuOpen?: boolean
  }

  /**
   * Render extension-supplied controls in the per-card bottom-right
   * action row of `ConversationCard`. All registered extensions
   * stack (in priority order); each returns its own React node (or
   * null to opt out for a given conversation).
   *
   * Consulted ONLY when the card's caller doesn't pass an explicit
   * `trailing` prop — caller-supplied trailing wins (e.g., the
   * project page's per-card "Remove from project" button overrides
   * any extension trailing on that surface).
   *
   * Rendered lazily — only after the user hovers the card — so an
   * extension that needs a network round-trip doesn't fire N
   * requests per page load.
   */
  renderConversationCardTrailing?: (
    conversation: import('@/api-client/types').Conversation,
  ) => import('react').ReactNode

  /**
   * Return the URL the ConversationPage back button should
   * navigate to for this conversation, or undefined to use chat's
   * default. First registered extension to return a non-undefined
   * string wins. Same first-non-undefined semantics as
   * conversationHref.
   */
  conversationBackHref?: (
    conversation: import('@/api-client/types').Conversation,
  ) => string | undefined

  /**
   * Reactive "is send blocked right now?" React hook. Called by
   * `ChatInput` on every render (via the registry's `useSendBlockers`
   * aggregator) to decide whether to disable the Send button.
   *
   * Implementations:
   * - MUST be safe to call unconditionally (regular React-hook rules
   *   apply — same hook count every render). The registry calls it
   *   inside an unconditional loop over the stable extension list.
   * - SHOULD read from the extension's own state via the raw zustand
   *   store hook (e.g. `useFileStore(s => s.uploadingFiles)`), NOT
   *   via `Stores.X.*` — the Stores proxy fires side-effect hooks on
   *   property access that can corrupt the outer hook count.
   * - Return `null` when send is unblocked. Return `{ reason }` when
   *   blocked (e.g. `{ reason: 'uploading' }` while file upload is
   *   in flight).
   *
   * For the click-time defensive cancel (e.g. a race where the user
   * clicks before the disable lands), pair with `beforeSendMessage`
   * that returns `{ cancel: true, errorMessage }` under the same
   * condition. ChatInput respects whichever fires first.
   */
  useSendBlocker?: () => SendBlocker | null

  /**
   * Called when the user clicks Edit on a previously-sent message and
   * the chat store needs each extension to rehydrate its own state
   * from the message's content blocks. Receives the FULL contents
   * array — extensions filter by their own `content_type` and act on
   * relevant blocks.
   *
   * Example: file extension filters for `content_type === 'file_attachment'`
   * blocks and calls `useFileStore.restoreFilesFromEdit(stubs)` so the
   * composer's file preview list re-populates. A future memory
   * extension could filter for `content_type === 'memory_reference'`
   * and restore the selected memories.
   *
   * Run AFTER the chat store has applied the edit (text content already
   * populated). May be async — the chat store awaits all extensions
   * sequentially before proceeding.
   *
   * The hook is the inversion partner of `provideUserContent` — that
   * one runs at send-time to GATHER blocks; this one runs at edit-time
   * to RESTORE state from those same blocks.
   */
  onMessageEditRestore?: (
    contents: MessageContent[],
  ) => void | Promise<void>

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
