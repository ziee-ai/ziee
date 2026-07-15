import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — per-pane edge-directional drop in an EXISTING split (TEST-28,
 * ITEM-70 — unified with the single-pane model). Dropping a conversation on a
 * pane's LEFT third inserts a new pane immediately BEFORE it, the RIGHT third
 * inserts AFTER, the CENTER replaces that pane; at the MAX_PANES(3) cap the edges
 * fall back to replace. An OS FILE dropped on a pane is ignored (belongs to the
 * composer). Dragging a pane's GRIP onto another pane's header still REORDERS.
 * Driven via synthetic HTML5 DnD (a shared DataTransfer + a clientX aimed at the
 * target third). No LLM.
 */
test.describe('Split chat — per-pane edge-directional drop (existing split)', () => {
  const CONV_MIME = 'application/x-ziee-conversation'

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

  // Drop a conversation onto a pane sub-element at a horizontal fraction
  // (0.15 = left third, 0.5 = center, 0.85 = right third). `sel` picks the column
  // (default) or the header — both are live conversation drop targets (blind-audit
  // fix: the header is a SIBLING of the column, so it needs its own handler).
  const dropOnPane = async (
    page: import('@playwright/test').Page,
    paneTestId: string,
    convId: string,
    frac: number,
    sel = '[data-pane-drop-column]',
  ) => {
    const col = sel === 'header' ? byTestId(page, paneTestId).getByTestId('chat-pane-header') : byTestId(page, paneTestId).locator(sel)
    await expect(col).toBeVisible({ timeout: 15000 })
    const box = await col.boundingBox()
    if (!box) throw new Error('no column box')
    const clientX = box.x + box.width * frac
    const clientY = box.y + box.height * 0.5
    const dt = await page.evaluateHandle(
      ({ mime, id }) => {
        const d = new DataTransfer()
        d.setData(mime, id)
        return d
      },
      { mime: CONV_MIME, id: convId },
    )
    await col.dispatchEvent('dragover', { dataTransfer: dt, clientX, clientY })
    await col.dispatchEvent('drop', { dataTransfer: dt, clientX, clientY })
    await dt.dispose()
  }

  const titles = (page: import('@playwright/test').Page, n: number) =>
    Array.from({ length: n }, (_, i) => byTestId(page, `chat-pane-${i}`).getByTestId('conversation-title'))

  // Build [A | B] (pane 0 = A, pane 1 = B).
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
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })
  }

  test('right edge inserts AFTER, left edge inserts BEFORE, center REPLACES; file ignored', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Drag Alpha')
    const convB = await mkConv(page, apiURL, token, 'Drag Bravo')
    const convC = await mkConv(page, apiURL, token, 'Drag Charlie')

    // --- RIGHT edge of pane 0 (A) → insert C AFTER A → [A, C, B] ---
    await openAB(page, baseURL, convA, convB)
    await dropOnPane(page, 'chat-pane-0', convC, 0.85)
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
    await expect(titles(page, 3)[0]).toContainText('Alpha')
    await expect(titles(page, 3)[1]).toContainText('Charlie')
    await expect(titles(page, 3)[2]).toContainText('Bravo')

    // --- LEFT edge of pane 0 (A) → insert C BEFORE A → [C, A, B] ---
    await openAB(page, baseURL, convA, convB)
    await dropOnPane(page, 'chat-pane-0', convC, 0.15)
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
    await expect(titles(page, 3)[0]).toContainText('Charlie')
    await expect(titles(page, 3)[1]).toContainText('Alpha')
    await expect(titles(page, 3)[2]).toContainText('Bravo')

    // --- CENTER of pane 1 (B) → REPLACE B with C → [A, C] (still 2 panes) ---
    await openAB(page, baseURL, convA, convB)
    await dropOnPane(page, 'chat-pane-1', convC, 0.5)
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Charlie', { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Alpha')
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0) // no new pane — replaced

    // --- Dropping on a pane's HEADER also works (it's a sibling of the column,
    // so it has its own unified handler): center of pane 1's header → REPLACE B
    // with C → [A, C]. Pre-fix the header ignored conversation drags (no-op). ---
    await openAB(page, baseURL, convA, convB)
    await dropOnPane(page, 'chat-pane-1', convC, 0.5, 'header')
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Charlie', { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Alpha')

    // --- FILE dropped on a pane is IGNORED (dragKind→'file') ---
    await openAB(page, baseURL, convA, convB)
    const fileDt = await page.evaluateHandle(() => {
      const d = new DataTransfer()
      d.items.add(new File(['x'], 'note.txt', { type: 'text/plain' }))
      return d
    })
    await byTestId(page, 'chat-pane-0').locator('[data-pane-drop-column]').dispatchEvent('drop', { dataTransfer: fileDt })
    await fileDt.dispose()
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Alpha')
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0)
  })

  test('at the MAX_PANES cap, an edge drop falls back to REPLACE (no 4th pane)', async ({
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

    // Build [A | B | C] (3 panes = MAX).
    await openAB(page, baseURL, convA, convB)
    await byTestId(page, 'chat-pane-1').getByTestId('chat-split-btn').click()
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-2').getByTestId(`conversation-picker-item-${convC}`).click()
    await expect(byTestId(page, 'chat-pane-2').getByTestId('conversation-title')).toContainText('Charlie', { timeout: 15000 })

    // Drop D on pane 0's RIGHT edge — at cap, this REPLACES pane 0 (no 4th pane).
    await dropOnPane(page, 'chat-pane-0', convD, 0.85)
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Delta', { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-3')).toHaveCount(0) // capped — no 4th pane
  })

  test('dragging a pane grip onto another pane header reorders the panes', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Order Alpha')
    const convB = await mkConv(page, apiURL, token, 'Order Bravo')
    await openAB(page, baseURL, convA, convB)
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0.getByTestId('conversation-title')).toContainText('Alpha')
    await expect(pane1.getByTestId('conversation-title')).toContainText('Bravo')

    const dt = await page.evaluateHandle(() => new DataTransfer())
    await pane0.getByTestId('chat-pane-grip').dispatchEvent('dragstart', { dataTransfer: dt })
    await pane1.getByTestId('chat-pane-header').dispatchEvent('dragover', { dataTransfer: dt })
    await pane1.getByTestId('chat-pane-header').dispatchEvent('drop', { dataTransfer: dt })
    await dt.dispose()

    await expect(pane0.getByTestId('conversation-title')).toContainText('Bravo', { timeout: 15000 })
    await expect(pane1.getByTestId('conversation-title')).toContainText('Alpha')
  })
})
