import type { Page, Route } from '@playwright/test'
import {
  mockChatTokenStream,
  startedEvent,
  mcpToolStartEvent,
  mcpToolCompleteEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
  type MockMessageWithContent,
} from '../../helpers/sse-mock-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
} from '../helpers/chat-helpers'

/**
 * Test fixtures for the inline file-preview feature.
 *
 * Each helper makes it easy to drive a chat → tool_use → tool_result
 * flow without a real LLM or sandbox. The mocked SSE stream emits the
 * shape the chat extension's stream parser expects; the mocked
 * /messages reload feeds the persisted message back into the chat
 * store after the stream `complete` event fires.
 */

/** Shape of a resource_link as it lives inside a tool_result content
 *  block on the wire. Matches `McpContentData::ToolResult.resource_links`
 *  on the backend (with frontend-friendly snake_case JSON keys). */
export interface MockResourceLink {
  uri: string
  name?: string
  mime_type?: string
  size?: number
  is_saved?: boolean
  /** Backing File id for backend-owned artifacts. When set, the inline
   *  preview resolves the File entity and renders via the authenticated
   *  `/api/files/{id}/...` path (mock those endpoints with `mockBackendFile`). */
  file_id?: string
}

export interface SeedToolResultOpts {
  /** Optional plain assistant text rendered as a `text` content block
   *  alongside the tool_use/tool_result pair. */
  text?: string
  /** Resource links carried by the tool_result block. Each test
   *  asserts on these in some way. */
  resourceLinks: MockResourceLink[]
  /** Custom tool name / server id for the tool_use block. Defaults
   *  to `get_resource_link` / a stable test server id. */
  toolName?: string
  serverId?: string
  /** Override the assistant message id (useful when a test seeds
   *  multiple messages and needs to address them individually). */
  assistantMessageId?: string
  /** Override the user message id likewise. */
  userMessageId?: string
}

/**
 * Build a tool_use content block matching the chat extension's
 * persisted shape. Used by `mockGetMessages` to replay the message
 * after the SSE complete event triggers a /messages reload.
 */
export function mockToolUseContent(opts: {
  toolUseId: string
  toolName: string
  serverId: string
  input?: unknown
}): MockMessageContent {
  return {
    content_type: 'tool_use',
    content: {
      type: 'tool_use',
      id: opts.toolUseId,
      name: opts.toolName,
      server_id: opts.serverId,
      input: opts.input ?? {},
    },
  }
}

/**
 * Build a tool_result content block carrying the given resource links.
 * `McpContentData::ToolResult` exposes `resource_links` in both the runtime
 * and the schema-facing variant, so the generated `MessageContentDataToolResult`
 * (api-client/types.ts) types it as `resource_links?: ResourceLink[] | null`.
 * `MessageFilesView` reads it directly off that typed field.
 */
export function mockToolResultContent(opts: {
  toolUseId: string
  toolName: string
  resourceLinks: MockResourceLink[]
  content?: string
}): MockMessageContent {
  return {
    content_type: 'tool_result',
    content: {
      type: 'tool_result',
      tool_use_id: opts.toolUseId,
      name: opts.toolName,
      content: opts.content ?? '',
      resource_links: opts.resourceLinks,
    },
  }
}

/**
 * One-shot: mocks the SSE stream + the /messages reload + navigates
 * to a fresh chat + sends a user message. After this returns, an
 * assistant message containing the tool_use/tool_result blocks (and
 * optional text) is mounted in the chat with the resource_links
 * available for inline rendering.
 */
export async function seedAssistantWithToolResult(
  page: Page,
  baseURL: string,
  opts: SeedToolResultOpts,
): Promise<{ assistantMessageId: string; userMessageId: string }> {
  const toolUseId = `tu_test_${Math.random().toString(36).slice(2, 9)}`
  const assistantMessageId = opts.assistantMessageId ?? `amsg_rl_${Math.random().toString(36).slice(2, 9)}`
  const userMessageId = opts.userMessageId ?? `umsg_rl_${Math.random().toString(36).slice(2, 9)}`
  const toolName = opts.toolName ?? 'get_resource_link'
  const serverId = opts.serverId ?? 'test-server-id'

  const events = [
    startedEvent({ userMessageId }),
    mcpToolStartEvent({
      toolUseId,
      toolName,
      server: serverId,
      input: { filename: opts.resourceLinks[0]?.name ?? 'unknown' },
    }),
    mcpToolCompleteEvent({
      toolUseId,
      isError: false,
      result: { resource_links: opts.resourceLinks },
    }),
    ...(opts.text ? [textDeltaEvent({ delta: opts.text, messageId: assistantMessageId })] : []),
    completeEvent(),
  ]
  await mockChatTokenStream(page, [events])

  const assistantContents: MockMessageContent[] = [
    mockToolUseContent({ toolUseId, toolName, serverId }),
    mockToolResultContent({
      toolUseId,
      toolName,
      resourceLinks: opts.resourceLinks,
    }),
  ]
  if (opts.text) {
    assistantContents.push({
      content_type: 'text',
      content: { type: 'text', text: opts.text },
    })
  }

  const assistantMessage: MockMessageWithContent = {
    id: assistantMessageId,
    role: 'assistant',
    contents: assistantContents,
  }

  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'do the thing' }),
    assistantMessage,
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = page
    .locator('textarea[placeholder*="Type your message"]')
    .first()
  await textarea.fill('do the thing')
  await page.getByRole('button', { name: 'Send message' }).click()

  // Wait for the persisted assistant bubble to mount via the
  // post-complete /messages reload.
  await page
    .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
    .first()
    .waitFor({ state: 'visible', timeout: 15000 })

  return { assistantMessageId, userMessageId }
}

/**
 * Build a multi-tool-result assistant message used by aggregation /
 * dedup tests. Differs from `seedAssistantWithToolResult` in that
 * it stitches multiple tool_use/tool_result pairs into one
 * assistant message.
 */
export async function seedAssistantWithMultipleToolResults(
  page: Page,
  baseURL: string,
  toolResults: Array<{
    toolName?: string
    serverId?: string
    resourceLinks: MockResourceLink[]
  }>,
  options?: { trailingText?: string },
): Promise<{ assistantMessageId: string }> {
  const assistantMessageId = `amsg_multi_${Math.random().toString(36).slice(2, 9)}`
  const userMessageId = `umsg_multi_${Math.random().toString(36).slice(2, 9)}`

  // Build events. Each tool_use generates a fresh tool_use_id.
  const events: Parameters<typeof mockChatTokenStream>[1][number] = [
    startedEvent({ userMessageId }),
  ]
  const contents: MockMessageContent[] = []
  for (const tr of toolResults) {
    const toolUseId = `tu_multi_${Math.random().toString(36).slice(2, 9)}`
    const toolName = tr.toolName ?? 'get_resource_link'
    const serverId = tr.serverId ?? 'test-server-id'
    events.push(
      mcpToolStartEvent({ toolUseId, toolName, server: serverId, input: {} }),
      mcpToolCompleteEvent({
        toolUseId,
        isError: false,
        result: { resource_links: tr.resourceLinks },
      }),
    )
    contents.push(
      mockToolUseContent({ toolUseId, toolName, serverId }),
      mockToolResultContent({ toolUseId, toolName, resourceLinks: tr.resourceLinks }),
    )
  }
  if (options?.trailingText) {
    events.push(textDeltaEvent({ delta: options.trailingText, messageId: assistantMessageId }))
    contents.push({
      content_type: 'text',
      content: { type: 'text', text: options.trailingText },
    })
  }
  events.push(completeEvent())

  await mockChatTokenStream(page, [events])
  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'do multiple things' }),
    { id: assistantMessageId, role: 'assistant', contents },
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = page
    .locator('textarea[placeholder*="Type your message"]')
    .first()
  await textarea.fill('do multiple things')
  await page.getByRole('button', { name: 'Send message' }).click()

  await page
    .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
    .first()
    .waitFor({ state: 'visible', timeout: 15000 })

  // A run of ≥2 consecutive tool calls folds into a collapsed
  // "N tools called" group (McpToolGroupCard). Expand it so each
  // tool_result's inline file previews render for the assertions.
  const groupToggle = page.locator('[data-testid="mcp-toolgroup-details-btn"]')
  if ((await groupToggle.count()) > 0) {
    await groupToggle.first().click()
  }

  return { assistantMessageId }
}

/**
 * Result of `mockResourceLinkUrl` — the test can assert on how many
 * times each URL was fetched (for cache / dedup tests).
 */
export interface ResourceLinkUrlMock {
  /** Per-URL request count, in the order URLs were registered. */
  callCount: (url: string) => number
  /** Total intercept count. */
  totalCalls: () => number
}

export interface MockResourceLinkResponse {
  url: string
  body: string | Buffer
  status?: number
  /** Sets `Content-Type` header on the response. The viewer-registry
   *  matches on the resource_link's `mime_type` field separately —
   *  this header just controls what the fetch returns. */
  contentType?: string
}

/**
 * Register one or more Playwright route handlers for resource_link
 * URLs. Each `url` is matched as an exact string (case-sensitive).
 *
 * Call BEFORE seedAssistantWithToolResult so the routes are active
 * when the inline viewer body fetches.
 */
export async function mockResourceLinkUrls(
  page: Page,
  responses: MockResourceLinkResponse[],
): Promise<ResourceLinkUrlMock> {
  const counts = new Map<string, number>()
  for (const r of responses) {
    counts.set(r.url, 0)
    // Capture by value — Playwright runs the handler many times.
    const resp = r
    // Playwright's `page.route(url, ...)` with a plain string matches
    // against the FULL URL, not just the pathname. The hook fetches
    // `/api/...` which becomes `http://localhost:<vitePort>/api/...`
    // at request time, so we need a glob (`**/<pathname>`) to match
    // regardless of host:port. Without this the fetch falls through
    // to vite → proxied to backend → 401, and the viewer body shows
    // "Failed to load file content."
    await page.route(`**${resp.url}`, async (route: Route) => {
      counts.set(resp.url, (counts.get(resp.url) ?? 0) + 1)
      await route.fulfill({
        status: resp.status ?? 200,
        contentType: resp.contentType ?? 'text/plain',
        body: resp.body,
      })
    })
  }
  return {
    callCount: url => counts.get(url) ?? 0,
    totalCalls: () => Array.from(counts.values()).reduce((a, b) => a + b, 0),
  }
}

/**
 * One-shot helper for the most common case: a single resource_link
 * URL the test wants to intercept. Returns the same mock shape so
 * tests can still inspect call counts.
 */
export async function mockResourceLinkUrl(
  page: Page,
  url: string,
  body: string | Buffer,
  opts: { status?: number; contentType?: string } = {},
): Promise<ResourceLinkUrlMock> {
  return mockResourceLinkUrls(page, [{ url, body, ...opts }])
}

export interface MockBackendFileOpts {
  fileId: string
  filename: string
  mimeType?: string
  /** When set, mocks `GET /api/files/{id}/text` to return this content
   *  (and reports `text_page_count: 1` on the entity). */
  textContent?: string
  fileSize?: number
}

/**
 * Mock the backend File endpoints a file-backed inline preview hits:
 * `GET /api/files/{id}` (the entity, fetched by `getMessageFile`) and,
 * when `textContent` is given, `GET /api/files/{id}/text` (the body the
 * text/code/CSV viewers render via the authenticated `{file}` path).
 *
 * Register BEFORE seeding so the routes are live when the preview mounts.
 */
export async function mockBackendFile(
  page: Page,
  opts: MockBackendFileOpts,
): Promise<void> {
  const entity = {
    id: opts.fileId,
    filename: opts.filename,
    file_size: opts.fileSize ?? (opts.textContent?.length ?? 0),
    mime_type: opts.mimeType,
    has_thumbnail: false,
    preview_page_count: 0,
    text_page_count: opts.textContent !== undefined ? 1 : 0,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    user_id: '00000000-0000-0000-0000-000000000000',
    created_by: 'mcp',
    processing_metadata: null,
  }
  // Playwright evaluates routes most-recently-registered first. Register the
  // broad entity route first and the specific /text route last so /text wins
  // for the text URL while the entity URL falls through to the entity route.
  await page.route(`**/api/files/${opts.fileId}`, route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(entity),
    }),
  )
  if (opts.textContent !== undefined) {
    await page.route(`**/api/files/${opts.fileId}/text`, route =>
      route.fulfill({
        status: 200,
        contentType: 'text/plain; charset=utf-8',
        body: opts.textContent!,
      }),
    )
  }
}
