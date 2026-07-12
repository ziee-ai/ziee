import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — window-management chrome is scoped to the context where it makes
 * sense (TEST-85, ITEM-55/56, FB-13). The action-audit found the chat header's
 * back / split / pop-out buttons — all designed for the main window — mis-behaving
 * in the new contexts: the back arrow (`navigate('/chats')`) collapsed the whole
 * split from a per-pane click AND turned the chat-only pop-out window into the whole
 * app; the split button spawned a split INSIDE the pop-out window; the pop-out
 * button was a self-focusing no-op there. So:
 *   - the BACK arrow shows only in single-pane (hidden in split panes + pop-out), and
 *   - the POP-OUT WINDOW hides all window-management chrome (back + split + pop-out),
 *     keeping only conversation actions (title, find, composer).
 * RUNS the render + asserts the DOM (present / absent), not a code-read.
 */
test.describe('Split chat — header chrome scoped per context (back / split / pop-out)', () => {
  test.describe.configure({ retries: 1 })

  test('back hidden in split + pop-out; the pop-out window hides all window-management chrome', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(90000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }
    const mk = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mk('HdrVis Alpha')
    const convB = await mk('HdrVis Bravo')

    // SINGLE-PANE: the back arrow IS shown (its normal "back to /chats" home).
    await page.goto(`${baseURL}/chat/${convA}`)
    await expect(page.locator('textarea[placeholder*="Type your message"]').first()).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'conversation-back-button')).toHaveCount(1)

    // SPLIT [A|B]: the back arrow is HIDDEN in every pane (panes use their ✕ close).
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()
    await expect(byTestId(page, 'conversation-back-button')).toHaveCount(0)

    // POP-OUT WINDOW route (chat-only): it's a focused single-conversation view, so
    // ALL window-management chrome is hidden — back, split, AND pop-out (ITEM-55/56).
    // The conversation actions (title, find, composer) remain. Reset the split
    // workspace first so this is a clean single-conversation window (like a real one).
    await page.evaluate(() =>
      Object.keys(localStorage)
        .filter(k => k.toLowerCase().includes('split'))
        .forEach(k => localStorage.removeItem(k)),
    )
    await page.goto(`${baseURL}/chat-window/${convA}`)
    await page.reload()
    await expect(page.locator('textarea[placeholder*="Type your message"]').first()).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'conversation-title').first()).toBeVisible()
    // Window-management chrome — ALL hidden.
    await expect(byTestId(page, 'conversation-back-button')).toHaveCount(0)
    await expect(byTestId(page, 'chat-split-btn')).toHaveCount(0)
    await expect(byTestId(page, 'chat-open-in-new-window')).toHaveCount(0)
    // Conversation actions — still present.
    await expect(byTestId(page, 'conversation-find-toggle-btn').first()).toBeVisible()
    await expect(page.locator('textarea[placeholder*="Type your message"]').first()).toBeVisible()
  })
})
