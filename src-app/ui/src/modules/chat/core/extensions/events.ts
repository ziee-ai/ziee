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
  return await chatExtensionRegistry.handleSSEEvent(event)
}
