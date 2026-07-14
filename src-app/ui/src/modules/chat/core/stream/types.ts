import type { SSEChatStreamEvent } from '@/api-client/types'
import type { BaseEvent } from '@ziee/framework/events'

/**
 * Live chat-token events, re-emitted onto the client EventBus by the
 * `ChatStreamClient` from the per-user `GET /api/chat/stream`. `chat:token`
 * carries one generation frame tagged with its conversation; the Chat store
 * routes it (applying deltas only for the open conversation). `chat:stream-
 * reconnect` fires after the stream is re-established so the open conversation
 * can re-subscribe + reconcile.
 */
declare module '@ziee/framework/events' {
  interface AppEvents {
    'chat:token': BaseEvent & {
      type: 'chat:token'
      data: { conversation_id: string; event: SSEChatStreamEvent }
    }
    'chat:stream-reconnect': BaseEvent & {
      type: 'chat:stream-reconnect'
      data: Record<string, never>
    }
  }
}
