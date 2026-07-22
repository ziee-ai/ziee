/**
 * Chat Extension System
 *
 * This module provides a comprehensive extension system for chat functionality.
 *
 * ## Core Concepts:
 *
 * 1. **Extensions**: Modular plugins that can hook into chat lifecycle events
 * 2. **Registry**: Central coordinator for all extensions
 * 3. **Stores**: Independent Zustand stores for extension state management
 * 4. **Lifecycle**: Methods extensions implement (initialize, cleanup, hooks)
 * 5. **Slots**: UI injection points for custom components
 * 6. **Events**: SSE event routing and handling
 *
 * ## Usage Example:
 *
 * Create an extension with state management:
 * - Define extension with createExtension()
 * - Add independent store via createStore
 * - Implement lifecycle hooks (beforeSendMessage, composeRequestFields, etc.)
 * - Access extension store via ChatStore.ExtensionName
 *
 * Extensions are auto-registered via extensions/index.ts
 */

// Type definitions
export type {
  ChatExtension,
  ChatSlotName,
  SlotRegistration,
  SSEEvent,
  SSEEventData,
  SSEEventHandlers,
  GenericSSEEvent,
  SSEEventTypeRegistry,
  ExtensionRequestFields,
  ContentRendererProps,
  BeforeSendResult,
  AfterStreamCompleteResult,
  HandleSSEEventResult,
  ExtensionRegistrationOptions,
  ExtensionSliceCreator,
  StreamingContentProviders,
  StreamingDeltaProcessors,
  MessageContentTyped,
} from './types'

// Slot definitions
export { CHAT_SLOTS } from './types'

// Extension store authoring — store-kit model (state / actions / init / persist / $).
// Returns a `() => proxy` factory for a chat-extension's `store.createStore`.
export { defineExtensionStore } from '@ziee/framework/store-kit'

// Registry
export {
  ChatExtensionRegistry,
  chatExtensionRegistry,
} from './registry'

// UI components
export { ExtensionSlot } from './slots'

// SSE event handling
export { parseSSEEvent, routeSSEEvent } from './events'

// Component-per-extension collectors (replace the former hooks-in-a-loop
// aggregators — Rules-of-Hooks-safe at any registration time).
export {
  useChatExtensionList,
  useSendBlocked,
  ConversationMenuContributions,
} from './contributions'

// Utility helpers
export {
  createExtension,
  createMessageInterceptor,
  createStreamCompletionHandler,
  createSSEEventHandler,
  createContentRenderer,
  createSlotRenderer,
  mergeBeforeSendResults,
  debounce,
  throttle,
  matchesPattern,
  parseCommand,
} from './utils'
