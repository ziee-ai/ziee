import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — tear-off wiring + desktop-only gate (TEST-93, ITEM-58).
 *
 * Runs on the WEB build. To prove the `onDragEnd` wiring end-to-end (not just the
 * gate) we STUB `window.open` (the web `openConversationWindow` base) to record
 * calls, and toggle a faked `window.__TAURI__` to flip the `isDesktop` gate the
 * hook reads at `dragend` time:
 *  - no `__TAURI__` + release past the edge  → opens NOTHING (desktop-only, DEC-70)
 *  - faked `__TAURI__` + release past the edge → OPENS `/chat/<id>` (proves the
 *    source is actually wired to onDragEnd → hook → plan → seam)
 *  - faked `__TAURI__` + release INSIDE the window → opens nothing (strict, DEC-71)
 * Verified for all three drag sources (card, recent-sidebar item, split-pane grip)
 * and, for the grip, that the pane MOVES (closes). The desktop NATIVE-window path
 * (`WebviewWindow`) can't be driven headlessly — same platform limit as TEST-83.
 */
test.describe('Split chat — tear-off wiring + desktop-only gate', () => {
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

  // Stub window.open to RECORD (never actually open a tab) + expose helpers to
  // toggle the faked desktop gate. Installed fresh per page load.
  const installHarness = (page: import('@playwright/test').Page) =>
    page.evaluate(() => {
      const w = window as unknown as { __opened: string[]; __setTauri: (on: boolean) => void }
      w.__opened = []
      window.open = ((url?: string | URL) => {
        w.__opened.push(String(url ?? ''))
        return { focus() {}, closed: false } as unknown as Window
      }) as typeof window.open
      w.__setTauri = (on: boolean) => {
        if (on) (window as unknown as { __TAURI__?: unknown }).__TAURI__ = {}
        else delete (window as unknown as { __TAURI__?: unknown }).__TAURI__
      }
    })

  const opened = (page: import('@playwright/test').Page) =>
    page.evaluate(() => (window as unknown as { __opened: string[] }).__opened.slice())
  const setTauri = (page: import('@playwright/test').Page, on: boolean) =>
    page.evaluate((v) => (window as unknown as { __setTauri: (b: boolean) => void }).__setTauri(v), on)

  // An off-window release (top-left, far outside) vs an in-window release
  // (window origin + a small inset, guaranteed inside outerWidth/Height).
  const OUTSIDE = { screenX: -9999, screenY: -9999 }
  const insidePoint = (page: import('@playwright/test').Page) =>
    page.evaluate(() => ({ screenX: window.screenX + 10, screenY: window.screenY + 10 }))

  test('card / sidebar / grip sources: desktop-only gate + strict trigger + pane MOVE', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(90000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Tear Alpha')
    const convB = await mkConv(page, apiURL, token, 'Tear Bravo')

    // ---- Source 1: ConversationCard on /chats ----
    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('load')
    await installHarness(page)
    const card = byTestId(page, `chat-conversation-card-${convA}`)
    await expect(card).toBeVisible({ timeout: 15000 })

    // (a) WEB (no __TAURI__), release outside → nothing opens (desktop-only gate).
    await card.dispatchEvent('dragend', OUTSIDE)
    await page.waitForTimeout(200)
    expect(await opened(page)).toHaveLength(0)

    // (b) faked desktop, release outside → opens /chat/<A> (proves card wiring).
    await setTauri(page, true)
    await card.dispatchEvent('dragend', OUTSIDE)
    await expect.poll(() => opened(page)).toEqual([`/chat/${convA}`])

    // (c) faked desktop, release INSIDE the window → nothing more (strict trigger).
    await card.dispatchEvent('dragend', await insidePoint(page))
    await page.waitForTimeout(200)
    expect(await opened(page)).toHaveLength(1)
    await expect(page).not.toHaveURL(/\/chat-window\//)

    // ---- Source 2: RecentConversationsWidget sidebar item ----
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await installHarness(page)
    await setTauri(page, true)
    const recentRow = page.locator('[draggable="true"]', { hasText: 'Tear Alpha' }).first()
    await expect(recentRow).toBeVisible({ timeout: 15000 })
    await recentRow.dispatchEvent('dragend', OUTSIDE)
    await expect.poll(() => opened(page)).toEqual([`/chat/${convA}`])

    // ---- Source 3: split-pane grip → tear-off MOVES (pane closes) ----
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.getByTestId('conversation-title')).toContainText('Bravo')
    await installHarness(page)
    await setTauri(page, true)
    await pane1.getByTestId('chat-pane-grip').dispatchEvent('dragend', OUTSIDE)
    // Opens B's window AND removes pane 1 (MOVE) → collapses back to single-pane.
    await expect.poll(() => opened(page)).toEqual([`/chat/${convB}`])
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0, { timeout: 15000 })
  })
})
