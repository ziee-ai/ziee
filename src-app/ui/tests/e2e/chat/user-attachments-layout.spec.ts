import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { mockChatStream, startedEvent } from '../helpers/sse-mock-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import { FILE_ASSETS, attachFileViaUI } from './helpers/file-panel-helpers'

/**
 * User-message attachment layout.
 *
 * Attachments on a USER message render as a horizontal, wrapping row ABOVE the
 * text bubble (outside the bordered box) — NOT stacked vertically inside it.
 * See `ChatMessage.tsx` (splits `file_attachment` blocks into the
 * `data-testid="message-attachments"` row above the bubble).
 */
test.describe('Chat — user attachments render above the bubble (horizontal row)', () => {
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

  test('two attached files lay out horizontally in a row above the text bubble', async ({
    page,
    testInfra,
  }) => {
    // started-only stream: no `complete`, so loadMessages never runs and the
    // optimistic user bubble (with its file_attachment blocks) stays mounted
    // for the assertions. Same trick as inline-file-streaming-consistency.
    await mockChatStream(page, [[startedEvent({ userMessageId: 'umsg_attach' })]])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')

    // Attach two distinct files (real upload via the UI).
    await attachFileViaUI(page, FILE_ASSETS.png)
    await attachFileViaUI(page, FILE_ASSETS.txt)

    const text = 'here are my files'
    const textarea = page
      .locator('textarea[placeholder*="Type your message"]')
      .first()
    await textarea.fill(text)
    await page.getByRole('button', { name: 'Send message' }).click()

    // The sent user bubble.
    const userMsg = page
      .locator('[data-testid="chat-message"][data-role="user"]')
      .filter({ hasText: text })
      .first()
    await expect(userMsg).toBeVisible({ timeout: 15000 })

    // Attachments render in their own row, with both file cards.
    const row = userMsg.locator('[data-testid="message-attachments"]')
    await expect(row).toBeVisible()
    const cards = row.locator('[data-testid="file-card"]')
    await expect(cards).toHaveCount(2)

    // Laid out HORIZONTALLY: same row (similar y), increasing x.
    const b0 = await cards.nth(0).boundingBox()
    const b1 = await cards.nth(1).boundingBox()
    expect(b0).toBeTruthy()
    expect(b1).toBeTruthy()
    expect(b1!.x).toBeGreaterThan(b0!.x)
    expect(Math.abs(b1!.y - b0!.y)).toBeLessThan(20)

    // And ABOVE the message text (the row sits above the text line).
    const textNode = userMsg.getByText(text, { exact: false }).first()
    const tb = await textNode.boundingBox()
    expect(tb).toBeTruthy()
    expect(b0!.y).toBeLessThan(tb!.y)
  })
})
