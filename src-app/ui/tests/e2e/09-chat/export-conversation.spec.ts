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
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'

// audit id 09d22fb649b2 — conversation export (chat/extensions/export) had no
// E2E coverage. Seed a conversation with a user + assistant text turn, then
// drive the toolbar Export dropdown → "Export as Markdown" and assert a real
// download fires carrying the conversation text. Only the chat SSE/messages
// boundary is mocked; the export blob/anchor path runs for real.

const ASSISTANT_TEXT = 'The capital of France is Paris.'
const USER_TEXT = 'What is the capital of France?'

function assistantTextMessage(id: string, text: string): MockMessageWithContent {
  return {
    id,
    role: 'assistant',
    contents: [{ content_type: 'text', content: { type: 'text', text } }],
  }
}

test.describe('Chat — conversation export', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('exports the conversation as Markdown via the toolbar dropdown', async ({
    page,
    testInfra,
  }) => {
    const userMessageId = 'umsg_export_1'
    const assistantMessageId = 'amsg_export_1'

    await mockChatTokenStream(page, [
      [startedEvent({ userMessageId }), completeEvent()],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: userMessageId, text: USER_TEXT }),
      assistantTextMessage(assistantMessageId, ASSISTANT_TEXT),
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page
      .locator('textarea[placeholder*="Type your message"]')
      .first()
    await textarea.fill(USER_TEXT)
    await page.getByRole('button', { name: 'Send message' }).click()

    // Assistant message rendered → the Export button is present (it hides when
    // there are no messages).
    await page
      .locator(
        `[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`,
      )
      .first()
      .waitFor({ state: 'visible', timeout: 15000 })

    await page.getByRole('button', { name: 'Export' }).click()

    const downloadPromise = page.waitForEvent('download')
    await page.getByRole('menuitem', { name: 'Export as Markdown' }).click()
    const download = await downloadPromise

    // Filename follows `conversation-<id8>.md`.
    expect(download.suggestedFilename()).toMatch(/^conversation-.*\.md$/)

    // Success toast + the exported file carries the conversation text.
    await expect(
      page.getByText('Conversation exported as Markdown'),
    ).toBeVisible({ timeout: 5000 })

    const path = await download.path()
    const { readFileSync } = await import('fs')
    const md = readFileSync(path, 'utf8')
    expect(md).toContain(ASSISTANT_TEXT)
    expect(md).toContain('**Assistant**')
  })
})
