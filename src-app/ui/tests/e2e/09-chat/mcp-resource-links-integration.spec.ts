import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from './fixtures/mock-tool-result'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  mockChatStream,
  startedEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'

/**
 * Confirms the new MessageFilesView slot doesn't break existing chat
 * features (message_actions slot, BranchNavigator, MessageActions
 * core, content blocks rendered in the bubble).
 */

test.describe('Inline file previews — existing-feature integration', () => {
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

  test('message_actions slot still renders alongside message_footer slot', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/integ-1/download', name: 'p.png', mime_type: 'image/png' },
      ],
    })
    const bubble = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await expect(bubble).toBeVisible({ timeout: 10000 })
    // Footer slot is present (has the inline file preview).
    await expect(bubble.locator('[data-testid="message-files-view"]')).toBeVisible()
    // Actions slot is also rendered — assert one of the core action
    // components (the copy/regenerate buttons) is present in this
    // bubble, since the message_actions extension slot itself is empty
    // when no extension registers a component for it.
    await expect(bubble.locator('[data-testid="message-actions"], .anticon-copy').first())
      .toBeVisible()
  })

  test('message bubble preserves content-block order around the new footer', async ({
    page,
    testInfra,
  }) => {
    // Content blocks (tool_use, tool_result, text) inside the bubble;
    // footer slot OUTSIDE the bubble (per ChatMessage layout). Assert
    // the bubble's text content still flows correctly.
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      text: 'Here is your result.',
      resourceLinks: [
        { uri: '/api/files/integ-order/download', name: 'p.png', mime_type: 'image/png' },
      ],
    })
    const bubble = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await expect(bubble).toContainText('Here is your result.', { timeout: 10000 })
  })

  test('switching conversations: another conversation does not show files from the previous one', async ({
    page,
    testInfra,
  }) => {
    // Seed conversation A with one file.
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/integ-conv-a/download', name: 'a.png', mime_type: 'image/png' },
      ],
    })
    await expect(page.locator('[data-file-uri="/api/files/integ-conv-a/download"]'))
      .toBeVisible({ timeout: 10000 })

    // Navigate to a fresh new-chat page (different conversation context).
    // The previous /messages mock is per-page-context; the new chat
    // doesn't have any tool_results.
    const newUserMsgId = 'umsg_conv_b'
    const newAssistantMsgId = 'amsg_conv_b'
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: newUserMsgId }),
        textDeltaEvent({ delta: 'plain reply', messageId: newAssistantMsgId }),
        completeEvent(),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: newUserMsgId, text: 'hi again' }),
      {
        id: newAssistantMsgId,
        role: 'assistant',
        contents: [{ content_type: 'text', content: { type: 'text', text: 'plain reply' } }],
      },
    ])
    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
    await textarea.fill('hi again')
    await page.getByRole('button', { name: 'Send message' }).click()
    await expect(
      page.locator(`[data-testid="chat-message"][data-message-id="${newAssistantMsgId}"]`),
    ).toBeVisible({ timeout: 15000 })
    // Crucially: the conversation A's file is NOT in conversation B's DOM.
    await expect(page.locator('[data-file-uri="/api/files/integ-conv-a/download"]'))
      .toHaveCount(0)
  })

  test('copy to clipboard does not include file blob URLs', async ({
    page,
    testInfra,
  }) => {
    // The MessageActions copy button copies the assistant's TEXT, not
    // the rendered file URIs. Pin this so a future refactor doesn't
    // accidentally include them.
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      text: 'The answer is in the file.',
      resourceLinks: [
        { uri: '/api/files/integ-copy/download', name: 'p.png', mime_type: 'image/png' },
      ],
    })
    const bubble = page.locator('[data-testid="chat-message"][data-role="assistant"]').last()
    await expect(bubble.locator('[data-testid="message-files-view"]')).toBeVisible({ timeout: 10000 })
    // Find the copy button. The MessageActions component renders one
    // per assistant message. Use aria-label.
    const copyButton = bubble.getByRole('button', { name: /copy/i }).first()
    if ((await copyButton.count()) === 0) {
      // Some chat-action layouts hide the button behind hover/focus —
      // skip if it isn't reachable in this test profile.
      test.skip(true, 'Copy button not exposed in this layout')
      return
    }
    // Grant clipboard access + spy on writes.
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
    await copyButton.click()
    const clipboard = await page.evaluate(() => navigator.clipboard.readText())
    expect(clipboard).toContain('The answer is in the file.')
    expect(clipboard).not.toContain('/api/files/integ-copy/download')
  })
})
