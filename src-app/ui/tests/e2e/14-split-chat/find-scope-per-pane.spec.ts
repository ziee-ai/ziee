import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — the find bar SEARCHES the pane it belongs to (TEST-97, audit #9).
 * ConversationFindBar read the focused-pane bridge (`Stores.Chat`) for the
 * conversation to search + the message to jump to; now it reads `useChatPaneOrNull()
 * ?.store`. Discriminator: with pane B focused, opening find in pane A and searching
 * for a word that exists ONLY in pane A's conversation returns a match — pre-fix it
 * searched the focused pane B (0 matches). Real send via the bridge (find hits the
 * server-side message search, so messages must exist).
 */
test.describe('Split chat — find bar searches its own pane', () => {
  test('find in pane A (pane B focused) searches pane A, not the focused pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(150000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mkConv('Find Alpha')
    const convB = await mkConv('Find Bravo')

    // [A | B] split.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()

    // Send a message with a UNIQUE word in each pane so each conversation has
    // distinct searchable content.
    const send = async (pane: ReturnType<typeof byTestId>, word: string) => {
      const ta = pane.locator('textarea[placeholder*="Type your message"]')
      await ta.click()
      await ta.fill(`the special keyword here is ${word}`)
      await pane.getByRole('button', { name: 'Send message' }).click()
      await expect(pane.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    }
    await send(pane0, 'ZEBRAWORD')
    await send(pane1, 'KIWIWORD')

    // Focus pane B, then open find in pane A and search for A's unique word.
    await pane1.click()
    await expect(pane1).toHaveClass(/opacity-100/)
    await pane0.getByTestId('conversation-find-toggle-btn').click()
    await expect(pane0.getByTestId('conversation-find-bar')).toBeVisible({ timeout: 5000 })
    await pane0.getByTestId('conversation-find-input').fill('ZEBRAWORD')

    // Pane A's find finds A's content ("N of M") — it searched pane A, not the
    // focused pane B (which has no ZEBRAWORD).
    await expect(pane0.getByTestId('conversation-find-count')).toContainText('of', { timeout: 15000 })

    // Sanity: searching pane A for B's word (KIWIWORD) → "No results" — confirms
    // the search really is scoped to pane A's conversation, not a merged/global set.
    await pane0.getByTestId('conversation-find-input').fill('KIWIWORD')
    await expect(pane0.getByTestId('conversation-find-count')).toContainText('No results', { timeout: 15000 })
  })
})
