import { Page } from '@playwright/test'

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

export interface ChatStreamMock {
  /** How many times /messages/stream was intercepted. */
  callCount: () => number
  /** Bodies of intercepted requests, in call order. */
  capturedRequests: () => Array<{ url: string; body: unknown }>
}

/**
 * Mock /api/conversations/*\/messages/stream with a queue of event scripts.
 * Each call consumes one script; once exhausted, the last script is replayed
 * (so tests for a single send call don't have to provide more than they need).
 *
 * Call BEFORE the first send. Unregister via `page.unroute()` if a test needs
 * to swap behavior mid-flight (rare).
 */
export async function mockChatStream(
  page: Page,
  scripts: ScriptedSseEvent[][],
): Promise<ChatStreamMock> {
  let callIndex = 0
  const captured: Array<{ url: string; body: unknown }> = []

  await page.route(/\/api\/conversations\/[^/]+\/messages\/stream/, async (route, request) => {
    let body: unknown = null
    try {
      body = JSON.parse(request.postData() || '{}')
    } catch {
      /* leave as null */
    }
    captured.push({ url: request.url(), body })

    const script = scripts[callIndex] ?? scripts[scripts.length - 1]
    callIndex++

    await route.fulfill({
      status: 200,
      contentType: 'text/event-stream',
      body: serializeSseScript(script),
    })
  })

  return {
    callCount: () => callIndex,
    capturedRequests: () => [...captured],
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

  await page.route(/\/api\/mcp\/elicitation\/([^/]+)\/respond/, async (route, request) => {
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
  })

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
export function startedEvent(opts: {
  branchId?: string
  userMessageId?: string
  conversationId?: string
} = {}): ScriptedSseEvent {
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
export function completeEvent(opts: { finishReason?: string } = {}): ScriptedSseEvent {
  return {
    event: 'complete',
    data: {
      finish_reason: opts.finishReason ?? 'end_turn',
    },
  }
}
