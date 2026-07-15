import { getAuthToken, getBaseUrl } from '@ziee/framework/api-client/core'
import type { ChatStreamFrame, SSEChatStreamEvent } from '@/api-client/types'
import { useEventBusStore } from '@ziee/framework/events/store'
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
//
// PER-INSTANCE (ITEM-6): `createChatStreamClient()` returns an INDEPENDENT client
// — its own connection, epoch, backoff, connection-id and subscribed
// conversation. Each split pane owns one, so two panes never fight over a single
// `desiredConversationId` (each holds a dedicated connection scoped to its own
// conversation; the backend registry supports N connections per user). The
// single-pane / primary-pane store creates exactly one, so its behaviour is
// unchanged.

const INITIAL_BACKOFF_MS = 1_000
const MAX_BACKOFF_MS = 30_000
const STABLE_AFTER_MS = 3_000

/** An independent chat-token SSE client bound to one conversation at a time. */
export interface ChatStreamClient {
  /** Start the stream (idempotent). Call when a user is authenticated. */
  start(): void
  /** Stop the stream. Call on logout / user-switch / pane teardown. */
  stop(): void
  /** Scope this client's stream to one conversation (or `null` for none). */
  setActiveConversation(conversationId: string | null): Promise<void>
}

/**
 * Handlers a pane's store wires to ITS client (ITEM-35). Frames are delivered
 * DIRECTLY to the owning pane instead of the global `chat:token` EventBus, so two
 * panes on the SAME conversation (compare-two-branches) each process only their
 * own connection's frames — the global bus made both stores apply BOTH clients'
 * frames, doubling/garbling live text. Omit them (single-pane legacy) → the
 * client falls back to the global EventBus emit, unchanged.
 */
export interface ChatStreamHandlers {
  onFrame?: (conversationId: string, event: unknown) => void
  onReconnect?: () => void
}

/** Create an independent chat-token SSE client (see the per-instance note). */
export function createChatStreamClient(
  handlers?: ChatStreamHandlers,
): ChatStreamClient {
  let started = false
  let epoch = 0
  let activeAbort: AbortController | null = null
  let backoffMs = INITIAL_BACKOFF_MS

  // The server-assigned id from the latest `connected` handshake (echoed on the
  // subscription PUT), and the conversation this client currently wants. Both
  // survive reconnects: a new connection re-PUTs the desired subscription.
  let connectionId: string | null = null
  let desiredConversationId: string | null = null

  function start(): void {
    if (started) return
    started = true
    backoffMs = INITIAL_BACKOFF_MS
    const myEpoch = ++epoch
    void connectLoop(myEpoch)
  }

  function stop(): void {
    started = false
    epoch++
    activeAbort?.abort()
    activeAbort = null
    connectionId = null
    desiredConversationId = null
  }

  function setActiveConversation(conversationId: string | null): Promise<void> {
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
      const resp = await fetch(`${baseUrl}/api/chat/stream/subscription`, {
        method: 'PUT',
        headers: {
          Authorization: `Bearer ${token}`,
          'Content-Type': 'application/json',
          'X-Chat-Stream-Connection-Id': connectionId,
        },
        body: JSON.stringify({ conversation_id: desiredConversationId }),
      })
      if (!resp.ok) {
        // A non-2xx PUT (stale connection id, 401, or 429 under the per-user
        // cap) means this connection is not subscribed. Don't swallow it: drop
        // the connection id and abort the live stream so the connect loop
        // reconnects with a fresh handshake and re-PUTs the desired
        // subscription — otherwise the pane would sit token-less silently.
        console.warn(
          `[chat-stream] subscription PUT failed: ${resp.status}; forcing reconnect`,
        )
        connectionId = null
        activeAbort?.abort()
      }
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
        // Direct per-pane reconnect (ITEM-35), else the global bus (legacy).
        if (handlers?.onReconnect) handlers.onReconnect()
        else
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
      // Direct per-pane delivery (ITEM-35), else the global bus (legacy).
      if (handlers?.onFrame) handlers.onFrame(frame.conversationId, frame.event)
      else
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
      if (handlers?.onFrame) handlers.onFrame(desiredConversationId, payload)
      else
        void useEventBusStore.getState().emit({
          type: 'chat:token',
          data: { conversation_id: desiredConversationId, event: payload },
        })
    }
  }

  return { start, stop, setActiveConversation }
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => globalThis.setTimeout(resolve, ms))
}
