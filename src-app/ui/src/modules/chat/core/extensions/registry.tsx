import type {
  ChatExtension,
  ExtensionRegistrationOptions,
  BeforeSendResult,
  SendBlocker,
  SSEEvent,
  SSEEventTypeRegistry,
  ChatSlotName,
  ExtensionRequestFields,
  ContentRendererProps,
} from '@/modules/chat/core/extensions/types'
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
   * Streaming delta processor registry: Maps content types to delta processors
   * Built at registration time for efficient O(1) delta routing
   */
  private streamingDeltaProcessorRegistry: Map<
    string,
    Array<{
      extension: ChatExtension
      processor: (content: import('@/api-client/types').MessageContent, delta: string) => import('@/api-client/types').MessageContent | Promise<import('@/api-client/types').MessageContent>
      priority: number
    }>
  > = new Map()

  /**
   * Streaming content provider registry: Maps content types to content factories
   * Built at registration time for efficient O(1) content creation routing
   */
  private streamingContentProviderRegistry: Map<
    string,
    Array<{
      extension: ChatExtension
      provider: (delta?: string) => import('@/api-client/types').MessageContent | null | Promise<import('@/api-client/types').MessageContent | null>
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

    // Seed the PRIMARY pane's chat store with this extension's store instance
    // (split panes seed their OWN via `injectExtensionStores` in each store's
    // init — see that method). Idempotent so it can't race/overwrite the
    // init-time injection: whichever runs first wins, the other skips.
    if (extension.store) {
      // Lazy import useChatStore to avoid circular dependency
      // Import happens at runtime when register() is called, not at module load time
      import('../stores/Chat.store').then(({ useChatStore }) => {
        // Inject store at root level of Chat store for reactive access via Stores.Chat.{storeName}
        // Direct mutation is safe now that Immer middleware has been removed
        const stateObject = useChatStore.getState() as unknown as Record<
          string,
          unknown
        >
        if (!stateObject[extension.store!.name]) {
          stateObject[extension.store!.name] = extension.store!.createStore()
          console.log(
            `[ChatExtensions] Injected store "${extension.store!.name}" for extension: ${extension.name}`,
          )
        }
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

    // Register streaming delta processors in registry
    if (extension.streamingDeltaProcessors) {
      for (const [contentType, processor] of Object.entries(extension.streamingDeltaProcessors)) {
        if (!this.streamingDeltaProcessorRegistry.has(contentType)) {
          this.streamingDeltaProcessorRegistry.set(contentType, [])
        }

        this.streamingDeltaProcessorRegistry.get(contentType)!.push({
          extension,
          processor: processor as (content: import('@/api-client/types').MessageContent, delta: string) => import('@/api-client/types').MessageContent | Promise<import('@/api-client/types').MessageContent>,
          priority: extension.priority ?? 100,
        })
      }

      console.log(
        `[ChatExtensions] Registered streaming delta processors for ${extension.name}:`,
        Object.keys(extension.streamingDeltaProcessors).join(', '),
      )
    }

    // Register streaming content providers in registry
    if (extension.streamingContentProviders) {
      for (const [contentType, provider] of Object.entries(extension.streamingContentProviders)) {
        if (!this.streamingContentProviderRegistry.has(contentType)) {
          this.streamingContentProviderRegistry.set(contentType, [])
        }

        this.streamingContentProviderRegistry.get(contentType)!.push({
          extension,
          provider: provider as (delta?: string) => import('@/api-client/types').MessageContent | null | Promise<import('@/api-client/types').MessageContent | null>,
          priority: extension.priority ?? 100,
        })
      }

      console.log(
        `[ChatExtensions] Registered streaming content providers for ${extension.name}:`,
        Object.keys(extension.streamingContentProviders).join(', '),
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
    const extension = this.extensions.get(name)

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

    // Clear streaming delta processor registry entries for this extension
    for (const [contentType, entries] of this.streamingDeltaProcessorRegistry.entries()) {
      const filtered = entries.filter(entry => entry.extension.name !== name)
      if (filtered.length === 0) {
        this.streamingDeltaProcessorRegistry.delete(contentType)
      } else {
        this.streamingDeltaProcessorRegistry.set(contentType, filtered)
      }
    }

    // Clear streaming content provider registry entries for this extension
    for (const [contentType, entries] of this.streamingContentProviderRegistry.entries()) {
      const filtered = entries.filter(entry => entry.extension.name !== name)
      if (filtered.length === 0) {
        this.streamingContentProviderRegistry.delete(contentType)
      } else {
        this.streamingContentProviderRegistry.set(contentType, filtered)
      }
    }

    // Remove extension store from Chat store (if it was injected)
    if (extension?.store) {
      import('../stores/Chat.store').then(({ useChatStore }) => {
        // Direct mutation is safe now that Immer middleware has been removed
        const stateObject = useChatStore.getState()
        const storeName = extension.store!.name
        if ((stateObject as any)[storeName]) {
          delete (stateObject as any)[storeName]
          console.log(`[ChatExtensions] Removed store "${storeName}" for extension: ${name}`)
        }
      })
    }

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
   * Inject a FRESH instance of every registered extension store into the given
   * chat store state, if not already present (ITEM-4/5). Each Chat store INSTANCE
   * calls this in its `init`, so every split pane gets its OWN extension-store
   * instances (e.g. its own composer `TextStore`) rather than sharing one — the
   * register-time injection only seeds the PRIMARY pane's store. Idempotent:
   * a store already present (the primary's register-time seed) is left as-is, so
   * single-pane behaviour is unchanged.
   *
   * Direct mutation (not `set`) mirrors the register-time injection: the nested
   * store carries its own reactivity (read as `Stores.Chat.<Name>`), so adding
   * the field to the parent state object needs no subscriber notification.
   */
  injectExtensionStores(chatState: Record<string, unknown>): void {
    for (const extension of this.extensions.values()) {
      const store = extension.store
      if (store && !chatState[store.name]) {
        chatState[store.name] = store.createStore()
      }
    }
  }

  /**
   * Initialize all extensions
   * Call this once when chat is mounted
   * Extensions access Stores.Chat directly for conversation data
   */
  /**
   * Initialize all extensions against a chat store (ITEM-5/34). Each extension's
   * `initialize(ctx)` receives the OWNING store's api + its own per-pane store
   * instance, so its subscriptions/reads bind to that store rather than the
   * global singleton. Called with no args → single-pane backward-compat: binds to
   * the primary `useChatStore` singleton + its injected extension stores. The
   * per-pane `PaneExtensionRuntime` passes the pane's api + a per-pane store
   * resolver instead.
   */
  async initialize(
    chatStore?: import('./types').ChatExtStoreApi,
    resolveStore?: (
      name: string,
    ) => import('@/core/stores').StoreProxy<any> | null,
  ): Promise<void> {
    if (this.initialized) {
      console.warn('[ChatExtensions] Already initialized')
      return
    }
    await this.initializeExtensions(chatStore, resolveStore)
    this.initialized = true
  }

  /**
   * Run every extension's `initialize(ctx)` against a chat store — WITHOUT the
   * `this.initialized` gate. The per-pane `PaneExtensionRuntime` drives this with
   * its OWN flag + the pane's api, so a second pane's extensions actually
   * initialize (the shared-flag bug: the global gate made the 2nd pane's
   * `initialize()` early-return, silently skipping its subscriptions +
   * keyboard/file/edit wiring — GAP-1). `initialize()` above is the single-pane
   * gated wrapper; both share this body.
   */
  async initializeExtensions(
    chatStore?: import('./types').ChatExtStoreApi,
    resolveStore?: (
      name: string,
    ) => import('@/core/stores').StoreProxy<any> | null,
  ): Promise<void> {
    const extensions = this.getExtensions()
    console.log(
      `[ChatExtensions] Initializing ${extensions.length} extensions...`,
    )

    let api = chatStore
    let resolve = resolveStore
    if (!api || !resolve) {
      const { useChatStore } = await import('../stores/Chat.store')
      const state = useChatStore.getState() as unknown as Record<string, unknown>
      api =
        api ?? (useChatStore as unknown as import('./types').ChatExtStoreApi)
      resolve =
        resolve ??
        ((name) =>
          (state[name] as import('@/core/stores').StoreProxy<any>) ?? null)
    }

    for (const extension of extensions) {
      try {
        if (extension.initialize) {
          const store = extension.store ? resolve(extension.store.name) : null
          await extension.initialize({ chatStore: api, store })
          console.log(`[ChatExtensions] Initialized: ${extension.name}`)
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Failed to initialize ${extension.name}:`,
          error,
        )
      }
    }
  }

  /**
   * Call afterCreateConversation hooks for all enabled extensions.
   * Called in priority order after chat auto-creates a conversation
   * in sendMessage, BEFORE the message stream starts.
   *
   * Each extension receives the latest accumulated conversation
   * shape. If an extension returns a Conversation, it replaces the
   * current accumulator (the next extension sees the updated
   * shape). If an extension returns void/undefined, the accumulator
   * is unchanged.
   *
   * Returns the final post-hook conversation so the caller can
   * adopt it as the canonical local state before emitting the
   * `conversation.created` event.
   *
   * @param conversation - The freshly-created conversation
   * @returns The conversation after all hooks have run
   */
  async afterCreateConversation(
    conversation: import('@/api-client/types').Conversation,
  ): Promise<import('@/api-client/types').Conversation> {
    const extensions = this.getExtensions().filter(
      ext => ext.afterCreateConversation !== undefined,
    )

    if (extensions.length === 0) {
      return conversation
    }

    let current = conversation
    for (const extension of extensions) {
      try {
        if (extension.afterCreateConversation) {
          const updated = await extension.afterCreateConversation(current)
          if (updated) {
            current = updated
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.afterCreateConversation:`,
          error,
        )
      }
    }

    return current
  }

  /**
   * Resolve the URL for a conversation. First registered extension
   * to return a non-undefined string wins; if no extension returns
   * one, returns undefined (caller falls back to chat's default
   * `/chat/{id}`).
   */
  conversationHref(
    conversation: import('@/api-client/types').Conversation,
  ): string | undefined {
    const extensions = this.getExtensions().filter(
      ext => ext.conversationHref !== undefined,
    )
    for (const extension of extensions) {
      try {
        if (extension.conversationHref) {
          const href = extension.conversationHref(conversation)
          if (href !== undefined) {
            return href
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.conversationHref:`,
          error,
        )
      }
    }
    return undefined
  }

  /**
   * Resolve the back URL for a conversation page. Same first-non-
   * undefined-wins semantics as conversationHref. Returns undefined
   * when no extension provides one (caller falls back to chat's
   * default).
   */
  conversationBackHref(
    conversation: import('@/api-client/types').Conversation,
  ): string | undefined {
    const extensions = this.getExtensions().filter(
      ext => ext.conversationBackHref !== undefined,
    )
    for (const extension of extensions) {
      try {
        if (extension.conversationBackHref) {
          const href = extension.conversationBackHref(conversation)
          if (href !== undefined) {
            return href
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.conversationBackHref:`,
          error,
        )
      }
    }
    return undefined
  }

  /**
   * Stack all extensions' trailing-renderers for the given
   * conversation. Returns a fragment of their ReactNode outputs in
   * priority order. Called by ConversationCard when no `trailing`
   * prop is supplied.
   */
  renderConversationCardTrailing(
    conversation: import('@/api-client/types').Conversation,
  ): React.ReactNode {
    const extensions = this.getExtensions().filter(
      ext => ext.renderConversationCardTrailing !== undefined,
    )
    if (extensions.length === 0) return null
    return (
      <>
        {extensions.map(extension => {
          try {
            return (
              <React.Fragment key={extension.name}>
                {extension.renderConversationCardTrailing!(conversation)}
              </React.Fragment>
            )
          } catch (error) {
            console.error(
              `[ChatExtensions] Error in ${extension.name}.renderConversationCardTrailing:`,
              error,
            )
            return null
          }
        })}
      </>
    )
  }

  /**
   * Call onConversationLoad hooks for all enabled extensions
   * Called when a conversation is loaded or switched
   *
   * @param conversation - The loaded conversation
   */
  async onConversationLoad(conversation: import('@/api-client/types').Conversation): Promise<void> {
    const extensions = this.getExtensions().filter(
      ext => ext.onConversationLoad !== undefined,
    )

    if (extensions.length === 0) {
      return
    }

    console.log(
      `[ChatExtensions] Calling onConversationLoad for ${extensions.length} extensions...`,
    )

    for (const extension of extensions) {
      try {
        if (extension.onConversationLoad) {
          await extension.onConversationLoad(conversation)
          console.log(
            `[ChatExtensions] ${extension.name}.onConversationLoad completed`,
          )
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.onConversationLoad:`,
          error,
        )
      }
    }
  }

  /**
   * Cleanup all extensions
   * Call this when chat is unmounted
   * Extensions access Stores.Chat directly for conversation data
   */
  async cleanup(): Promise<void> {
    await this.cleanupExtensions()
    this.initialized = false
  }

  /**
   * Run every extension's `cleanup()` — WITHOUT flipping `this.initialized`. The
   * per-pane `PaneExtensionRuntime` drives this with its OWN flag, so ONE pane
   * unmounting no longer flips the shared global flag (which left surviving panes
   * marked uninitialized with no re-init → dead keyboard/file/edit — GAP-1).
   */
  async cleanupExtensions(
    chatStore?: import('./types').ChatExtStoreApi,
    resolveStore?: (
      name: string,
    ) => import('@/core/stores').StoreProxy<any> | null,
  ): Promise<void> {
    const extensions = this.getExtensions()
    console.log(
      `[ChatExtensions] Cleaning up ${extensions.length} extensions...`,
    )

    let api = chatStore
    let resolve = resolveStore
    if (!api || !resolve) {
      const { useChatStore } = await import('../stores/Chat.store')
      const state = useChatStore.getState() as unknown as Record<string, unknown>
      api =
        api ?? (useChatStore as unknown as import('./types').ChatExtStoreApi)
      resolve =
        resolve ??
        ((name) =>
          (state[name] as import('@/core/stores').StoreProxy<any>) ?? null)
    }

    for (const extension of extensions) {
      try {
        if (extension.cleanup) {
          const store = extension.store ? resolve(extension.store.name) : null
          await extension.cleanup({ chatStore: api, store })
          console.log(`[ChatExtensions] Cleaned up: ${extension.name}`)
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Failed to cleanup ${extension.name}:`,
          error,
        )
      }
    }
  }

  /**
   * Fan out `onMessageEditRestore` across all extensions. Called by
   * the chat store when the user clicks Edit on a previous message —
   * each extension filters the contents array for its own
   * content_type blocks and rehydrates its store accordingly (file:
   * restores file_attachment blocks into selectedFiles; future
   * extensions: same pattern with their own content types).
   *
   * Sequential rather than parallel so error context stays
   * deterministic. One extension throwing is logged and does NOT
   * block subsequent extensions (edit-restore is best-effort).
   */
  async onMessageEditRestore(
    contents: import('@/api-client/types').MessageContent[],
  ): Promise<void> {
    for (const extension of this.getExtensions()) {
      if (extension.onMessageEditRestore) {
        try {
          await extension.onMessageEditRestore(contents)
        } catch (error) {
          console.error(
            `[ChatExtensions] Error in ${extension.name}.onMessageEditRestore:`,
            error,
          )
        }
      }
    }
  }

  /**
   * Reactive send-blocker aggregator. Returns the list of blockers
   * currently reported by extensions (file: "uploading", future
   * extensions could report "awaiting-approval", etc.).
   *
   * THIS IS A REACT HOOK — call only from inside a render path
   * (`ChatInput` does). Iterates the extension list in stable
   * insertion order and calls each extension's `useSendBlocker`
   * unconditionally. The set of extensions is fixed at app boot,
   * so the hook count is stable across renders.
   *
   * Returns an empty array when no extension blocks. Callers
   * typically check `blockers.length > 0` to disable the Send button.
   */
  useSendBlockers(): SendBlocker[] {
    const out: SendBlocker[] = []
    for (const extension of this.getExtensions()) {
      if (extension.useSendBlocker) {
        try {
          const result = extension.useSendBlocker()
          if (result) out.push(result)
        } catch (error) {
          console.error(
            `[ChatExtensions] Error in ${extension.name}.useSendBlocker:`,
            error,
          )
        }
      }
    }
    return out
  }

  /**
   * Execute beforeSendMessage hook across all extensions
   * Collects all results and processes discardCancel to allow extensions to override cancellations
   * Extensions access their own stores for data (e.g., TextStore for text)
   */
  async beforeSendMessage(): Promise<BeforeSendResult> {
    const extensions = this.getExtensions().filter(ext =>
      ext.beforeSendMessage !== undefined,
    )

    // Collect all results with extension names
    const results: Map<string, BeforeSendResult> = new Map()

    for (const extension of extensions) {
      try {
        if (extension.beforeSendMessage) {
          const hookResult = await extension.beforeSendMessage()
          results.set(extension.name, hookResult)
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.beforeSendMessage:`,
          error,
        )
      }
    }

    // Collect all discarded extension names
    const discarded = new Set<string>()
    for (const [_, result] of results) {
      if (result.discardCancel) {
        result.discardCancel.forEach(name => discarded.add(name))
      }
    }

    // Check for remaining (non-discarded) cancellations
    for (const [name, result] of results) {
      if (result.cancel && !discarded.has(name)) {
        console.log(
          `[ChatExtensions] Message send cancelled by: ${name}`,
        )
        return {
          cancel: true,
          errorMessage: result.errorMessage,
        }
      }
    }

    return { cancel: false }
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
    // The get/set of the store INSTANCE processing this frame (ITEM-4/5). In
    // split view a frame belongs to a specific pane's store; passing that pane's
    // get/set here is what routes SSE-driven extension state (tool_use blocks,
    // title) to the STREAMING pane instead of the (hardcoded, former) primary.
    // Single-pane passes the primary's get/set, so behaviour is unchanged.
    chatGet: () => any,
    chatSet: (partial: any | ((state: any) => any)) => void,
  ): Promise<boolean> {
    const eventType = event.event_type as keyof SSEEventTypeRegistry
    const handlers = this.sseEventHandlerRegistry.get(eventType)

    // Try new registry-based handlers first (O(1) lookup)
    if (handlers && handlers.length > 0) {
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
  renderContent(
    props: ContentRendererProps,
  ): { node: React.ReactNode; consumed: number } | null {
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

    // Return the first renderer that claims this block. A renderer MAY declare
    // an optional static `contentMatch(content) => boolean` to claim only its
    // own blocks (e.g. a specific tool_result `name`); a renderer without one
    // is a catch-all (the historical first-wins behavior). This lets several
    // extensions co-own a content type (`tool_result`) without an internal
    // delegation chain — each claims its own, the catch-all handles the rest.
    for (const { extension, Component } of enabledRegistered) {
      const statics = Component as {
        contentMatch?: (c: ContentRendererProps['content']) => boolean
        contentSpan?: (blocks: ContentRendererProps['content'][], index: number) => number
      }
      if (statics.contentMatch && !statics.contentMatch(props.content)) {
        continue
      }
      try {
        // A grouping renderer may consume this block + following ones — but only
        // when it has the neighbor list (inline in a message). Rendered
        // standalone (no `blocks`), it always consumes exactly one, so grouping
        // never recurses when a group renders its own members.
        const consumed =
          props.blocks && props.index != null && statics.contentSpan
            ? Math.max(1, statics.contentSpan(props.blocks, props.index))
            : 1
        return { node: <Component {...props} />, consumed }
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
  async composeRequestFields(
    ctx: import('./types').ChatHookCtx,
  ): Promise<ExtensionRequestFields> {
    const extensions = this.getExtensions().filter(ext =>
      ext.composeRequestFields !== undefined,
    )

    let fields: ExtensionRequestFields = {}

    for (const extension of extensions) {
      try {
        if (extension.composeRequestFields) {
          const extensionFields = await extension.composeRequestFields(ctx)
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

  /**
   * Execute onMessageSent hook across all extensions
   * Called after message is successfully sent, before streaming starts
   * Extensions access Stores.Chat directly for conversation data
   */
  async onMessageSent(): Promise<void> {
    const extensions = this.getExtensions().filter(ext =>
      ext.onMessageSent !== undefined,
    )

    for (const extension of extensions) {
      try {
        if (extension.onMessageSent) {
          const result = await extension.onMessageSent()

          // Execute any custom actions
          if (result.actions) {
            for (const action of result.actions) {
              await action()
            }
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.onMessageSent:`,
          error,
        )
      }
    }
  }

  /**
   * Execute onStreamStart hook across all extensions
   * Called when streaming starts
   * Extensions access Stores.Chat directly for conversation data
   */
  async onStreamStart(): Promise<void> {
    const extensions = this.getExtensions().filter(ext =>
      ext.onStreamStart !== undefined,
    )

    for (const extension of extensions) {
      try {
        if (extension.onStreamStart) {
          const result = await extension.onStreamStart()

          // Execute any custom actions
          if (result.actions) {
            for (const action of result.actions) {
              await action()
            }
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.onStreamStart:`,
          error,
        )
      }
    }
  }

  /**
   * Execute onStreamError hook across all extensions
   * Called when streaming encounters an error
   * Extensions access Stores.Chat directly for conversation data
   */
  async onStreamError(error: Error): Promise<void> {
    const extensions = this.getExtensions().filter(ext =>
      ext.onStreamError !== undefined,
    )

    for (const extension of extensions) {
      try {
        if (extension.onStreamError) {
          const result = await extension.onStreamError(error)

          // Execute any custom actions
          if (result.actions) {
            for (const action of result.actions) {
              await action()
            }
          }
        }
      } catch (hookError) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.onStreamError:`,
          hookError,
        )
      }
    }
  }

  /**
   * Provide user message content
   * Orchestrates extensions to contribute content for user message creation
   * Extensions are called in priority order
   */
  async provideUserContent(
    text: string,
    composedRequest: any,
    // The SENDING pane's id (ITEM-32) so an extension reads THAT pane's composer
    // buffer (e.g. the file extension's per-pane attachments). Null single-pane.
    composerPaneId?: string | null,
  ): Promise<import('@/api-client/types').MessageContent[]> {
    const extensions = this.getExtensions().filter(ext =>
      ext.provideUserContent !== undefined,
    )

    const allContent: import('@/api-client/types').MessageContent[] = []

    for (const extension of extensions) {
      try {
        if (extension.provideUserContent) {
          const content = await extension.provideUserContent(
            text,
            composedRequest,
            composerPaneId ?? null,
          )
          if (content && content.length > 0) {
            allContent.push(...content)
            console.log(
              `[ChatExtensions] ${extension.name} provided ${content.length} content block(s)`,
            )
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.provideUserContent:`,
          error,
        )
      }
    }

    return allContent
  }

  /**
   * Provide streaming content
   * Uses registry for O(1) lookup by content type
   * Falls back to legacy provideStreamingContent hook for backward compatibility
   * Returns first non-null content block created for this type
   */
  async provideStreamingContent(
    contentType: string,
    delta?: string,
  ): Promise<import('@/api-client/types').MessageContent | null> {
    const providers = this.streamingContentProviderRegistry.get(contentType)

    // Try new registry-based providers first (O(1) lookup by content type)
    if (providers && providers.length > 0) {
      // Filter enabled extensions and sort by priority
      const enabledProviders = providers
        .filter(({ extension }) => {
          const options = this.extensionOptions.get(extension.name)
          return options?.enabled !== false
        })
        .sort((a, b) => a.priority - b.priority)

      // Execute first provider (first extension that registers for this type wins)
      for (const { extension, provider } of enabledProviders) {
        try {
          const content = await provider(delta)
          if (content) {
            console.log(
              `[ChatExtensions] Streaming content for "${contentType}" provided by: ${extension.name}`,
            )
            return content
          }
        } catch (error) {
          console.error(
            `[ChatExtensions] Error in ${extension.name}.streamingContentProviders.${contentType}:`,
            error,
          )
        }
      }
    }

    // Fall back to legacy provideStreamingContent hook for backward compatibility
    const legacyExtensions = this.getExtensions().filter(ext =>
      ext.provideStreamingContent !== undefined,
    )

    for (const extension of legacyExtensions) {
      try {
        if (extension.provideStreamingContent) {
          const content = await extension.provideStreamingContent(contentType, delta)
          if (content) {
            console.log(
              `[ChatExtensions] ${extension.name} provided streaming content for type: ${contentType} (legacy)`,
            )
            return content
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.provideStreamingContent:`,
          error,
        )
      }
    }

    return null
  }

  /**
   * Process streaming delta
   * Uses registry for O(1) lookup by content type
   * Falls back to legacy processStreamingDelta hook for backward compatibility
   * Returns updated content or original if no processor handles it
   */
  async processStreamingDelta(
    content: import('@/api-client/types').MessageContent,
    delta: string,
  ): Promise<import('@/api-client/types').MessageContent> {
    const contentType = (content.content as any).type
    const processors = this.streamingDeltaProcessorRegistry.get(contentType)

    // Try new registry-based processors first (O(1) lookup by content type)
    if (processors && processors.length > 0) {
      // Filter enabled extensions and sort by priority
      const enabledProcessors = processors
        .filter(({ extension }) => {
          const options = this.extensionOptions.get(extension.name)
          return options?.enabled !== false
        })
        .sort((a, b) => a.priority - b.priority)

      // Execute first processor (first extension that registers for this type wins)
      for (const { extension, processor } of enabledProcessors) {
        try {
          const updatedContent = await processor(content, delta)
          if (updatedContent !== content) {
            console.log(
              `[ChatExtensions] Streaming delta for "${contentType}" processed by: ${extension.name}`,
            )
            return updatedContent
          }
        } catch (error) {
          console.error(
            `[ChatExtensions] Error in ${extension.name}.streamingDeltaProcessors.${contentType}:`,
            error,
          )
        }
      }
    }

    // Fall back to legacy processStreamingDelta hook for backward compatibility
    const legacyExtensions = this.getExtensions().filter(ext =>
      ext.processStreamingDelta !== undefined,
    )

    for (const extension of legacyExtensions) {
      try {
        if (extension.processStreamingDelta) {
          const updatedContent = await extension.processStreamingDelta(content, delta)
          if (updatedContent !== content) {
            console.log(
              `[ChatExtensions] Streaming delta processed by: ${extension.name} (legacy)`,
            )
            return updatedContent
          }
        }
      } catch (error) {
        console.error(
          `[ChatExtensions] Error in ${extension.name}.processStreamingDelta (legacy):`,
          error,
        )
      }
    }

    // No processor handled it - return original content
    return content
  }
}

// Singleton instance
export const chatExtensionRegistry = new ChatExtensionRegistry()

/**
 * React hook that aggregates `useConversationMenu` contributions
 * from every enabled extension. Returns combined menu items +
 * stacked overlays, ready to drop into an antd Dropdown's
 * `menu.items` and rendered alongside the trigger.
 *
 * Each extension's `useConversationMenu` is itself a hook, so this
 * call iterates the registered extensions and calls each in
 * priority order. Standard rules-of-hooks apply: the set of
 * extensions implementing this hook must be stable for the
 * lifetime of the component using it (true under the
 * auto-discovery setup; HMR re-registrations re-mount consumers).
 */
export function useConversationMenuContributions(
  conversation: import('@/api-client/types').Conversation,
): {
  items: import('@/components/ui').DropdownItem[]
  overlays: React.ReactNode[]
  keepMenuOpen: boolean
} {
  const items: import('@/components/ui').DropdownItem[] = []
  const overlays: React.ReactNode[] = []
  let keepMenuOpen = false
  const extensions = chatExtensionRegistry
    .getExtensions()
    .filter(ext => ext.useConversationMenu !== undefined)
  for (const ext of extensions) {
    // eslint-disable-next-line react-hooks/rules-of-hooks
    const contrib = ext.useConversationMenu!(conversation)
    if (contrib.items) items.push(...contrib.items)
    if (contrib.overlays) {
      overlays.push(
        <React.Fragment key={ext.name}>{contrib.overlays}</React.Fragment>,
      )
    }
    if (contrib.keepMenuOpen) keepMenuOpen = true
  }
  return { items, overlays, keepMenuOpen }
}
