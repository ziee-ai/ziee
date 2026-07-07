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
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'

/**
 * ITEM-7 / TEST-13 — composer drafts persist across navigation and clear on send.
 *
 * Unsent text survives navigating away and back; sending clears the persisted
 * draft. Only the LLM token-stream + message-history boundary is mocked.
 */

const DRAFT = 'a half-written thought I have not sent yet'
const COMPOSER = 'textarea[placeholder*="Type your message"]'

test.describe('Chat — composer draft persistence', () => {
  test.describe.configure({ retries: 1 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('restores an unsent draft after navigating away and back', async ({
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
  })

  test('clears the persisted draft after the message is sent', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(apiURL)

    // Send from an EXISTING conversation (no new-chat→create navigation), so the
    // onMessageSent draft-clear runs deterministically. Only the LLM token
    // stream + message history is mocked.
    const res = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ title: 'Draft clear test' }),
    })
    const convId = (await res.json()).id as string

    await mockChatTokenStream(page, [[startedEvent({}), completeEvent()]])
    await mockGetMessages(page, [mockUserMessage({ id: 'umsg_draft', text: DRAFT })])

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.locator(COMPOSER)).toBeVisible({ timeout: 30000 })
    await selectModelInDropdown(page, 'GPT-4o Mini')

    // Type a draft (persisted under this conversation's key) and send it.
    await page.locator(COMPOSER).fill(DRAFT)
    await page.getByRole('button', { name: 'Send message' }).click()

    // The composer clears on send (confirms onMessageSent ran + cleared the draft).
    await expect(page.locator(COMPOSER)).toHaveValue('', { timeout: 20000 })

    // Reload the same conversation: the draft is gone (not re-restored).
    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.locator(COMPOSER)).toBeVisible({ timeout: 30000 })
    await expect(page.locator(COMPOSER)).toHaveValue('')
  })
})
