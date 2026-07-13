import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — PER-TAB workspace persistence (TEST-51, ITEM-25/26 + ITEM-73/
 * DEC-74). The persistence model changed (FB-20): a split is per-TAB
 * (sessionStorage) and is restored ONLY on a same-tab RELOAD, never resurrected by
 * a fresh full-load navigation. That is exactly what stops a NEW TAB / deep-link
 * from inheriting another tab's split (the cross-tab bug). So with a 2-conversation
 * split open:
 *   - a same-tab RELOAD (F5) restores both panes (from `ziee-split-workspace-v2:
 *     <userId>` in sessionStorage), and
 *   - a fresh full-load navigation to a conversation is URL-authoritative → a
 *     single pane, NOT the resurrected split.
 * The in-memory SPA-nav reconcile (clicking a conversation into an OPEN split →
 * focus/replace/new-pane) is a separate, still-live path covered by
 * `open-in-split.spec.ts` (⋯-menu) + the ITEM-43 open-choice prompt (TEST-63).
 * No LLM.
 */
test.describe('Split chat — per-tab workspace persist + nav', () => {
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

  test('a same-tab RELOAD restores the split; a fresh full-load navigation starts single-pane', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Persist Alpha')
    const convB = await mkConv(page, apiURL, token, 'Persist Bravo')

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
    // The per-tab blob is written once panes>=2 (250ms debounce) — settle first.
    await page.waitForTimeout(600)

    // --- Same-tab RELOAD (F5): both panes restore from this tab's sessionStorage.
    // (This is the ONE navigation type that restores — see ITEM-73/DEC-74.) ---
    await page.reload()
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()

    // --- A fresh full-load navigation to a conversation is URL-authoritative: a
    // SINGLE pane, NOT the resurrected split. This is the crux of the per-tab model
    // (FB-20): a new tab / deep-link boots the app the same way and therefore can
    // NEVER inherit the split. ---
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'conversation-title').first()).toContainText('Alpha', { timeout: 15000 })
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0)
  })
})
