import type { SSEEvent } from '@/modules/chat/core/extensions/types'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions/registry'

/**
 * Parse SSE message data into typed event
 */
export function parseSSEEvent(event: MessageEvent): SSEEvent | null {
  try {
    const data = JSON.parse(event.data)
    return {
      event_type: data.event_type || 'unknown',
      data: data.data || data,
    }
  } catch (error) {
    console.error('[ChatExtensions] Failed to parse SSE event:', error)
    return null
  }
}

/**
 * Route SSE event to extensions
 * Returns true if any extension handled the event
 */
export async function routeSSEEvent(event: SSEEvent): Promise<boolean> {
  // Standalone router with no pane context → route to the primary chat store
  // (its own get/set). The in-stream path threads the streaming pane's get/set
  // directly in Chat.store; this export is the context-free fallback.
  const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
  return await chatExtensionRegistry.handleSSEEvent(
    event,
    useChatStore.getState,
    useChatStore.setState,
  )
}
