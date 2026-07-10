import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — mobile viewport (TEST-23). The dedicated tab-strip mode was
 * deferred (DRIFT-1.11); the SHIPPED behavior is that `SplitChatView` renders
 * both panes as (narrow) columns at ALL viewports and both composers stay usable.
 * This asserts that shipped behavior at a phone-sized viewport. No LLM.
 */
test.describe('Split chat — mobile viewport', () => {
  test.use({ viewport: { width: 390, height: 844 } })

  test('at a phone viewport the split still renders both panes as usable columns', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Mobile Split' },
    })
    const convA = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')

    // Both panes render side-by-side (columns) even at 390px wide.
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })
    const box0 = await pane0.boundingBox()
    const box1 = await pane1.boundingBox()
    expect(box0).not.toBeNull()
    expect(box1).not.toBeNull()
    // Laid out as columns: pane 1 starts to the RIGHT of pane 0 (not stacked).
    expect((box1?.x ?? 0)).toBeGreaterThan((box0?.x ?? 0))

    // Both composers remain usable at the narrow width.
    const inputA = pane0.locator('textarea[placeholder*="Type your message"]')
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await expect(inputA).toBeVisible({ timeout: 15000 })
    await expect(inputB).toBeVisible()
    await inputA.fill('mobile A')
    await inputB.fill('mobile B')
    await expect(inputA).toHaveValue('mobile A')
    await expect(inputB).toHaveValue('mobile B')
  })
})
