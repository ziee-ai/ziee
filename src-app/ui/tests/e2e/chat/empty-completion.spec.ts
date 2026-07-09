import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  mockChatStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'

/**
 * Empty-completion notice spec (TEST-4).
 *
 * When a turn finalises with only reasoning (or nothing) and no tool call, the
 * persisted assistant message has no user-visible answer. `ChatMessage.tsx`
 * detects that from the message CONTENT at render time (it does NOT consume the
 * stream's `finish_reason`; the backend's `finish_reason: "empty"` is an
 * independent telemetry signal asserted separately in the backend test) and
 * shows an inline notice so the chat never appears to silently hang. Because the
 * detection is render-time it must also survive a page reload — this spec
 * asserts the notice appears after `complete` and again after a reload.
 */

// An assistant message whose only content is a thinking block — the reported
// "renders the thinking block, then stops" case.
const assistantThinkingOnly = (id: string): MockMessageWithContent => ({
  id,
  role: 'assistant',
  contents: [
    {
      content_type: 'thinking',
      content: { type: 'thinking', thinking: 'Let me work this out…' },
    },
  ],
})

const assistantBubble = (page: Page) =>
  page.locator('[data-testid="chat-message"][data-role="assistant"]').last()

test.describe('Empty-completion notice', () => {
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

  test('reasoning-only turn shows the notice, and it survives a reload', async ({
    page,
    testInfra,
  }) => {
    // Stream ends with only reasoning + a terminal `empty` finish_reason, then
    // the post-complete reload returns the persisted thinking-only message.
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_empty_1' }),
        completeEvent({ finishReason: 'empty' }),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_empty_1', text: 'do something' }),
      assistantThinkingOnly('amsg_empty_1'),
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')

    const textarea = byTestId(page, 'chat-message-textarea').first()
    await textarea.fill('do something')
    await byTestId(page, 'chat-input-send-btn').click()

    // The assistant bubble mounts (thinking card), and the empty-completion
    // notice renders inside it because no user-visible answer was produced.
    await expect(assistantBubble(page)).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-empty-completion-notice')).toBeVisible({
      timeout: 15000,
    })

    // Reload-robust: the notice is derived at render time from the persisted
    // message, so it must reappear after a full reload (no live stream).
    await page.reload()
    await expect(assistantBubble(page)).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-empty-completion-notice')).toBeVisible({
      timeout: 15000,
    })
  })
})
