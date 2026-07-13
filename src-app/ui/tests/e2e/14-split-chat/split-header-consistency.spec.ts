import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — the per-pane header matches the single-pane app header
 * (TEST-108, ITEM-71 / FB-18). Regressions from hand-rolling a parallel header:
 *  - height was 44px (h-11) vs HeaderBarContainer's 50px.
 *  - the sidebar collapse/expand toggle was UNCLICKABLE in split: the focused
 *    pane's `z-10` matched the fixed `z-10` toggle and, being later in the DOM,
 *    covered it. A REAL `.click()` (not synthetic dispatch) exercises the fix —
 *    Playwright's actionability check fails if the element is covered.
 * No LLM.
 */
test.describe('Split chat — per-pane header matches the app header', () => {
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

  test('pane header is 50px tall AND the sidebar toggle stays clickable with the leftmost pane focused', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Hdr Alpha')
    const convB = await mkConv(page, apiURL, token, 'Hdr Bravo')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })

    // Height matches HeaderBarContainer's 50px (was 44).
    const hbox = await pane0.getByTestId('chat-pane-header').boundingBox()
    expect(hbox && Math.round(hbox.height)).toBe(50)

    // Focus the LEFTMOST pane (pane 0) — that's the pane whose focus-ring z used to
    // cover the fixed toggle at the top-left.
    await pane0.click()
    await expect(pane0).toHaveClass(/ring-primary/)

    const toggle = byTestId(page, 'layout-sidebar-toggle-button')
    await expect(toggle).toBeVisible()
    const header = pane0.getByTestId('chat-pane-header')
    const padLeft = () => header.evaluate(el => getComputedStyle(el).paddingLeft)

    // COLLAPSE the sidebar (a real click — if the focused pane covered the toggle,
    // this would fail with "element intercepts pointer events"). aria-expanded → false.
    if ((await toggle.getAttribute('aria-expanded')) === 'true') await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'false', { timeout: 5000 })
    // Collapsed → the LEFTMOST pane reserves 48px to clear the toggle + traffic lights.
    expect(await padLeft()).toBe('48px')

    // EXPAND again — proves the toggle is STILL clickable while collapsed + pane 0
    // focused (the crux of the bug). aria-expanded → true; inset drops back to 12.
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'true', { timeout: 5000 })
    expect(await padLeft()).toBe('12px')
  })
})
