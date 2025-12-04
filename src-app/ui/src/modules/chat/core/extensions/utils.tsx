import React from 'react'
import type {
  ChatExtension,
  BeforeSendResult,
  AfterStreamCompleteResult,
  HandleSSEEventResult,
} from './types'

/**
 * Helper to create a basic extension with common defaults
 */
export function createExtension(
  config: Omit<ChatExtension, 'priority'> & { priority?: number },
): ChatExtension {
  return {
    priority: 100,
    ...config,
  }
}

/**
 * Helper to create a message send interceptor extension
 */
export function createMessageInterceptor(
  name: string,
  handler: () => BeforeSendResult | Promise<BeforeSendResult>,
  priority = 100,
): ChatExtension {
  return {
    name,
    description: `Message interceptor: ${name}`,
    priority,
    beforeSendMessage: handler,
  }
}

/**
 * Helper to create a stream completion handler extension
 */
export function createStreamCompletionHandler(
  name: string,
  handler: (
    message: import('@/api-client/types').MessageWithContent,
  ) => AfterStreamCompleteResult | Promise<AfterStreamCompleteResult>,
  priority = 100,
): ChatExtension {
  return {
    name,
    description: `Stream completion handler: ${name}`,
    priority,
    afterStreamComplete: handler,
  }
}

/**
 * Helper to create an SSE event handler extension (DEPRECATED)
 * @deprecated Use sseEventHandlers field instead for type-safe event handling
 * @example
 * ```typescript
 * // Old pattern (deprecated):
 * const ext = createSSEEventHandler('my-handler', (event) => {
 *   if (event.event_type === 'foo') { ... }
 * })
 *
 * // New pattern (recommended):
 * const ext = createExtension({
 *   name: 'my-extension',
 *   sseEventHandlers: {
 *     foo: (data) => { ... },  // Type-safe!
 *     bar: (data) => { ... }
 *   }
 * })
 * ```
 */
export function createSSEEventHandler(
  name: string,
  handler: (
    event: import('./types').SSEEvent,
  ) => HandleSSEEventResult | Promise<HandleSSEEventResult>,
  priority = 100,
): ChatExtension {
  return {
    name,
    description: `SSE event handler: ${name}`,
    priority,
    handleSSEEvent: handler,
  }
}

/**
 * Helper to create a content renderer extension
 */
export function createContentRenderer(
  name: string,
  contentTypeComponents: Record<
    string,
    React.ComponentType<import('./types').ContentRendererProps>
  >,
  priority = 100,
): ChatExtension {
  return {
    name,
    description: `Content renderer: ${name} (${Object.keys(contentTypeComponents).join(', ')})`,
    priority,
    contentTypes: contentTypeComponents,
  }
}

/**
 * Helper to create a UI slot extension
 */
export function createSlotRenderer(
  name: string,
  slotComponents: Partial<Record<import('./types').ChatSlotName, import('./types').SlotRegistration>>,
  priority = 100,
): ChatExtension {
  return {
    name,
    description: `Slot renderer: ${name} (${Object.keys(slotComponents).join(', ')})`,
    priority,
    slots: slotComponents,
  }
}

/**
 * Helper to merge multiple BeforeSendResult objects
 * Only merges cancel and errorMessage - request fields are handled by composeRequestFields
 */
export function mergeBeforeSendResults(
  results: BeforeSendResult[],
): BeforeSendResult {
  return results.reduce(
    (merged, result) => ({
      cancel: merged.cancel || result.cancel,
      errorMessage: result.errorMessage || merged.errorMessage,
    }),
    { cancel: false } as BeforeSendResult,
  )
}

/**
 * Helper to debounce extension actions
 */
export function debounce<T extends (...args: never[]) => unknown>(
  fn: T,
  delay: number,
): (...args: Parameters<T>) => void {
  let timeoutId: NodeJS.Timeout | null = null

  return (...args: Parameters<T>) => {
    if (timeoutId) {
      clearTimeout(timeoutId)
    }
    timeoutId = setTimeout(() => {
      fn(...args)
    }, delay)
  }
}

/**
 * Helper to throttle extension actions
 */
export function throttle<T extends (...args: never[]) => unknown>(
  fn: T,
  delay: number,
): (...args: Parameters<T>) => void {
  let lastCall = 0

  return (...args: Parameters<T>) => {
    const now = Date.now()
    if (now - lastCall >= delay) {
      lastCall = now
      fn(...args)
    }
  }
}

/**
 * Helper to check if a message matches a pattern
 * Useful for command-style extensions (e.g., /command args)
 */
export function matchesPattern(
  message: string,
  pattern: RegExp | string,
): RegExpMatchArray | null {
  if (typeof pattern === 'string') {
    pattern = new RegExp(`^${pattern}`)
  }
  return message.match(pattern)
}

/**
 * Helper to extract command and args from a message
 * Example: "/search hello world" => { command: "search", args: "hello world" }
 */
export function parseCommand(
  message: string,
  prefix = '/',
): { command: string; args: string } | null {
  const pattern = new RegExp(`^${prefix}(\\w+)\\s*(.*)$`)
  const match = message.match(pattern)

  if (!match) {
    return null
  }

  return {
    command: match[1],
    args: match[2].trim(),
  }
}

