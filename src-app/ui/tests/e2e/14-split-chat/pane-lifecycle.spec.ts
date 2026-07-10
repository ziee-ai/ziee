import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — pane lifecycle (TEST-29 / TEST-52, ITEM-29). MAX_PANES = 3.
 *  - Pop-out MOVES a pane out of the split (opens its own window AND removes the
 *    pane, so there are never two live copies); the survivor keeps its state.
 *  - Closing panes down to one EXITS split to the survivor's single-pane view.
 *  - Opening beyond MAX_PANES is prevented with a "Replace focused pane" offer.
 * No LLM (conversations created via the API).
 */
test.describe('Split chat — pane lifecycle', () => {
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

  // Build a [A | B] split (two existing conversations) via the recent-row ⋯-menu.
  const openAB = async (
    page: import('@playwright/test').Page,
    baseURL: string,
    convA: string,
    convB: string,
  ) => {
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    const rowB = byTestId(page, `chat-recent-conversations-menu-item-${convB}`)
    await expect(rowB).toBeVisible({ timeout: 20000 })
    await rowB.hover()
    await byTestId(page, `chat-recent-row-actions-btn-${convB}`).click()
    await byTestId(page, `chat-recent-row-menu-${convB}-item-open-in-split`).click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
  }

  test('pop-out moves a pane out of the split (opens a window and removes the pane)', async ({
    page,
    context,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Popout Alpha')
    const convB = await mkConv(page, apiURL, token, 'Popout Bravo')
    await openAB(page, baseURL, convA, convB)

    // Pop out pane 1 (conv B). A new top-level page opens AND pane 1 leaves the
    // workspace → the split collapses to pane 0's single-pane view.
    const pane1 = byTestId(page, 'chat-pane-1')
    const [popup] = await Promise.all([
      context.waitForEvent('page'),
      pane1.getByTestId('chat-open-in-new-window').click(),
    ])
    await popup.waitForLoadState('domcontentloaded')
    expect(popup.url()).toContain(`/chat/${convB}`)
    // Back in the original window: down to a single pane (split-chat-view gone).
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0, { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-header')).toHaveCount(0)
    await popup.close()
  })

  test('closing panes down to one exits split to the single-pane view', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Close Alpha')
    const convB = await mkConv(page, apiURL, token, 'Close Bravo')
    await openAB(page, baseURL, convA, convB)

    // Close pane 1 → one survivor (pane 0) → SplitView.reset() → single-pane.
    await byTestId(page, 'chat-pane-1').getByTestId('chat-pane-close').click()
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0, { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-header')).toHaveCount(0)
    // The survivor's single-pane surface is present (title + composer).
    await expect(byTestId(page, 'conversation-title')).toBeVisible()
    await expect(page.locator('textarea[placeholder*="Type your message"]')).toBeVisible()
  })

  test('opening beyond MAX_PANES (3) is prevented with a replace-focused offer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Cap Alpha')
    const convB = await mkConv(page, apiURL, token, 'Cap Bravo')
    const convC = await mkConv(page, apiURL, token, 'Cap Charlie')
    const convD = await mkConv(page, apiURL, token, 'Cap Delta')
    await openAB(page, baseURL, convA, convB)

    // Add a 3rd pane (C) via Cmd-click → now at MAX_PANES = 3.
    await byTestId(page, `chat-recent-conversations-menu-item-${convC}`).click({
      modifiers: ['ControlOrMeta'],
    })
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })

    // A 4th open-in-new-pane (D) is capped: the reducer offers a confirm dialog
    // ("Replace focused pane") instead of silently exceeding 3 panes.
    await byTestId(page, `chat-recent-conversations-menu-item-${convD}`).click({
      modifiers: ['ControlOrMeta'],
    })
    const replaceBtn = page.getByRole('button', { name: 'Replace focused pane' })
    await expect(replaceBtn).toBeVisible({ timeout: 15000 })
    await replaceBtn.click()
    // Still capped at 3 panes — D replaced the focused pane, no 4th pane appeared.
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-3')).toHaveCount(0)
  })
})
