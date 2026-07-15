import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — the URL always tracks the FOCUSED pane (TEST-109, ITEM-72 /
 * FB-19). The bug: the only path that navigated was the sidebar-open hook, so
 * opening a pane via the Split button / an edge-drop / the picker — or clicking a
 * different pane to focus it — changed the focused conversation WITHOUT updating
 * the URL. The address bar stayed stuck on the FIRST conversation, so "open in new
 * tab" (which reuses the current URL) reopened the already-showing conversation
 * ("the current rendering one") instead of the one the user was focused on.
 *
 * This spec drives two ACTIVE panes and asserts the address bar mirrors whichever
 * pane is focused at each step — the missing workspace→URL direction. No LLM.
 */
test.describe('Split chat — the URL tracks the focused pane', () => {
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

  test('opening a pane + focusing panes keeps the URL in lockstep with the focused conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'URL-Track Alpha')
    const convB = await mkConv(page, apiURL, token, 'URL-Track Bravo')

    // Single-pane baseline: the URL is convA (unchanged behavior).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(page).toHaveURL(new RegExp(`/chat/${convA}$`))

    // Split, then pick convB into the NEW (focused) pane. The URL MUST follow the
    // focused pane to convB — before the fix it stayed on convA (the bug).
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })
    await expect(page).toHaveURL(new RegExp(`/chat/${convB}$`), { timeout: 10000 })

    // Focus the LEFT pane (convA) by clicking it → the URL follows back to convA.
    // This is the regression crux: a URL-based "open in new tab" here now targets
    // the conversation the user is actually looking at, not a stale first one.
    await pane0.click()
    await expect(pane0).toHaveClass(/opacity-100/)
    await expect(page).toHaveURL(new RegExp(`/chat/${convA}$`), { timeout: 10000 })

    // Focus the RIGHT pane (convB) again → the URL follows to convB.
    await pane1.click()
    await expect(pane1).toHaveClass(/opacity-100/)
    await expect(page).toHaveURL(new RegExp(`/chat/${convB}$`), { timeout: 10000 })
  })
})
