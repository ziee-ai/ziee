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
 * This spec drives the LITERAL user flow across REAL tabs — the split is formed by
 * DRAGGING convB onto the right third of the single pane (the app's real HTML5 drop
 * handler, not the split-button+picker shortcut), and the new tab is opened by a real
 * `window.open` (so it genuinely inherits a COPY of the opener's sessionStorage —
 * split blob included — not an `addInitScript` simulation, which mis-boots the app):
 *  build [A|B] by drag; then
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

  // Build the split the LITERAL way the user does: DRAG a conversation onto the
  // right third of the single pane (the app's real HTML5 drop handler, shared
  // DataTransfer — same method as single-pane-drop.spec), NOT the split-button +
  // picker shortcut. 0.85 = right third → [current | dropped].
  const CONV_MIME = 'application/x-ziee-conversation'
  const dragConvOntoRightThird = async (
    page: import('@playwright/test').Page,
    convId: string,
  ) => {
    const column = byTestId(page, 'chat-single-drop-column')
    await expect(column).toBeVisible({ timeout: 15000 })
    const box = await column.boundingBox()
    if (!box) throw new Error('no drop-column box')
    const clientX = box.x + box.width * 0.85
    const clientY = box.y + box.height * 0.5
    const dt = await page.evaluateHandle(
      ({ mime, id }) => {
        const d = new DataTransfer()
        d.setData(mime, id)
        return d
      },
      { mime: CONV_MIME, id: convId },
    )
    await column.dispatchEvent('dragover', { dataTransfer: dt, clientX, clientY })
    await column.dispatchEvent('drop', { dataTransfer: dt, clientX, clientY })
    await dt.dispose()
    // The proof the DRAG worked is that the split actually forms below (chat-pane-1
    // appears with the dropped conversation) — a no-op drop would leave single-pane.
  }

  // Shared setup: log in, create A + B, and build the [A|B] split by DRAGGING convB
  // onto the right third of the single pane (the literal "drag one onto the right
  // pane" flow). On return the per-tab blob has flushed and is asserted saved.
  const dragBuildSplit = async (
    page: import('@playwright/test').Page,
    testInfra: { baseURL: string; apiURL: string },
  ) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Tab Alpha')
    const convB = await mkConv(page, apiURL, token, 'Tab Bravo')
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await dragConvOntoRightThird(page, convB)
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Alpha')
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Bravo', { timeout: 15000 })
    await page.waitForTimeout(600) // the per-tab blob is debounced (250ms) — settle
    // Assert the split IS saved, so the pop-out below genuinely copies a split-bearing
    // sessionStorage (else the isolation would pass trivially).
    const saved = await page.evaluate(() =>
      Object.keys(sessionStorage).some((k) => k.startsWith('ziee-split-workspace')),
    )
    expect(saved, 'the split is saved to sessionStorage (per-tab)').toBe(true)
    return { baseURL, convA, convB }
  }

  // THE LITERAL BUG (NO reload — exactly the reported repro): drag to split, then pop
  // the right pane out to a new tab → that tab must show ONLY the dropped conversation,
  // never the inherited split.
  test('drag to split, then pop the pane out to a new tab → the new tab shows only that conversation', async ({
    page,
    context,
    testInfra,
  }) => {
    const { convB } = await dragBuildSplit(page, testInfra)
    // ⤢ pop out pane B → `window.open('/chat/B')` copies THIS tab's sessionStorage
    // (split blob included). The reload gate drops the copied split on the fresh nav.
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

  // SEPARATE per-tab-persistence concern (NOT part of the bug repro): a same-tab
  // reload restores that tab's own split.
  test("a same-tab reload restores that tab's split (per-tab persistence)", async ({
    page,
    testInfra,
  }) => {
    await dragBuildSplit(page, testInfra)
    await page.reload()
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
  })
})
