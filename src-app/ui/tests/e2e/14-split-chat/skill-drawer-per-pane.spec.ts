import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — the "Skills in this chat" drawer is PER-PANE (TEST-94, audit #1).
 * The store was a single global `{open}`, and the host + menu item render once per
 * pane, so opening skills in one pane rendered EVERY pane's dialog (stacked dialogs
 * on different conversations). Now the store is keyed by `openConversationId` and
 * both composer slots read the pane's own conversation, so exactly ONE dialog (the
 * pane whose conversation was opened) appears. The count is the discriminator:
 * pre-fix = 2 dialogs, post-fix = 1. No LLM.
 */
test.describe('Split chat — skills-in-chat drawer per-pane', () => {
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

  test('opening skills in pane B renders exactly ONE dialog (not one per pane)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Skill Alpha')
    const convB = await mkConv(page, apiURL, token, 'Skill Bravo')

    // [A | B] split so BOTH panes' composer hosts are mounted.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 15000,
    })

    // Open "Skills in this chat" from pane B's + menu.
    await pane1.click()
    await pane1.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'skill-conversation-menu-item').first().click()

    // Exactly ONE dialog (keyed to pane B's conversation) — not one per pane.
    await expect(byTestId(page, 'skill-conversation-dialog')).toHaveCount(1, { timeout: 10000 })
    await expect(byTestId(page, 'skill-conversation-dialog')).toBeVisible()

    // Closing it clears the single opened conversation (no lingering pane-A copy).
    await page.keyboard.press('Escape')
    await expect(byTestId(page, 'skill-conversation-dialog')).toHaveCount(0, { timeout: 10000 })
  })
})
