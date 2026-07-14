import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — the "Open in split view" button is HIDDEN once the workspace is
 * at MAX_PANES (TEST-113, ITEM-78). There's no room for another pane at the cap, so
 * the button would only produce a "cap reached" no-op — hide it instead. No LLM.
 */
test.describe('Split chat — split button hides at the pane cap', () => {
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

  test('split button is present below MAX_PANES and gone at MAX_PANES', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Cap A')
    const convB = await mkConv(page, apiURL, token, 'Cap B')
    const convC = await mkConv(page, apiURL, token, 'Cap C')

    // [A | B] — 2 panes, still room → the button is present.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').first().click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-1').getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Cap B', { timeout: 15000 })
    await expect(byTestId(page, 'chat-split-btn').first()).toBeVisible()

    // Add a 3rd pane (MAX_PANES) → the button disappears from every pane header.
    await byTestId(page, 'chat-pane-1').getByTestId('chat-split-btn').click()
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-2').getByTestId(`conversation-picker-item-${convC}`).click()
    await expect(byTestId(page, 'chat-pane-2').getByTestId('conversation-title')).toContainText('Cap C', { timeout: 15000 })
    await expect(byTestId(page, 'chat-split-btn')).toHaveCount(0, { timeout: 15000 })
  })
})
