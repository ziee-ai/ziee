import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  mcpToolStartEvent,
  mcpToolCompleteEvent,
  artifactCreatedEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import { mockBackendFile } from './fixtures/mock-tool-result'

/**
 * Bug 1 — the FileCard flash. A tool artifact used to be injected as a
 * `file_attachment` block (a FileCard) during streaming, then disappear when
 * the post-complete reload replaced it with a footer InlineFilePreview. The
 * fix injects the artifact as a `resource_link` on the tool_result block, so
 * it renders as the SAME InlineFilePreview during streaming AND after reload —
 * never a FileCard, never a disappear/swap.
 */
test.describe('Inline file — streaming/persistence consistency (no FileCard flash)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  async function send(page: import('@playwright/test').Page, baseURL: string, text: string) {
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
    await textarea.fill(text)
    await page.getByRole('button', { name: 'Send message' }).click()
  }

  test('DURING streaming the artifact renders as an inline preview, not a FileCard', async ({
    page,
    testInfra,
  }) => {
    const toolUseId = 'tu_stream_art'
    // Resolve the backend File entity with a DISTINCT filename so the
    // assertion proves real entity resolution (InlineFilePreview prefers
    // file.filename) rather than the resource_link fallback name 'plot.png'.
    await mockBackendFile(page, {
      fileId: 'file-stream-art',
      filename: 'resolved-stream.png',
      mimeType: 'image/png',
    })
    // No `complete` event → the stream ends without triggering loadMessages,
    // so the assertions observe the live streaming-injected state.
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_art' }),
        mcpToolStartEvent({ toolUseId, toolName: 'make_plot', server: 'test-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: 'done' }),
        artifactCreatedEvent({
          toolUseId,
          fileId: 'file-stream-art',
          filename: 'plot.png',
          mimeType: 'image/png',
        }),
      ],
    ])

    await send(page, testInfra.baseURL, 'make a plot')

    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 15000 })
    // Resolved entity name (proves getMessageFile resolution, not the stub).
    await expect(preview).toContainText('resolved-stream.png')
    // The whole point: NO transient FileCard for the artifact.
    await expect(page.locator('[data-testid="file-card"]')).toHaveCount(0)
  })

  test('artifact preview persists across the post-complete reload (no disappear/swap)', async ({
    page,
    testInfra,
  }) => {
    const toolUseId = 'tu_persist_art'
    const fileId = 'file-persist-art'
    // Distinct resolved name proves entity resolution after the reload.
    await mockBackendFile(page, { fileId, filename: 'resolved-plot.png', mimeType: 'image/png' })
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_persist' }),
        mcpToolStartEvent({ toolUseId, toolName: 'make_plot', server: 'test-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: 'done' }),
        artifactCreatedEvent({ toolUseId, fileId, filename: 'plot.png', mimeType: 'image/png' }),
        completeEvent(),
      ],
    ])
    // Persisted state the post-complete reload returns: tool_use + tool_result
    // carrying the resource_link (file_id-backed), matching what the backend
    // stores. The artifact must keep rendering as the same inline preview.
    const assistantMsgId = 'amsg_persist'
    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_persist', text: 'make a plot' }),
      {
        id: assistantMsgId,
        role: 'assistant',
        contents: [
          {
            content_type: 'tool_use',
            content: { type: 'tool_use', id: toolUseId, name: 'make_plot', server_id: 'test-server', input: {} },
          },
          {
            content_type: 'tool_result',
            content: {
              type: 'tool_result',
              tool_use_id: toolUseId,
              content: '',
              resource_links: [
                { uri: `/api/files/${fileId}`, file_id: fileId, name: 'plot.png', mime_type: 'image/png' },
              ],
            },
          },
        ],
      },
    ])

    await send(page, testInfra.baseURL, 'make a plot')

    // After the complete → reload, the inline preview is still present at the
    // tool_result position and there is still no FileCard.
    const bubble = page.locator(`[data-testid="chat-message"][data-message-id="${assistantMsgId}"]`)
    await expect(bubble).toBeVisible({ timeout: 15000 })
    await expect(bubble.locator('[data-testid="inline-file-preview"]')).toHaveCount(1)
    await expect(bubble.locator('[data-testid="inline-file-preview"]')).toContainText('resolved-plot.png')
    await expect(page.locator('[data-testid="file-card"]')).toHaveCount(0)
  })

  test('multiple artifacts from one tool merge into a single tool_result group', async ({
    page,
    testInfra,
  }) => {
    // Two artifactCreated events for the SAME tool_use_id → the handler's
    // find-or-create + dedupe-by-file_id path collects both resource_links
    // into ONE tool_result block (one inline group, two previews).
    const toolUseId = 'tu_multi_art'
    await mockBackendFile(page, { fileId: 'mf1', filename: 'one.png', mimeType: 'image/png' })
    await mockBackendFile(page, { fileId: 'mf2', filename: 'two.png', mimeType: 'image/png' })
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_multi' }),
        mcpToolStartEvent({ toolUseId, toolName: 'make_plots', server: 'test-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: 'done' }),
        artifactCreatedEvent({ toolUseId, fileId: 'mf1', filename: 'one.png', mimeType: 'image/png' }),
        artifactCreatedEvent({ toolUseId, fileId: 'mf2', filename: 'two.png', mimeType: 'image/png' }),
        // A duplicate of mf1 must be deduped by file_id (still 2 previews).
        artifactCreatedEvent({ toolUseId, fileId: 'mf1', filename: 'one.png', mimeType: 'image/png' }),
      ],
    ])

    await send(page, testInfra.baseURL, 'make plots')

    await expect(page.locator('[data-testid="tool-result-files"]')).toHaveCount(1, { timeout: 15000 })
    await expect(page.locator('[data-testid="inline-file-preview"]')).toHaveCount(2)
  })

  test('artifact without tool_use_id falls back to the last tool_use block', async ({
    page,
    testInfra,
  }) => {
    // Older-backend path: artifactCreated carries no tool_use_id. The handler
    // walks back to the most recent tool_use and attaches the resource_link
    // there, so the file still renders inline.
    const toolUseId = 'tu_fallback'
    await mockBackendFile(page, { fileId: 'fb1', filename: 'fallback.png', mimeType: 'image/png' })
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_fb' }),
        mcpToolStartEvent({ toolUseId, toolName: 'make_plot', server: 'test-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: 'done' }),
        // No toolUseId → exercises the last-tool_use fallback.
        artifactCreatedEvent({ fileId: 'fb1', filename: 'fallback.png', mimeType: 'image/png' }),
      ],
    ])

    await send(page, testInfra.baseURL, 'make a plot')

    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 15000 })
    await expect(preview).toContainText('fallback.png')
    await expect(page.locator('[data-testid="file-card"]')).toHaveCount(0)
  })

  test('post-tool narration streams into a new text block, below the inline preview', async ({
    page,
    testInfra,
  }) => {
    // Exercises the text-segmentation change: leading text, then a tool +
    // artifact, then trailing narration. The trailing text must land in a NEW
    // block AFTER the inline preview (not merge up into the leading text).
    // No completeEvent → observe the live streamed order (no /messages reload).
    const toolUseId = 'tu_seg'
    await mockBackendFile(page, { fileId: 'seg1', filename: 'seg.png', mimeType: 'image/png' })
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_seg' }),
        textDeltaEvent({ delta: 'Let me make a plot.', messageId: 'amsg_seg' }),
        mcpToolStartEvent({ toolUseId, toolName: 'make_plot', server: 'test-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: 'done' }),
        artifactCreatedEvent({ toolUseId, fileId: 'seg1', filename: 'seg.png', mimeType: 'image/png' }),
        textDeltaEvent({ delta: 'Here is the result.', messageId: 'amsg_seg' }),
      ],
    ])

    await send(page, testInfra.baseURL, 'make a plot')

    const bubble = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    const preview = bubble.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 15000 })
    const trailing = bubble.getByText('Here is the result.')
    await expect(trailing).toBeVisible()
    // Trailing narration is its own block, positioned AFTER the inline preview.
    const previewBox = await preview.boundingBox()
    const trailingBox = await trailing.boundingBox()
    expect(previewBox).not.toBeNull()
    expect(trailingBox).not.toBeNull()
    expect(trailingBox!.y).toBeGreaterThan(previewBox!.y)
    // Leading text stays above the preview (it did NOT absorb the trailing text).
    const leadingBox = await bubble.getByText('Let me make a plot.').boundingBox()
    expect(leadingBox).not.toBeNull()
    expect(leadingBox!.y).toBeLessThan(previewBox!.y)
  })
})
