import { Page, Route } from '@playwright/test'

/**
 * page.route helpers for mocking chat SSE streams and elicitation endpoints.
 *
 * The chat client (src/api-client/core.ts) reads the response body as a
 * text/event-stream, parsing lines of the form:
 *
 *   event: <event_name>
 *   data: <json>
 *   <blank line>
 *
 * For deterministic E2E tests we intercept POST /api/conversations/:id/messages/stream
 * and return a pre-baked event script per call. The chat extension then dispatches
 * each event (`mcpToolStart`, `mcpElicitationRequired`, `complete`, etc.) to the
 * appropriate handler the same as it would for real backend output.
 *
 * Tests can intercept multiple consecutive calls (e.g. tool approval triggers a
 * second sendMessage that re-enters the same endpoint) by passing an array of
 * scripts. The N-th call consumes the N-th script; calls beyond the array
 * length re-use the last script.
 */

export interface ScriptedSseEvent {
  event: string
  data: unknown
}

/** Serialize a script of events into SSE wire format. */
export function serializeSseScript(events: ScriptedSseEvent[]): string {
  let body = ''
  for (const evt of events) {
    body += `event: ${evt.event}\n`
    body += `data: ${JSON.stringify(evt.data)}\n\n`
  }
  return body
}

/** Generation frames ride an envelope `{conversationId, event:{type,…}}`;
 *  extension events (mcp*, titleUpdated, artifactCreated) are sent raw
 *  `{type,…}` — matching `ChatStreamClient.handleFrame`'s two shapes. */
const ENVELOPED_EVENTS = new Set(['started', 'content', 'complete', 'error'])

function serializeChatStreamFrames(
  events: ScriptedSseEvent[],
  conversationId: string,
): string {
  let body = ''
  for (const evt of events) {
    const inner = { type: evt.event, ...(evt.data as Record<string, unknown>) }
    const data = ENVELOPED_EVENTS.has(evt.event)
      ? { conversationId, event: inner }
      : inner
    body += `event: ${evt.event}\n`
    body += `data: ${JSON.stringify(data)}\n\n`
  }
  return body
}

export interface ChatTokenStreamMock {
  /** Number of POST /messages calls intercepted. */
  sendCount: () => number
  /** Bodies of intercepted POST /messages requests, in order. */
  capturedSends: () => Array<{ url: string; body: unknown }>
}

/**
 * Mock the NEW fire-and-forget chat path (replaces `mockChatStream`):
 *   - `PUT  /api/chat/stream/subscription` → 204
 *   - `POST /api/conversations/:id/messages` → `{user_message_id,
 *      assistant_message_id}` JSON (ids derived from the script's `started`
 *      event), and SIGNALS the matching send.
 *   - `GET  /api/chat/stream` → a `connected` handshake then, once the matching
 *      send fires, the script's frames (generation frames enveloped, ext events
 *      raw). The body then ends; the client reconnects and the next GET serves
 *      the next script. This per-send reconnect is what lets an interactive
 *      multi-step flow (send → approval panel → approve → resume) work without
 *      Playwright's one-shot `route.fulfill` having to push mid-response.
 *
 * Pass one script per expected send. Extra sends reuse the last script; a GET
 * past the last script just serves the handshake (idle).
 */
export async function mockChatTokenStream(
  page: Page,
  scripts: ScriptedSseEvent[][],
  options: { failSends?: Record<number, number> } = {},
): Promise<ChatTokenStreamMock> {
  const captured: Array<{ url: string; body: unknown }> = []

  // A producer/consumer queue coupling each POST (producer) to the next GET
  // reconnect (consumer). The client keeps ONE chat-stream open at a time and
  // re-opens it after each body ends, so each send's frames go out on a fresh
  // GET. The queue decouples their timing (a send may land before its GET
  // reconnects, or vice-versa). Works for ANY number of sends (resume reuses
  // the last script).
  type Payload = { conversationId: string; scriptIdx: number } | { failed: true }
  const queue: Payload[] = []
  const waiters: Array<(p: Payload) => void> = []
  const enqueue = (p: Payload) => {
    const w = waiters.shift()
    if (w) w(p)
    else queue.push(p)
  }
  const dequeue = (): Promise<Payload> =>
    queue.length > 0
      ? Promise.resolve(queue.shift() as Payload)
      : new Promise<Payload>(r => waiters.push(r))

  let sendNum = 0
  let getNum = 0

  await page.route(/\/api\/chat\/stream\/subscription$/, route =>
    route.fulfill({ status: 204, contentType: 'application/json', body: '' }),
  )

  await page.route(
    /\/api\/conversations\/[^/]+\/messages(\?|$)/,
    async (route, request) => {
      if (request.method() !== 'POST') {
        return route.fallback()
      }
      const url = request.url()
      const conversationId =
        url.match(/\/conversations\/([^/]+)\/messages/)?.[1] ?? ''
      let body: unknown = null
      try {
        body = JSON.parse(request.postData() || '{}')
      } catch {
        /* leave null */
      }
      captured.push({ url, body })

      const n = sendNum
      sendNum++

      const failStatus = options.failSends?.[n]
      if (failStatus) {
        enqueue({ failed: true })
        await route.fulfill({
          status: failStatus,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'simulated send failure' }),
        })
        return
      }

      const scriptIdx = Math.min(n, scripts.length - 1)
      const started = scripts[scriptIdx]?.find(e => e.event === 'started')
      const userMessageId =
        ((started?.data as Record<string, unknown> | undefined)
          ?.user_message_id as string) ?? `umsg_${n}`
      const assistantMessageId = `amsg_${n}_${Date.now()}`

      enqueue({ conversationId, scriptIdx })

      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          user_message_id: userMessageId,
          assistant_message_id: assistantMessageId,
        }),
      })
    },
  )

  // Fulfil a chat-stream GET, returning false if the connection was aborted
  // (client navigated/reconnected) before delivery — route.fulfill rejects on a
  // dead request. The caller re-queues undelivered frames so they aren't lost.
  const safeFulfill = async (
    route: Route,
    body: string,
  ): Promise<boolean> => {
    try {
      await route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body,
      })
      return true
    } catch {
      return false
    }
  }

  await page.route(/\/api\/chat\/stream(\?|$)/, async (route, request) => {
    if (request.method() !== 'GET') {
      return route.fallback()
    }
    const cid = getNum
    getNum++
    const handshake = `event: connected\ndata: ${JSON.stringify({
      connectionId: `conn_${cid}`,
    })}\n\n`

    const payload = await dequeue()
    if ('failed' in payload) {
      // The matching send failed — no frames; serve the handshake only.
      await safeFulfill(route, handshake)
      return
    }
    const body =
      handshake +
      serializeChatStreamFrames(scripts[payload.scriptIdx], payload.conversationId)
    // The client keeps only ONE chat-stream open and aborts the old one on every
    // navigation/reconnect, so a GET that dequeued a payload may already be dead
    // (e.g. goToNewChatPage tears down the connection that grabbed it). If
    // delivery fails, re-queue the frames so the next LIVE GET reconnect gets
    // them — otherwise the send's `complete` never arrives, the post-complete
    // /messages reload never fires, and the assistant bubble never mounts.
    const delivered = await safeFulfill(route, body)
    if (!delivered) {
      enqueue(payload)
    }
  })

  return {
    sendCount: () => sendNum,
    capturedSends: () => [...captured],
  }
}

export interface ChatStreamMock {
  /** How many times /messages/stream was intercepted. */
  callCount: () => number
  /** Bodies of intercepted requests, in call order. */
  capturedRequests: () => Array<{ url: string; body: unknown }>
}

/**
 * Mock the chat send + stream with a queue of event scripts (one per send).
 *
 * Thin adapter over {@link mockChatTokenStream} — the fire-and-forget refactor
 * removed the old direct-SSE `POST …/messages/stream` route this helper used to
 * target, replacing it with `POST …/messages` (returns `{user_message_id,
 * assistant_message_id}`) + a long-lived `GET /api/chat/stream` that pushes
 * enveloped frames `{conversationId, event:{type,…}}`. `mockChatTokenStream`
 * models that path and consumes the SAME `ScriptedSseEvent[][]`, so this helper
 * now delegates to it: callers keep their existing scripts (+ `mockGetMessages`
 * for the post-`complete` reload) but the assistant frames actually arrive.
 *
 * Call BEFORE the first send. The producer/consumer queue couples each POST to
 * the next `GET /api/chat/stream` reconnect (see mockChatTokenStream).
 */
export async function mockChatStream(
  page: Page,
  scripts: ScriptedSseEvent[][],
): Promise<ChatStreamMock> {
  // The old direct-SSE-over-POST route (`…/messages/stream`) was removed by the
  // fire-and-forget refactor. `mockChatTokenStream` models the current path
  // (POST `…/messages` → frames over the long-lived `GET /api/chat/stream`) and
  // consumes the SAME `ScriptedSseEvent[][]`, so `mockChatStream` is now a thin
  // adapter over it — existing callers (explicit-messageId scripts paired with
  // `mockGetMessages`) keep working unchanged, but now actually stream.
  const mock = await mockChatTokenStream(page, scripts)
  return {
    callCount: () => mock.sendCount(),
    capturedRequests: () => mock.capturedSends(),
  }
}

export interface ElicitationResponseCapture {
  /** All POST /respond bodies seen so far, in arrival order. */
  responses: () => Array<{ elicitationId: string; body: unknown }>
  /** Convenience: number of POSTs captured. */
  count: () => number
}

/**
 * Capture every POST to /api/mcp/elicitation/{id}/respond. Returns 200 with
 * `{ success: true }` (matching the real backend's success shape) so the
 * frontend treats it as accepted.
 */
export async function captureElicitationResponses(
  page: Page,
): Promise<ElicitationResponseCapture> {
  const responses: Array<{ elicitationId: string; body: unknown }> = []

  await page.route(
    /\/api\/mcp\/elicitation\/([^/]+)\/respond/,
    async (route, request) => {
      const match = request.url().match(/\/elicitation\/([^/]+)\/respond/)
      const elicitationId = match?.[1] ?? ''
      let body: unknown = null
      try {
        body = JSON.parse(request.postData() || '{}')
      } catch {
        /* leave as null */
      }
      responses.push({ elicitationId, body })

      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ success: true }),
      })
    },
  )

  return {
    responses: () => [...responses],
    count: () => responses.length,
  }
}

// ──────────────────────────────────────────────────────────────────────────
// Event builders — each returns a well-formed ScriptedSseEvent matching the
// payloads the backend would actually send. Pure functions; no Playwright
// dependency, easy to compose into scripts.
// ──────────────────────────────────────────────────────────────────────────

/**
 * Initial `started` event the chat store expects to clear the temp user
 * message and apply branch state. Must be first in any script.
 */
export function startedEvent(
  opts: {
    branchId?: string
    userMessageId?: string
    conversationId?: string
  } = {},
): ScriptedSseEvent {
  return {
    event: 'started',
    data: {
      branch_id: opts.branchId ?? 'br_mock_1',
      user_message_id: opts.userMessageId ?? 'umsg_mock_1',
      conversation_id: opts.conversationId,
    },
  }
}

export function mcpToolStartEvent(opts: {
  toolUseId: string
  toolName: string
  server: string
  input?: unknown
}): ScriptedSseEvent {
  return {
    event: 'mcpToolStart',
    data: {
      tool_use_id: opts.toolUseId,
      tool_name: opts.toolName,
      server: opts.server,
      input: opts.input ?? {},
    },
  }
}

/**
 * Tool-call approval gate. The extension creates the streaming message + the
 * tool_use content block on receipt (mcpToolStart is NOT sent when approval
 * is required — see extension.tsx:358-469).
 */
export function mcpApprovalRequiredEvent(opts: {
  toolUseId: string
  toolName: string
  server: string
  serverId?: string
  input?: unknown
}): ScriptedSseEvent {
  return {
    event: 'mcpApprovalRequired',
    data: {
      tool_use_id: opts.toolUseId,
      tool_name: opts.toolName,
      server: opts.server,
      server_id: opts.serverId ?? opts.server,
      input: opts.input ?? {},
    },
  }
}

export function mcpElicitationRequiredEvent(opts: {
  elicitationId: string
  messageId: string
  message: string
  requestedSchema: unknown
  server?: string
}): ScriptedSseEvent {
  return {
    event: 'mcpElicitationRequired',
    data: {
      elicitation_id: opts.elicitationId,
      message_id: opts.messageId,
      message: opts.message,
      requested_schema: opts.requestedSchema,
      server: opts.server ?? 'mock-server',
    },
  }
}

export function mcpToolCompleteEvent(opts: {
  toolUseId: string
  isError?: boolean
  result?: unknown
}): ScriptedSseEvent {
  return {
    event: 'mcpToolComplete',
    data: {
      tool_use_id: opts.toolUseId,
      is_error: opts.isError ?? false,
      result: opts.result,
    },
  }
}

/**
 * Artifact-created event — a tool produced a file. The MCP extension turns
 * this into a resource_link on the matching tool_result block (keyed by
 * `toolUseId`), which the file extension renders inline. `fileId` makes it a
 * backend-owned artifact (rendered via the authenticated /api/files path).
 */
export function artifactCreatedEvent(opts: {
  /** Omit to simulate an older backend that doesn't send tool_use_id — the
   *  frontend then falls back to the last tool_use block. */
  toolUseId?: string
  fileId: string
  filename: string
  mimeType?: string
  fileSize?: number
}): ScriptedSseEvent {
  return {
    event: 'artifactCreated',
    data: {
      ...(opts.toolUseId !== undefined ? { tool_use_id: opts.toolUseId } : {}),
      file_id: opts.fileId,
      filename: opts.filename,
      mime_type: opts.mimeType,
      file_size: opts.fileSize ?? 1024,
    },
  }
}

/**
 * Plain text delta — appends to the streaming message's text content block,
 * or creates one if none exists yet.
 */
export function textDeltaEvent(opts: {
  delta: string
  messageId?: string
}): ScriptedSseEvent {
  return {
    event: 'content',
    data: {
      content: [{ type: 'text_delta', delta: opts.delta }],
      message_id: opts.messageId,
    },
  }
}

/** Stream-end event. Every script SHOULD end with this. */
export function completeEvent(
  opts: { finishReason?: string } = {},
): ScriptedSseEvent {
  return {
    event: 'complete',
    data: {
      finish_reason: opts.finishReason ?? 'end_turn',
    },
  }
}

// ──────────────────────────────────────────────────────────────────────────
// GET /messages mock — critical for chat-stream tests because the chat
// store calls loadMessages() AFTER the SSE stream completes, which wipes
// the optimistic streamingMessage state. Returning a synthetic message list
// here keeps the elicitation/tool-use content blocks alive in the UI.
// ──────────────────────────────────────────────────────────────────────────

export interface MockMessageContent {
  id?: string
  message_id?: string
  content_type: string
  content: unknown
  sequence_order?: number
  created_at?: string
  updated_at?: string
}

export interface MockMessageWithContent {
  id: string
  role: 'user' | 'assistant'
  contents: MockMessageContent[]
  originated_from_id?: string
  edit_count?: number
  created_at?: string
}

/**
 * Mock GET /api/conversations/*\/messages to return the given message list
 * for ANY conversation id. The chat store calls this after the SSE stream
 * completes (`complete` event handler), so without this mock the optimistic
 * elicitation/tool-use content gets wiped.
 */
export async function mockGetMessages(
  page: Page,
  messages: MockMessageWithContent[],
): Promise<void> {
  const fullMessages = messages.map(m => ({
    id: m.id,
    role: m.role,
    contents: m.contents.map((c, idx) => ({
      id: c.id ?? `${m.id}-content-${idx}`,
      message_id: c.message_id ?? m.id,
      content_type: c.content_type,
      content: c.content,
      sequence_order: c.sequence_order ?? idx,
      created_at: c.created_at ?? new Date().toISOString(),
      updated_at: c.updated_at ?? new Date().toISOString(),
    })),
    originated_from_id: m.originated_from_id ?? '',
    edit_count: m.edit_count ?? 0,
    created_at: m.created_at ?? new Date().toISOString(),
  }))

  await page.route(
    /\/api\/conversations\/[^/]+\/messages(\?|$)/,
    async (route, req) => {
      if (req.method() !== 'GET') {
        return route.fallback()
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(fullMessages),
      })
    },
  )
}

/** Convenience: build a user message with one text content block. */
export function mockUserMessage(opts: {
  id: string
  text: string
}): MockMessageWithContent {
  return {
    id: opts.id,
    role: 'user',
    contents: [
      {
        content_type: 'text',
        content: { type: 'text', text: opts.text },
      },
    ],
  }
}

/** Convenience: assistant message carrying an elicitation_request content block. */
export function mockAssistantElicitationMessage(opts: {
  id: string
  elicitationId: string
  message: string
  requestedSchema: unknown
  server?: string
  status?: 'pending' | 'accepted' | 'declined' | 'cancelled'
  responseContent?: Record<string, unknown>
}): MockMessageWithContent {
  return {
    id: opts.id,
    role: 'assistant',
    contents: [
      {
        content_type: 'elicitation_request',
        content: {
          type: 'elicitation_request',
          elicitation_id: opts.elicitationId,
          message_id: opts.id,
          message: opts.message,
          requested_schema: opts.requestedSchema,
          server: opts.server ?? 'mock-server',
          status: opts.status ?? 'pending',
          response_content: opts.responseContent,
        },
      },
    ],
  }
}

/** Convenience: assistant message carrying a tool_use content block. */
export function mockAssistantToolUseMessage(opts: {
  id: string
  toolUseId: string
  toolName: string
  serverId: string
  input?: unknown
}): MockMessageWithContent {
  return {
    id: opts.id,
    role: 'assistant',
    contents: [
      {
        content_type: 'tool_use',
        content: {
          type: 'tool_use',
          id: opts.toolUseId,
          name: opts.toolName,
          server_id: opts.serverId,
          input: opts.input ?? {},
        },
      },
    ],
  }
}
