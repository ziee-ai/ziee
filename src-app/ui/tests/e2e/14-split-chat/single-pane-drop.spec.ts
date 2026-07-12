import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — single-pane edge-directional drop (TEST-90, ITEM-57). On the
 * UNSPLIT conversation view, dropping a sidebar conversation:
 *   - on the LEFT third   → opens it as a new pane on the LEFT  ([dropped | current])
 *   - on the RIGHT third  → opens it as a new pane on the RIGHT ([current | dropped])
 *   - on the CENTER third → REPLACES the current conversation (no split)
 * Driven via synthetic HTML5 DnD (a shared DataTransfer + a clientX aimed at the
 * target third), like `drag-to-split.spec.ts`. No LLM.
 */
test.describe('Split chat — single-pane edge-directional drop', () => {
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

  // Drop a conversation onto the single-pane column at a horizontal fraction
  // (0.15 = left third, 0.5 = center, 0.85 = right third).
  const dropAtFraction = async (
    page: import('@playwright/test').Page,
    convId: string,
    frac: number,
  ) => {
    const column = byTestId(page, 'chat-single-drop-column')
    await expect(column).toBeVisible({ timeout: 15000 })
    const box = await column.boundingBox()
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
    await column.dispatchEvent('dragover', { dataTransfer: dt, clientX, clientY })
    await column.dispatchEvent('drop', { dataTransfer: dt, clientX, clientY })
    await dt.dispose()
  }

  test('right third → split [current | dropped]; left third → [dropped | current]; center → replace', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Solo Alpha')
    const convB = await mkConv(page, apiURL, token, 'Solo Bravo')
    const convC = await mkConv(page, apiURL, token, 'Solo Charlie')

    // --- RIGHT third: [A | B] (A stays left, B new on the right) ---
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await dropAtFraction(page, convB, 0.85)
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Alpha')
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Bravo')

    // --- LEFT third (fresh single-pane on A): [C | A] (C new on the LEFT) ---
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0) // back to single-pane
    await dropAtFraction(page, convC, 0.15)
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText('Charlie')
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Alpha')

    // --- CENTER (fresh single-pane on A): replace A with B, still single-pane ---
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await dropAtFraction(page, convB, 0.5)
    // No split forms; the single view now shows Bravo.
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0)
    await expect(page).toHaveURL(new RegExp(`/chat/${convB}`), { timeout: 15000 })
    await expect(byTestId(page, 'conversation-title')).toContainText('Bravo')
  })
})
