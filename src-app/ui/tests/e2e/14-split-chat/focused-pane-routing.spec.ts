import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — actions that were wrongly routed to the FOCUSED pane now act on
 * the pane they belong to (audit #2/#3). Both panes hold REAL conversations (no
 * idle-empty control):
 *  - TEST-95 Cmd/Ctrl-F opens the find bar in the FOCUSED pane only (each pane
 *    registers a window keydown; the un-gated handler opened it in every pane).
 *  - TEST-96 renaming pane B's title while pane A is focused updates pane B's
 *    conversation (verified via the API), not the focused pane A's.
 * No LLM (conversations need no messages for either assertion).
 */
test.describe('Split chat — focused-pane routing fixes', () => {
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

  const openAB = async (
    page: import('@playwright/test').Page,
    baseURL: string,
    convA: string,
    convB: string,
  ) => {
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 15000,
    })
  }

  test('TEST-95: Cmd/Ctrl-F opens the find bar in the FOCUSED pane only', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Focus Alpha')
    const convB = await mkConv(page, apiURL, token, 'Focus Bravo')
    await openAB(page, baseURL, convA, convB)
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')

    // Focus pane 0, press Ctrl-F → find bar in pane 0 ONLY.
    await pane0.click()
    await expect(pane0).toHaveClass(/ring-primary/)
    await page.keyboard.press('Control+f')
    await expect(pane0.getByTestId('conversation-find-bar')).toBeVisible({ timeout: 5000 })
    await expect(pane1.getByTestId('conversation-find-bar')).toHaveCount(0)

    // Close, focus pane 1, press Ctrl-F → now the bar opens in pane 1 ONLY.
    await page.keyboard.press('Escape')
    await expect(pane0.getByTestId('conversation-find-bar')).toHaveCount(0, { timeout: 5000 })
    await pane1.click()
    await expect(pane1).toHaveClass(/ring-primary/)
    await page.keyboard.press('Control+f')
    await expect(pane1.getByTestId('conversation-find-bar')).toBeVisible({ timeout: 5000 })
    await expect(pane0.getByTestId('conversation-find-bar')).toHaveCount(0)
  })

  test('TEST-96: renaming pane B while pane A is focused updates pane B, not A', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Rename Alpha')
    const convB = await mkConv(page, apiURL, token, 'Rename Bravo')
    await openAB(page, baseURL, convA, convB)
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')

    // Focus pane 0 (so the focused-pane bridge points at A), then rename pane B.
    await pane0.click()
    await expect(pane0).toHaveClass(/ring-primary/)

    await pane1.getByTestId('chat-title-edit-btn').click()
    const input = pane1.getByTestId('chat-title-input')
    await input.fill('Bravo Renamed')
    await pane1.getByTestId('chat-title-save-btn').click()

    // Pane B's displayed title updates; pane A's stays.
    await expect(pane1.getByTestId('conversation-title')).toContainText('Bravo Renamed', {
      timeout: 10000,
    })
    await expect(pane0.getByTestId('conversation-title')).toContainText('Rename Alpha')

    // The SAVE hit conversation B, not the focused A — verify server-side.
    const auth = { Authorization: `Bearer ${token}` }
    const bTitle = (await (await page.request.get(`${apiURL}/api/conversations/${convB}`, { headers: auth })).json()).title
    const aTitle = (await (await page.request.get(`${apiURL}/api/conversations/${convA}`, { headers: auth })).json()).title
    expect(bTitle).toBe('Bravo Renamed')
    expect(aTitle).toBe('Rename Alpha')
  })
})
