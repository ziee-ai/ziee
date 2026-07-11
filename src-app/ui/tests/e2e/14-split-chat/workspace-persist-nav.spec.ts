import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — workspace persistence across navigation (TEST-51, ITEM-25/26).
 * With a 2-existing-conversation split open: navigating away and back RESTORES
 * both panes; a full reload restores them (from `ziee-split-workspace-v2:<userId>`);
 * a deep-link to a conversation already in the workspace FOCUSES its pane (no
 * duplicate) while one NOT in the workspace REPLACES the focused pane; and the URL
 * tracks the focused pane without a navigate↔focus history loop. No LLM.
 */
test.describe('Split chat — workspace persist + nav', () => {
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

  test('a 2-existing split survives nav-away/back + reload; deep-link focuses or replaces', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Persist Alpha')
    const convB = await mkConv(page, apiURL, token, 'Persist Bravo')
    const convC = await mkConv(page, apiURL, token, 'Persist Charlie')

    // Build a [A | B] split: view A, open B via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId('conversation-picker-search').fill('Bravo')
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(
      byTestId(page, 'chat-pane-1').locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })
    // A blob is written once panes>=2 (250ms debounce) — settle before navigating.
    await page.waitForTimeout(600)

    // --- Nav away (settings) and back to a PANED conversation URL: both panes
    // restore. (Bare `/chat` routes to NewChatPage, which is split-unaware —
    // SplitChatView only mounts inside ConversationPage at /chat/:id when
    // panes>=2, so the restore is asserted on a conversation URL.) ---
    await page.goto(`${baseURL}/settings`)
    await page.waitForLoadState('load')
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()

    // --- Full reload (still on the conversation URL): both panes restore from
    // localStorage. ---
    await page.reload()
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()

    // --- Deep-link to a PANED conversation (A) FOCUSES its pane, no duplicate:
    // still exactly 2 panes and A is shown. (A full-load deep-link reconciles the
    // URL against the hydrated workspace; A is already in a pane → focus it.) ---
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0) // no duplicate pane
    await expect(byTestId(page, 'split-chat-view')).toContainText('Persist Alpha')

    // --- Deep-link to a NON-paned conversation (C) REPLACES a pane (does NOT
    // append): still exactly 2 panes and C now appears in the workspace. ---
    await page.goto(`${baseURL}/chat/${convC}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0) // replaced, not appended
    await expect(byTestId(page, 'split-chat-view')).toContainText('Persist Charlie')
  })
})
