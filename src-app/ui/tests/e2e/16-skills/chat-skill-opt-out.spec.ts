import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from '../09-chat/helpers/chat-helpers'
import { byTestId } from '../testid.ts'

// audit id c3b03b06dba6 — the only skills E2E specs were admin-gating + the
// settings list/detail load; the in-CHAT skill surface (the "+" dropdown →
// "Skills in this chat" → per-conversation opt-out panel, where a user controls
// which boot-synced skills the model sees in this conversation) was untested.
// This drives that surface end-to-end with the real per-conversation hide/unhide
// API (no live model needed — only the chat SSE/messages boundary is mocked).

test.describe('Skills — in-chat per-conversation opt-out', () => {
  test.describe.configure({ retries: 2 })

  test('open the in-chat skills panel and toggle a skill off for this conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const userMessageId = 'umsg_skill_1'
    await mockChatTokenStream(page, [
      [startedEvent({ userMessageId }), completeEvent()],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: userMessageId, text: 'hello' }),
      { id: 'amsg_skill_1', role: 'assistant', contents: [] },
    ])

    // Send a message → a real conversation is created (the "Skills in this chat"
    // "+" item is hidden until a conversation exists).
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await byTestId(page, 'chat-message-textarea').first().fill('hello')
    await byTestId(page, 'chat-input-send-btn').click()
    await page
      .locator('[data-testid="chat-message"][data-message-id="amsg_skill_1"]')
      .first()
      .waitFor({ state: 'visible', timeout: 15000 })

    // Open the "+" dropdown → "Skills in this chat".
    await byTestId(page, 'chat-input-add-btn').click()
    await byTestId(page, 'skill-conversation-menu-item').click()

    // The per-conversation skills modal opens and lists the boot-synced skills.
    const modal = byTestId(page, 'skill-conversation-dialog')
    await expect(modal).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'skill-conversation-list')).toBeVisible({
      timeout: 10000,
    })

    // At least one skill row with a visibility toggle (boot-synced built-ins).
    const firstSwitch = modal
      .locator('[data-testid^="skill-conversation-switch-"]')
      .first()
    await expect(firstSwitch).toBeVisible({ timeout: 10000 })

    // The skill is visible (available) by default → toggling it off hides it for
    // this conversation (real ConversationSkills.hide API call).
    await expect(firstSwitch).toBeChecked()
    await firstSwitch.click()
    await expect(firstSwitch).not.toBeChecked({ timeout: 10000 })
  })
})
