import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  seedAssistantWithToolResult,
  seedAssistantWithMultipleToolResults,
  mockResourceLinkUrl,
} from './fixtures/mock-tool-result'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'

// 1x1 transparent PNG — keeps ImageBody's <img> in DOM (see
// mcp-resource-links-dispatch.spec.ts for the full explanation).
const TINY_PNG = Buffer.from(
  '89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000D49444154789C6200010000050001' +
    '0D0A2DB40000000049454E44AE426082',
  'hex',
)

/**
 * Tool-returned files render INLINE at their tool_result block's position
 * (file extension's `tool_result` content renderer), NOT aggregated into a
 * footer. Each tool_result owns its own `tool-result-files` group; dedupe is
 * per-block; files appear where the tool returned them (before trailing text).
 */
test.describe('Inline file previews — per-block positioning', () => {
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

  test('no inline files for a text-only assistant message', async ({
    page,
    testInfra,
  }) => {
    const userMsgId = 'umsg_textonly'
    const assistantMsgId = 'amsg_textonly'
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: userMsgId }),
        textDeltaEvent({ delta: 'just a text reply', messageId: assistantMsgId }),
        completeEvent(),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: userMsgId, text: 'say hi' }),
      {
        id: assistantMsgId,
        role: 'assistant',
        contents: [
          { content_type: 'text', content: { type: 'text', text: 'just a text reply' } },
        ],
      },
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
    await textarea.fill('say hi')
    await byTestId(page, 'chat-input-send-btn').click()

    const bubble = page.locator(`[data-testid="chat-message"][data-message-id="${assistantMsgId}"]`)
    await expect(bubble).toBeVisible({ timeout: 15000 })
    await expect(bubble.locator('[data-testid="tool-result-files"]')).toHaveCount(0)
    await expect(bubble.locator('[data-testid="inline-file-preview"]')).toHaveCount(0)
  })

  test('multiple resource_links in ONE tool_result render in one group', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/agg-1/download', name: 'a.png', mime_type: 'image/png' },
        { uri: '/api/files/agg-2/download', name: 'b.png', mime_type: 'image/png' },
        { uri: '/api/files/agg-3/download', name: 'c.png', mime_type: 'image/png' },
      ],
    })
    await expect(page.locator('[data-testid="inline-file-preview"]')).toHaveCount(3, {
      timeout: 10000,
    })
    // One tool_result block → exactly one inline group (no footer).
    await expect(page.locator('[data-testid="tool-result-files"]')).toHaveCount(1)
  })

  test('files from MULTIPLE tool_results render at each block (separate groups)', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithMultipleToolResults(page, testInfra.baseURL, [
      {
        resourceLinks: [
          { uri: '/api/files/m1/download', name: 'a.png', mime_type: 'image/png' },
          { uri: '/api/files/m2/download', name: 'b.png', mime_type: 'image/png' },
        ],
      },
      {
        resourceLinks: [
          { uri: '/api/files/m3/download', name: 'c.png', mime_type: 'image/png' },
          { uri: '/api/files/m4/download', name: 'd.png', mime_type: 'image/png' },
        ],
      },
    ])
    await expect(page.locator('[data-testid="inline-file-preview"]')).toHaveCount(4, {
      timeout: 10000,
    })
    // TWO tool_result blocks → TWO separate inline groups (the old behavior
    // aggregated all four into one footer; this is the fix).
    await expect(page.locator('[data-testid="tool-result-files"]')).toHaveCount(2)
  })

  test('per-block dedup: same URI twice in ONE tool_result renders once', async ({
    page,
    testInfra,
  }) => {
    const shared = '/api/files/dup-in-block/download'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: shared, name: 'p.png', mime_type: 'image/png' },
        { uri: shared, name: 'p.png', mime_type: 'image/png' },
        { uri: '/api/files/dup-unique/download', name: 'x.png', mime_type: 'image/png' },
      ],
    })
    await expect(page.locator('[data-testid="inline-file-preview"]')).toHaveCount(2, {
      timeout: 10000,
    })
    expect(await page.locator(`[data-file-uri="${shared}"]`).count()).toBe(1)
  })

  test('same URI across DIFFERENT tool_results renders at each (no cross-block dedup)', async ({
    page,
    testInfra,
  }) => {
    const shared = '/api/files/cross-block/download'
    await seedAssistantWithMultipleToolResults(page, testInfra.baseURL, [
      { resourceLinks: [{ uri: shared, name: 'p.png', mime_type: 'image/png' }] },
      { resourceLinks: [{ uri: shared, name: 'p.png', mime_type: 'image/png' }] },
    ])
    // Each tool genuinely returned the file → it shows once per tool_result.
    await expect(page.locator(`[data-file-uri="${shared}"]`)).toHaveCount(2, {
      timeout: 10000,
    })
  })

  test('file renders INLINE before trailing narration text', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      text: 'Here is your file.',
      resourceLinks: [
        { uri: '/api/files/before-text/download', name: 'p.png', mime_type: 'image/png' },
      ],
    })
    const bubble = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    const preview = bubble.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    const text = bubble.getByText('Here is your file.')
    await expect(text).toBeVisible()

    // Persisted block order is [tool_use, tool_result(file), text], so the
    // inline preview must sit ABOVE the trailing text — the opposite of the
    // old footer that pushed every file to the very end of the message.
    const previewBox = await preview.boundingBox()
    const textBox = await text.boundingBox()
    expect(previewBox).not.toBeNull()
    expect(textBox).not.toBeNull()
    expect(previewBox!.y).toBeLessThan(textBox!.y)
  })

  test('mixed file types in one tool_result render via their viewers', async ({
    page,
    testInfra,
  }) => {
    const pngUri = '/api/files/mixed-png/download'
    const csvUri = '/api/files/mixed-csv/download'
    const mdUri = '/api/files/mixed-md/download'
    await mockResourceLinkUrl(page, pngUri, TINY_PNG, { contentType: 'image/png' })
    await mockResourceLinkUrl(page, csvUri, 'a,b\n1,2\n', { contentType: 'text/csv' })
    await mockResourceLinkUrl(page, mdUri, '# Hi', { contentType: 'text/markdown' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: pngUri, name: 'a.png', mime_type: 'image/png' },
        { uri: csvUri, name: 'b.csv', mime_type: 'text/csv' },
        { uri: mdUri, name: 'c.md', mime_type: 'text/markdown' },
      ],
    })
    const group = page.locator('[data-testid="tool-result-files"]').first()
    await expect(group).toBeVisible({ timeout: 10000 })
    await expect(group.locator('img').first()).toBeVisible({ timeout: 10000 })
    await expect(group.locator('table').first()).toBeVisible({ timeout: 10000 })
    // The 3rd file (markdown) sits lower now that the run is wrapped in the group
    // card; InlineFilePreview viewport-gates a body until it scrolls within ~800px,
    // so scroll the md preview into view before asserting its rendered <h1>.
    const mdPreview = group.locator('[data-testid="inline-file-preview"]').nth(2)
    await mdPreview.scrollIntoViewIfNeeded()
    await expect(group.locator('h1').first()).toHaveText('Hi', { timeout: 10000 })
  })
})
