import { test, expect } from '../../fixtures/test-context'
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
  mockChatStream,
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
 * MessageFilesView dispatcher behavior — aggregation, dedup, DOM
 * positioning, presence/absence.
 */

test.describe('Inline file previews — MessageFilesView aggregation', () => {
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

  test('MessageFilesView is absent for text-only assistant message', async ({
    page,
    testInfra,
  }) => {
    // Plain text response — no tool_use/tool_result anywhere. Confirms
    // the footer slot renders nothing when there are no resource_links.
    const userMsgId = 'umsg_textonly'
    const assistantMsgId = 'amsg_textonly'
    await mockChatStream(page, [
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
          {
            content_type: 'text',
            content: { type: 'text', text: 'just a text reply' },
          },
        ],
      },
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
    await textarea.fill('say hi')
    await page.getByRole('button', { name: 'Send message' }).click()

    const bubble = page.locator(`[data-testid="chat-message"][data-message-id="${assistantMsgId}"]`)
    await expect(bubble).toBeVisible({ timeout: 15000 })
    await expect(bubble.locator('[data-testid="message-files-view"]')).toHaveCount(0)
  })

  test('multiple resource_links in one tool_result all render', async ({
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
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(3, { timeout: 10000 })
  })

  test('resource_links across MULTIPLE tool_results aggregated into one footer', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithMultipleToolResults(page, testInfra.baseURL, [
      {
        resourceLinks: [
          { uri: '/api/files/agg-m1/download', name: 'a.png', mime_type: 'image/png' },
          { uri: '/api/files/agg-m2/download', name: 'b.png', mime_type: 'image/png' },
        ],
      },
      {
        resourceLinks: [
          { uri: '/api/files/agg-m3/download', name: 'c.png', mime_type: 'image/png' },
          { uri: '/api/files/agg-m4/download', name: 'd.png', mime_type: 'image/png' },
        ],
      },
    ])
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(4, { timeout: 10000 })
    // All inside the same footer.
    expect(await page.locator('[data-testid="message-files-view"]').count()).toBe(1)
  })

  test('dedup by URI: same file referenced twice renders once', async ({
    page,
    testInfra,
  }) => {
    const shared = '/api/files/agg-dup/download'
    await seedAssistantWithMultipleToolResults(page, testInfra.baseURL, [
      {
        resourceLinks: [
          { uri: shared, name: 'p.png', mime_type: 'image/png' },
          { uri: '/api/files/agg-unique-1/download', name: 'x.png', mime_type: 'image/png' },
        ],
      },
      {
        resourceLinks: [
          { uri: shared, name: 'p.png', mime_type: 'image/png' },
          { uri: '/api/files/agg-unique-2/download', name: 'y.png', mime_type: 'image/png' },
        ],
      },
    ])
    const previews = page.locator('[data-testid="inline-file-preview"]')
    // 4 raw entries → 1 dup removed = 3 unique.
    await expect(previews).toHaveCount(3, { timeout: 10000 })
    // Specifically, the shared URI appears exactly once.
    expect(await page.locator(`[data-file-uri="${shared}"]`).count()).toBe(1)
  })

  test('dedup preserves first-seen order', async ({ page, testInfra }) => {
    // Order: A, B, A (dup), C → final order should be A, B, C.
    await seedAssistantWithMultipleToolResults(page, testInfra.baseURL, [
      {
        resourceLinks: [
          { uri: '/api/files/ord-A/download', name: 'a.png', mime_type: 'image/png' },
          { uri: '/api/files/ord-B/download', name: 'b.png', mime_type: 'image/png' },
        ],
      },
      {
        resourceLinks: [
          { uri: '/api/files/ord-A/download', name: 'a.png', mime_type: 'image/png' },
          { uri: '/api/files/ord-C/download', name: 'c.png', mime_type: 'image/png' },
        ],
      },
    ])
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(3, { timeout: 10000 })
    const orderedUris = await previews.evaluateAll(els =>
      els.map(el => el.getAttribute('data-file-uri')),
    )
    expect(orderedUris).toEqual([
      '/api/files/ord-A/download',
      '/api/files/ord-B/download',
      '/api/files/ord-C/download',
    ])
  })

  test('MessageFilesView appears AFTER content blocks in DOM', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      text: 'Here is your file.',
      resourceLinks: [
        { uri: '/api/files/ord-after/download', name: 'p.png', mime_type: 'image/png' },
      ],
    })
    const bubble = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    const filesView = bubble.locator('[data-testid="message-files-view"]')
    await expect(filesView).toBeVisible({ timeout: 10000 })
    // The MessageContext.Provider in ChatMessage.tsx renders the footer
    // slot AFTER the content-blocks .map(). Assert via DOM Y position.
    const lastContentTop = await bubble
      .locator('.ant-card, [data-chat-extension-slot], .w-full.overflow-x-auto')
      .last()
      .evaluate(el => el.getBoundingClientRect().top)
    const filesViewTop = await filesView.evaluate(el => el.getBoundingClientRect().top)
    expect(filesViewTop).toBeGreaterThanOrEqual(lastContentTop)
  })

  test('mixed file types in one message all render via their viewers', async ({
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
    const view = page.locator('[data-testid="message-files-view"]').first()
    await expect(view).toBeVisible({ timeout: 10000 })
    // Image body: an <img>.
    await expect(view.locator('img').first()).toBeVisible({ timeout: 10000 })
    // CSV body: a <table>.
    await expect(view.locator('table').first()).toBeVisible({ timeout: 10000 })
    // Markdown body: an <h1> from the fetched markdown.
    await expect(view.locator('h1').first()).toHaveText('Hi', { timeout: 10000 })
  })
})
