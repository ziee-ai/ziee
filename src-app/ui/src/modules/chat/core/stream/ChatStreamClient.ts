import { getAuthToken, getBaseUrl } from '@/api-client/core'
import type { ChatStreamFrame, SSEChatStreamEvent } from '@/api-client/types'
import { useEventBusStore } from '@/core/events/store'
import './types'

// Live chat-token SSE client. A thin bridge (mirrors `core/sync/SyncClient`):
// opens the per-user `GET /api/chat/stream` and re-emits each generation frame
// onto the client EventBus as `chat:token` (`{conversation_id, event}`). The
// Chat store routes it. The device tells the server WHICH conversation's tokens
// it wants via `setActiveConversation` (a `PUT` echoing the handshake
// connection id), so the stream is server-scoped to the open conversation.
//
// This lives under the chat module (NOT `core/`) because it serves only chat;
// `core/sync` stays in core because it is cross-cutting across every entity.

const INITIAL_BACKOFF_MS = 1_000
const MAX_BACKOFF_MS = 30_000
const STABLE_AFTER_MS = 3_000

let started = false
let epoch = 0
let activeAbort: AbortController | null = null
let backoffMs = INITIAL_BACKOFF_MS

// The server-assigned id from the latest `connected` handshake (echoed on the
// subscription PUT), and the conversation this device currently wants. Both
// survive reconnects: a new connection re-PUTs the desired subscription.
let connectionId: string | null = null
let desiredConversationId: string | null = null

/** Start the chat-token stream (idempotent). Call when a user is authenticated. */
export function startChatStream(): void {
  if (started) return
  started = true
  backoffMs = INITIAL_BACKOFF_MS
  const myEpoch = ++epoch
  void connectLoop(myEpoch)
}

/** Stop the stream. Call on logout / user-switch. */
export function stopChatStream(): void {
  started = false
  epoch++
  activeAbort?.abort()
  activeAbort = null
  connectionId = null
  desiredConversationId = null
}

/**
 * Scope this device's token stream to one conversation (or `null` to receive
 * nothing). Persists the desire so it is re-applied on reconnect. The server
 * replays the conversation's reply-so-far if it is mid-generation.
 */
export function setActiveConversation(
  conversationId: string | null,
): Promise<void> {
  if (desiredConversationId === conversationId) return Promise.resolve()
  desiredConversationId = conversationId
  return putSubscription()
}

async function putSubscription(): Promise<void> {
  if (!connectionId) return // re-sent once the next `connected` handshake lands
  const token = getAuthToken()
  if (!token) return
  const baseUrl = await getBaseUrl()
  try {
    await fetch(`${baseUrl}/api/chat/stream/subscription`, {
      method: 'PUT',
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'X-Chat-Stream-Connection-Id': connectionId,
      },
      body: JSON.stringify({ conversation_id: desiredConversationId }),
    })
  } catch (error) {
    console.warn('[chat-stream] subscription update failed', error)
  }
}

async function connectLoop(myEpoch: number): Promise<void> {
  while (started && myEpoch === epoch) {
    try {
      await connectOnce(myEpoch)
    } catch (error) {
      if (!started || myEpoch !== epoch) break
      if (!(error instanceof DOMException && error.name === 'AbortError')) {
        console.warn('[chat-stream] stream ended; reconnecting', error)
      }
    }
    if (!started || myEpoch !== epoch) break
    await delay(backoffMs)
    backoffMs = Math.min(backoffMs * 2, MAX_BACKOFF_MS)
  }
}

async function connectOnce(myEpoch: number): Promise<void> {
  const token = getAuthToken()
  if (!token) return

  const baseUrl = await getBaseUrl()
  if (!started || myEpoch !== epoch) return

  const abort = new AbortController()
  activeAbort = abort

  const response = await fetch(`${baseUrl}/api/chat/stream`, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: 'text/event-stream',
    },
    signal: abort.signal,
  })

  if (!started || myEpoch !== epoch) {
    abort.abort()
    return
  }
  if (!response.ok || !response.body) {
    throw new Error(`[chat-stream] subscribe failed: ${response.status}`)
  }

  const stabilityTimer = globalThis.setTimeout(() => {
    backoffMs = INITIAL_BACKOFF_MS
  }, STABLE_AFTER_MS)

  const reader = response.body.getReader()
  const decoder = new globalThis.TextDecoder()
  let buffer = ''
  let currentEvent = ''

  try {
    while (started && myEpoch === epoch) {
      const { done, value } = await reader.read()
      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split(/\r\n|\n/)
      buffer = lines.pop() || ''

      for (const line of lines) {
        if (line.trim() === '') {
          currentEvent = ''
          continue
        }
        if (line.startsWith('event: ')) {
          currentEvent = line.slice(7).trim()
        } else if (line.startsWith('data: ')) {
          const raw = line.slice(6)
          let parsed: unknown = raw
          try {
            parsed = JSON.parse(raw)
          } catch {
            // keep as string
          }
          handleFrame(currentEvent, parsed)
        }
      }
    }
  } finally {
    globalThis.clearTimeout(stabilityTimer)
  }
}

function handleFrame(event: string, data: unknown): void {
  if (event === 'connected') {
    const id = (data as { connectionId?: string } | null)?.connectionId
    if (typeof id === 'string') {
      connectionId = id
      // Re-apply the desired subscription under the new connection id, and let
      // the open conversation reconcile (it may have advanced while we were
      // disconnected). Both fire only on a genuine (re)connect handshake.
      void putSubscription()
      void useEventBusStore
        .getState()
        .emit({ type: 'chat:stream-reconnect', data: {} })
    }
    return
  }

  if (!data || typeof data !== 'object') return

  // Two shapes cross this stream:
  //  1. enveloped generation frames `{conversationId, event}` (started /
  //     content / complete / error) — carry their own conversation id;
  //  2. raw extension events `{type, …}` (titleUpdated, mcpToolStart, …) — no
  //     envelope; they belong to whatever conversation THIS connection is
  //     subscribed to (the server only delivered them because we're subscribed).
  const frame = data as Partial<ChatStreamFrame> & { type?: string }
  if (frame.conversationId && frame.event) {
    void useEventBusStore.getState().emit({
      type: 'chat:token',
      data: { conversation_id: frame.conversationId, event: frame.event },
    })
  } else if (
    desiredConversationId &&
    (typeof frame.type === 'string' || event)
  ) {
    // Raw extension event. It is routed downstream by `data.type`; if a
    // hand-built event omitted `type` (some MCP notifications do), fall back to
    // the SSE `event:` name so it still dispatches instead of silently dropping.
    const payload =
      typeof frame.type === 'string'
        ? (data as SSEChatStreamEvent)
        : ({
            ...(data as object),
            type: event,
          } as unknown as SSEChatStreamEvent)
    void useEventBusStore.getState().emit({
      type: 'chat:token',
      data: { conversation_id: desiredConversationId, event: payload },
    })
  }
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => globalThis.setTimeout(resolve, ms))
}
