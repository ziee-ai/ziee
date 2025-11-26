import type {
  MessageWithContent,
  MessageContent,
  SSEChatStreamEvent,
} from '@/api-client/types'

/**
 * SSE Event Type Registry
 * This is an alias for the auto-generated SSEChatStreamEvent type from the API client
 * All SSE event types are defined in the OpenAPI spec and auto-generated
 */
export type SSEEventTypeRegistry = SSEChatStreamEvent

/**
 * Available slot names for UI injection
 * Extensions can register components to render in these slots
 */
export type ChatSlotName =
  | 'message_list_header' // Above message list
  | 'message_list_footer' // Below message list
  | 'message_item_prefix' // Before each message
  | 'message_item_suffix' // After each message
  | 'input_area_prefix' // Before input textarea
  | 'input_area_suffix' // After input textarea
  | 'toolbar_actions' // Additional toolbar buttons
  | 'message_actions' // Message-level actions (edit, copy, etc.)

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
  /** Override message text */
  message?: string
  /** Add custom fields to request */
  requestFields?: ExtensionRequestFields
}

export interface AfterStreamCompleteResult {
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
   * Create extension store (REQUIRED for stateful extensions)
   * Creates an independent Zustand store for the extension
   * Store will be accessible via Stores.Chat.{extensionName}
   * Provides full reactivity, lifecycle management, and reference counting
   *
   * @example
   * ```typescript
   * createStore: () => createExtensionStore<MyState, MyActions>(
   *   { selectedId: null },
   *   (set, get) => ({
   *     selectId: (id) => set(state => { state.selectedId = id })
   *   })
   * )
   * ```
   */
  createStore?: () => import('@/core/stores').StoreProxy<any>

  /**
   * Initialize extension
   * Called once when extension is registered
   * Extensions should access Stores.Chat for conversation data
   */
  initialize?: () => void | Promise<void>

  /**
   * Called before a message is sent
   * Can modify message, add request fields, or cancel send
   * Extensions should access Stores.Chat for conversation data
   */
  beforeSendMessage?: (
    message: string,
  ) => BeforeSendResult | Promise<BeforeSendResult>

  /**
   * Called after stream completes
   * Can perform cleanup, analytics, etc.
   * Extensions should access Stores.Chat for conversation data
   */
  afterStreamComplete?: (
    message: MessageWithContent,
  ) => AfterStreamCompleteResult | Promise<AfterStreamCompleteResult>

  /**
   * Handle SSE events
   * Return handled: true to prevent other extensions from processing
   * Extensions should access Stores.Chat for conversation data
   */
  handleSSEEvent?: (
    event: SSEEvent,
  ) => HandleSSEEventResult | Promise<HandleSSEEventResult>

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
