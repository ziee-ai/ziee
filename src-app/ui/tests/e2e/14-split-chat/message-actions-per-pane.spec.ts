import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — message actions act on the pane they're rendered in (TEST-58,
 * ITEM-38). Regenerate / Edit / branch prev-next bind to the pane's OWN store
 * (`useChatPaneOrNull()?.store`), NOT `SplitView.focusedPaneId` — so an action on
 * pane B's message acts on pane B even when pane A is focused (no wrong-pane
 * auto-send / branch corruption). Uses the local OpenAI-compatible bridge.
 */
test.describe('Split chat — message actions per-pane', () => {
  const mkConv = async (
    page: import('@playwright/test').Page,
    apiURL: string,
    token: string,
    title: string,
  ): Promise<string> => {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title },
    })
    expect(res.status()).toBeLessThan(300)
    return (await res.json()).id as string
  }

  test('Regenerate / Edit / branch on pane B act on pane B even when pane A is focused', async ({
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
    const convA = await mkConv(page, apiURL, token, 'MsgAct Alpha')
    const convB = await mkConv(page, apiURL, token, 'MsgAct Bravo')

    // [A | B] split via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()

    // Send a first turn in pane 1 (B) so it has a user + assistant message.
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await inputB.click()
    await inputB.fill('Reply with exactly the single word: PING')
    await pane1.getByRole('button', { name: 'Send message' }).click()
    await expect(pane1.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    await expect(pane1.locator('[data-role="assistant"]')).toBeVisible({ timeout: 60000 })

    // Focus pane 0 (so focusedPaneId = pane 0), then act on pane 1's messages.
    await pane0.click()
    await expect(pane0).toHaveClass(/ring-primary/)

    // EDIT pane 1's user message → its text is restored into PANE 1's composer,
    // while pane 0's composer stays empty (render-pane binding, not focused pane).
    const userMsg = pane1.locator('[data-role="user"]').first()
    await userMsg.hover()
    await pane1.getByTestId('edit-message-button').first().click()
    await expect(inputB).toHaveValue(/PING/, { timeout: 15000 })
    await expect(pane0.locator('textarea[placeholder*="Type your message"]')).toHaveValue('')
    // Cancel the edit (Esc) to restore the message list before regenerating.
    await inputB.press('Escape')

    // REGENERATE pane 1's assistant reply → a fresh assistant stream in pane 1;
    // pane 0 receives nothing. Regenerating also forks a branch → the branch
    // navigator appears under the forked message in pane 1.
    const assistantMsg = pane1.locator('[data-role="assistant"]').first()
    await assistantMsg.hover()
    await pane1.getByTestId('regenerate-button').first().click()
    await expect(pane1.locator('[data-role="assistant"]')).toBeVisible({ timeout: 60000 })
    await expect(pane0.locator('[data-role="assistant"]')).toHaveCount(0)
    await expect(pane0.locator('[data-role="user"]')).toHaveCount(0)

    // The branch navigator (2 branches now) is scoped to pane 1; stepping it acts
    // on pane 1 only.
    const branchNav = pane1.getByTestId('branch-navigator').first()
    await expect(branchNav).toBeVisible({ timeout: 20000 })
    await branchNav.getByTestId('chat-branch-prev-btn').click()
    await expect(pane0.getByTestId('branch-navigator')).toHaveCount(0)
  })
})
