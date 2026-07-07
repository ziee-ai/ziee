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
} from '../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'

/**
 * ITEM-7 / TEST-13 — composer drafts persist across navigation and clear on send.
 *
 * Unsent text survives navigating away and back; sending clears the persisted
 * draft. The `new`-chat bucket is exercised (type on the new-chat page, leave,
 * return, restored; then send and confirm it's gone). Only the LLM token-stream
 * boundary is mocked.
 */

const DRAFT = 'a half-written thought I have not sent yet'
const COMPOSER = 'textarea[placeholder*="Type your message"]'

test.describe('Chat — composer draft persistence', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('restores an unsent draft after navigating away and back, then clears on send', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await goToNewChatPage(page, baseURL)
    const textarea = page.locator(COMPOSER)
    await expect(textarea).toBeVisible({ timeout: 30000 })

    // Type a draft on the new-chat page.
    await textarea.fill(DRAFT)

    // Navigate away to the history page, then back to the new-chat page.
    await page.goto(`${baseURL}/chats`)
    await expect(page.locator(COMPOSER)).toHaveCount(0)
    await goToNewChatPage(page, baseURL)

    // The draft is restored.
    await expect(page.locator(COMPOSER)).toHaveValue(DRAFT)

    // Now send it (mocked token stream) — the composer + persisted draft clear.
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await mockChatTokenStream(page, [[startedEvent({}), completeEvent()]])
    await page.locator(COMPOSER).fill(DRAFT)
    await page.getByRole('button', { name: 'Send message' }).click()

    // Composer clears on send.
    await expect(page.locator(COMPOSER)).toHaveValue('', { timeout: 15000 })

    // Return to a fresh new-chat page: the draft is gone (cleared on send).
    await goToNewChatPage(page, baseURL)
    await expect(page.locator(COMPOSER)).toHaveValue('')
  })
})
