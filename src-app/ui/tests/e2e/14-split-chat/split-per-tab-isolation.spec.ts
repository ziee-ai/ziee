import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — splits are PER-TAB and a new tab does NOT inherit the split
 * (TEST-111, ITEM-73 / FB-20). The reported bug: open convA single-pane, drag
 * convB onto the right (→ [A|B]), then open the right pane in a NEW TAB — and BOTH
 * tabs rendered the same split. Root cause: the split was persisted to a per-user
 * localStorage key that every tab hydrated on boot. Fix: sessionStorage (per-tab) +
 * hydrate only on a same-tab reload.
 *
 * This spec drives the exact repro across REAL tabs (a real `window.open`, so the
 * new tab genuinely inherits a COPY of the opener's sessionStorage — split blob
 * included — rather than a `addInitScript` simulation, which mis-boots the app):
 *  (C) reloading the ORIGINAL tab restores that tab's split (per-tab reload); then
 *  (A) clicking pane B's ⤢ pops it out via `window.open('/chat/B')` — and the new
 *      tab shows ONLY convB, single pane, NOT the inherited split (the reload gate
 *      drops the copied split); the origin tab collapses to single-pane A.
 * No LLM.
 */
test.describe('Split chat — splits are per-tab; a new tab does not inherit the split', () => {
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

  test('new tab shows one conversation even when it inherits the split blob; reload still restores', async ({
    page,
    context,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Tab Alpha')
    const convB = await mkConv(page, apiURL, token, 'Tab Bravo')

    // Tab 1: open A single-pane, then split with B on the right → [A|B].
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-1').getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Bravo', { timeout: 15000 })
    // The per-tab blob is written once panes>=2 (250ms debounce) — settle first.
    await page.waitForTimeout(600)

    // Confirm the split IS persisted to sessionStorage (per-tab).
    const savedKey = await page.evaluate(() =>
      Object.keys(sessionStorage).find((k) => k.startsWith('ziee-split-workspace')) ?? null,
    )
    expect(savedKey, 'the split is saved to sessionStorage (per-tab)').not.toBeNull()

    // (C) SAME-TAB RELOAD restores that tab's split (from its sessionStorage). Done
    // BEFORE the pop-out below, which moves pane B out of this tab.
    await page.reload()
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
    await page.waitForTimeout(600) // re-save the restored split before the pop-out copies it

    // (A) THE EXACT USER SCENARIO: with the [A|B] split open, click pane B's ⤢
    // "open in new tab". On web that is `window.open('/chat/B')`, whose sessionStorage
    // is a live COPY of this tab's — split blob included. The new tab MUST show ONLY
    // convB, single-pane: the reload gate refuses to restore a split on a fresh
    // navigation, so the copied split can never leak into the new tab.
    const [popup] = await Promise.all([
      context.waitForEvent('page'),
      byTestId(page, 'chat-pane-1').getByTestId('chat-open-in-new-window').click(),
    ])
    await popup.waitForLoadState('load')
    expect(popup.url()).toContain(`/chat/${convB}`)
    await expect(popup.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 30000 })
    await expect(popup.getByTestId('split-chat-view')).toHaveCount(0)
    await expect(popup.getByTestId('chat-pane-1')).toHaveCount(0)
    await expect(popup.getByTestId('conversation-title')).toContainText('Bravo', { timeout: 15000 })

    // ...and the ORIGINAL tab collapsed to single-pane A (pane B moved out).
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0, { timeout: 15000 })
  })
})
