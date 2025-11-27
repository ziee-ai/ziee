import type {
  ChatExtension,
  ExtensionRegistrationOptions,
  BeforeSendResult,
  SSEEvent,
  SSEEventTypeRegistry,
  ChatSlotName,
  ExtensionRequestFields,
  ContentRendererProps,
} from './types'
import React from 'react'

/**
 * Central registry for managing chat extensions
 * Handles registration, ordering, and orchestration
 * Extensions access Stores.Chat directly for conversation data
 */
export class ChatExtensionRegistry {
  private extensions: Map<string, ChatExtension> = new Map()
  private extensionOptions: Map<string, ExtensionRegistrationOptions> =
    new Map()
  private initialized = false

  /**
   * Slot registry: Maps slot names to components with order
   * Built at registration time for efficient rendering
   */
  private slotRegistry: Map<
    ChatSlotName,
    Array<{ extension: ChatExtension; Component: React.ComponentType; order: number }>
  > = new Map()

  /**
   * Content type registry: Maps content types to renderer components
   * Built at registration time for efficient rendering
   */
  private contentTypeRegistry: Map<
    string,
    Array<{ extension: ChatExtension; Component: React.ComponentType<ContentRendererProps> }>
  > = new Map()

  /**
   * SSE event handler registry: Maps event types to handlers
   * Built at registration time for efficient O(1) event routing
   */
  private sseEventHandlerRegistry: Map<
    keyof SSEEventTypeRegistry,
    Array<{
      extension: ChatExtension
      handler: (data: any, get: () => any, set: (partial: any | ((state: any) => any)) => void) => void | Promise<void>
      priority: number
    }>
  > = new Map()

  /**
   * Register a new extension
   * Extensions are automatically sorted by priority
   * Supports re-registration for HMR (Hot Module Replacement)
   */
  register(
    extension: ChatExtension,
    options: ExtensionRegistrationOptions = { enabled: true },
  ): void {
    if (this.extensions.has(extension.name)) {
      console.warn(
        `[ChatExtensions] Extension "${extension.name}" is already registered. Re-registering for HMR...`,
      )
      this.unregister(extension.name)
    }

    this.extensions.set(extension.name, extension)
    this.extensionOptions.set(extension.name, options)

    // Create independent extension store if provided
    if (extension.createStore) {
      const store = extension.createStore()

      // Lazy import useChatStore to avoid circular dependency
      // Import happens at runtime when register() is called, not at module load time
      import('../stores/Chat.store').then(({ useChatStore }) => {
        // Inject store at root level of Chat store for reactive access via Stores.Chat.{extensionName}
        // Direct mutation is safe now that Immer middleware has been removed
        const stateObject = useChatStore.getState()
        ;(stateObject as any)[extension.name] = store

        console.log(
          `[ChatExtensions] Injected independent store for extension: ${extension.name}`,
        )
      })
    }

    // Register content type components in content type registry
    if (extension.contentTypes) {
      for (const [contentType, Component] of Object.entries(extension.contentTypes)) {
        if (!this.contentTypeRegistry.has(contentType)) {
          this.contentTypeRegistry.set(contentType, [])
        }

        this.contentTypeRegistry.get(contentType)!.push({
          extension,
          Component,
        })
      }

      console.log(
        `[ChatExtensions] Registered content types for ${extension.name}:`,
        Object.keys(extension.contentTypes).join(', '),
      )
    }

    // Register slot components in slot registry
    if (extension.slots) {
      for (const [slotName, slotRegistration] of Object.entries(extension.slots)) {
        if (!this.slotRegistry.has(slotName as ChatSlotName)) {
          this.slotRegistry.set(slotName as ChatSlotName, [])
        }

        this.slotRegistry.get(slotName as ChatSlotName)!.push({
          extension,
          Component: slotRegistration.component,
          order: slotRegistration.order ?? 100, // Default order: 100
        })
      }

      console.log(
        `[ChatExtensions] Registered slots for ${extension.name}:`,
        Object.keys(extension.slots).join(', '),
      )
    }

    // Register SSE event handlers in registry
    if (extension.sseEventHandlers) {
      for (const [eventType, handler] of Object.entries(extension.sseEventHandlers)) {
        if (!this.sseEventHandlerRegistry.has(eventType as keyof SSEEventTypeRegistry)) {
          this.sseEventHandlerRegistry.set(eventType as keyof SSEEventTypeRegistry, [])
        }

        this.sseEventHandlerRegistry.get(eventType as keyof SSEEventTypeRegistry)!.push({
          extension,
          handler,
          priority: extension.priority ?? 100,
        })
      }

      console.log(
        `[ChatExtensions] Registered SSE event handlers for ${extension.name}:`,
        Object.keys(extension.sseEventHandlers).join(', '),
      )
    }

    console.log(
      `[ChatExtensions] Registered extension: ${extension.name} (priority: ${extension.priority ?? 100})`,
    )
  }

  /**
   * Unregister an extension by name
   * Cleans up all registry entries (slots, content types, stores)
   */
  unregister(name: string): void {
    this.extensions.delete(name)
    this.extensionOptions.delete(name)

    // Clear slot registry entries for this extension
    for (const [slotName, entries] of this.slotRegistry.entries()) {
      const filtered = entries.filter(entry => entry.extension.name !== name)
      if (filtered.length === 0) {
        this.slotRegistry.delete(slotName)
      } else {
        this.slotRegistry.set(slotName, filtered)
      }
    }

    // Clear content-type registry entries for this extension
    for (const [contentType, entries] of this.contentTypeRegistry.entries()) {
      const filtered = entries.filter(entry => entry.extension.name !== name)
      if (filtered.length === 0) {
        this.contentTypeRegistry.delete(contentType)
      } else {
        this.contentTypeRegistry.set(contentType, filtered)
      }
    }

    // Clear SSE event handler registry entries for this extension
    for (const [eventType, entries] of this.sseEventHandlerRegistry.entries()) {
      const filtered = entries.filter(entry => entry.extension.name !== name)
      if (filtered.length === 0) {
        this.sseEventHandlerRegistry.delete(eventType)
      } else {
        this.sseEventHandlerRegistry.set(eventType, filtered)
      }
    }

    // Remove extension store from Chat store (if it was injected)
    import('../stores/Chat.store').then(({ useChatStore }) => {
      // Direct mutation is safe now that Immer middleware has been removed
      const stateObject = useChatStore.getState()
      if ((stateObject as any)[name]) {
        delete (stateObject as any)[name]
        console.log(`[ChatExtensions] Removed store for extension: ${name}`)
      }
    })

    console.log(`[ChatExtensions] Unregistered extension: ${name}`)
  }

  /**
   * Get all registered extensions sorted by priority
   */
  getExtensions(): ChatExtension[] {
    return Array.from(this.extensions.values())
      .filter(ext => {
        const options = this.extensionOptions.get(ext.name)
        return options?.enabled !== false
      })
      .sort((a, b) => (a.priority ?? 100) - (b.priority ?? 100))
  }

  /**
   * Get extension by name
   */
  getExtension(name: string): ChatExtension | undefined {
    return this.extensions.get(name)
  }

  /**
   * Initialize all extensions
   * Call this once when chat is mounted
   * Extensions access Stores.Chat directly for conversation data
   */
  async initialize(): Promise<void> {
    if (this.initialized) {
      console.warn('[ChatExtensions] Already initialized')
      return
    }

    const extensions = this.getExtensions()
    console.log(
      `[ChatExtensions] Initializing ${extensions.length} extensions...`,
    )

    for (const extension of extensions) {
      try {
        if (extension.initialize) {
          await extension.initialize()
          console.log(`[ChatExtensions] Initialized: ${extension.name}`)
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Failed to initialize ${extension.name}:`,
          error,
        )
      }
    }

    this.initialized = true
  }

  /**
   * Cleanup all extensions
   * Call this when chat is unmounted
   * Extensions access Stores.Chat directly for conversation data
   */
  async cleanup(): Promise<void> {
    const extensions = this.getExtensions()
    console.log(
      `[ChatExtensions] Cleaning up ${extensions.length} extensions...`,
    )

    for (const extension of extensions) {
      try {
        if (extension.cleanup) {
          await extension.cleanup()
          console.log(`[ChatExtensions] Cleaned up: ${extension.name}`)
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Failed to cleanup ${extension.name}:`,
          error,
        )
      }
    }

    this.initialized = false
  }

  /**
   * Execute beforeSendMessage hook across all extensions
   * Stops if any extension returns cancel: true
   * Extensions access Stores.Chat directly for conversation data
   */
  async beforeSendMessage(message: string): Promise<BeforeSendResult> {
    const extensions = this.getExtensions().filter(ext =>
      ext.beforeSendMessage !== undefined,
    )

    let result: BeforeSendResult = {
      message,
      requestFields: {},
    }

    for (const extension of extensions) {
      try {
        if (extension.beforeSendMessage) {
          const hookResult = await extension.beforeSendMessage(
            result.message || message,
          )

          // Merge results
          if (hookResult.cancel) {
            console.log(
              `[ChatExtensions] Message send cancelled by: ${extension.name}`,
            )
            return { cancel: true }
          }

          if (hookResult.message !== undefined) {
            result.message = hookResult.message
          }

          if (hookResult.requestFields) {
            result.requestFields = {
              ...result.requestFields,
              ...hookResult.requestFields,
            }
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.beforeSendMessage:`,
          error,
        )
      }
    }

    return result
  }

  /**
   * Execute afterStreamComplete hook across all extensions
   * Extensions access Stores.Chat directly for conversation data
   */
  async afterStreamComplete(
    message: import('@/api-client/types').MessageWithContent,
  ): Promise<void> {
    const extensions = this.getExtensions().filter(ext =>
      ext.afterStreamComplete !== undefined,
    )

    for (const extension of extensions) {
      try {
        if (extension.afterStreamComplete) {
          const result = await extension.afterStreamComplete(message)

          // Execute any custom actions
          if (result.actions) {
            for (const action of result.actions) {
              await action()
            }
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.afterStreamComplete:`,
          error,
        )
      }
    }
  }

  /**
   * Route SSE event to extensions
   * Uses registry for O(1) lookup, falls back to legacy handleSSEEvent hook
   * Accepts both typed SSEEvent and GenericSSEEvent for unknown events
   * Extensions receive (data, get, set) where get and set are from Chat store
   */
  async handleSSEEvent(
    event: SSEEvent | import('./types').GenericSSEEvent,
  ): Promise<boolean> {
    const eventType = event.event_type as keyof SSEEventTypeRegistry
    const handlers = this.sseEventHandlerRegistry.get(eventType)

    // Try new registry-based handlers first (O(1) lookup)
    if (handlers && handlers.length > 0) {
      // Get Chat store's get and set functions
      const { useChatStore } = await import('../stores/Chat.store')
      const chatGet = useChatStore.getState
      const chatSet = useChatStore.setState

      // Filter enabled extensions and sort by priority
      const enabledHandlers = handlers
        .filter(({ extension }) => {
          const options = this.extensionOptions.get(extension.name)
          return options?.enabled !== false
        })
        .sort((a, b) => a.priority - b.priority)

      // Execute handlers in priority order
      for (const { extension, handler } of enabledHandlers) {
        try {
          await handler(event.data, chatGet, chatSet)
          console.log(
            `[ChatExtensions] SSE event "${event.event_type}" handled by: ${extension.name}`,
          )
        } catch (error) {
          console.error(
            `[ChatExtensions] Error in ${extension.name}.sseEventHandlers.${eventType}:`,
            error,
          )
        }
      }
      return true
    }

    // Fall back to legacy handleSSEEvent hook for backward compatibility
    const legacyExtensions = this.getExtensions().filter(ext =>
      ext.handleSSEEvent !== undefined,
    )

    for (const extension of legacyExtensions) {
      try {
        if (extension.handleSSEEvent) {
          const result = await extension.handleSSEEvent(event as SSEEvent)

          // Execute UI updates
          if (result.uiUpdates) {
            for (const update of result.uiUpdates) {
              update()
            }
          }

          // Stop propagation if handled
          if (result.handled) {
            console.log(
              `[ChatExtensions] SSE event "${event.event_type}" handled by: ${extension.name} (legacy)`,
            )
            return true
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.handleSSEEvent (legacy):`,
          error,
        )
      }
    }

    return false
  }

  /**
   * Get content renderer for a specific content type
   * Uses pre-built content-type registry for efficient rendering
   * Only creates components for extensions registered to this content type
   */
  renderContent(props: ContentRendererProps): React.ReactNode | null {
    const contentType = props.content.content_type
    const registered = this.contentTypeRegistry.get(contentType)

    if (!registered || registered.length === 0) {
      return null
    }

    // Filter by enabled extensions and sort by priority
    const enabledRegistered = registered
      .filter(({ extension }) => {
        const options = this.extensionOptions.get(extension.name)
        return options?.enabled !== false
      })
      .sort((a, b) => (a.extension.priority ?? 100) - (b.extension.priority ?? 100))

    // Return first successful renderer (early exit pattern)
    for (const { extension, Component } of enabledRegistered) {
      try {
        return <Component {...props} />
      } catch (error) {
        console.error(
          `[ChatExtensions] Error rendering content type '${contentType}' in ${extension.name}:`,
          error,
        )
      }
    }

    return null
  }

  /**
   * Get all slot renderers for a slot name
   * Uses pre-built slot registry for efficient rendering
   * Only creates components for extensions registered to this slot
   * Components are sorted by their order property (lower = renders first)
   */
  renderSlot(slot: ChatSlotName): React.ReactNode[] {
    const registered = this.slotRegistry.get(slot)

    if (!registered || registered.length === 0) {
      return []
    }

    // Filter by enabled extensions and sort by order (not priority)
    const enabledRegistered = registered
      .filter(({ extension }) => {
        const options = this.extensionOptions.get(extension.name)
        return options?.enabled !== false
      })
      .sort((a, b) => a.order - b.order) // Sort by order property

    const renderers: React.ReactNode[] = []

    for (const { extension, Component } of enabledRegistered) {
      try {
        renderers.push(<Component key={extension.name} />)
      } catch (error) {
        console.error(
          `[ChatExtensions] Error rendering slot '${slot}' in ${extension.name}:`,
          error,
        )
      }
    }

    return renderers
  }

  /**
   * Compose request fields from all extensions
   * Extensions access Stores.Chat directly for conversation data
   */
  async composeRequestFields(): Promise<ExtensionRequestFields> {
    const extensions = this.getExtensions().filter(ext =>
      ext.composeRequestFields !== undefined,
    )

    let fields: ExtensionRequestFields = {}

    for (const extension of extensions) {
      try {
        if (extension.composeRequestFields) {
          const extensionFields = await extension.composeRequestFields()
          fields = { ...fields, ...extensionFields }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.composeRequestFields:`,
          error,
        )
      }
    }

    return fields
  }
}

// Singleton instance
export const chatExtensionRegistry = new ChatExtensionRegistry()
