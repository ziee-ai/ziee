import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — mobile tab-strip mode (TEST-23, ITEM-30 / ITEM-12).
 *
 * Below the `md` breakpoint there is no room to tile columns, so `SplitChatView`
 * renders `data-split-mode="tabs"`: a `PaneTabStrip` with one tab per open
 * conversation and exactly ONE visible pane (the focused one; the others are
 * MOUNTED but `hidden`). "Open beside" adds a tab; tapping a tab swaps the
 * visible conversation. No LLM (conversations created via the API).
 */
test.describe('Split chat — mobile tab strip', () => {
  test.use({ viewport: { width: 390, height: 844 } })

  test('a phone-width split renders a tab strip (one visible pane); tabs swap the visible conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const mkConv = async (title: string): Promise<string> => {
      const res = await page.request.post(`${apiURL}/api/conversations`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { title },
      })
      expect(res.status()).toBeLessThan(300)
      return (await res.json()).id as string
    }
    const convA = await mkConv('Mobile Tab A')
    const convB = await mkConv('Mobile Tab B')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // Split → a 2nd pane. At phone width the workspace is TAB mode, not columns.
    await byTestId(page, 'chat-split-btn').click()
    const view = byTestId(page, 'split-chat-view')
    await expect(view).toBeVisible({ timeout: 15000 })
    await expect(view).toHaveAttribute('data-split-mode', 'tabs')

    // A tab strip with a tab per pane (conv A + the new empty/new-chat pane).
    const strip = byTestId(page, 'pane-tab-strip')
    await expect(strip).toBeVisible()
    await expect(byTestId(page, 'pane-tab-0')).toBeVisible()
    await expect(byTestId(page, 'pane-tab-1')).toBeVisible()

    // Fill the 2nd pane with EXISTING conversation B via the picker (it is the
    // focused/visible pane right after Split).
    const pane1 = byTestId(page, 'chat-pane-1')
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()

    // Exactly ONE pane is visible at a time (tab mode, no side-by-side columns):
    // the focused pane 1 (conv B) shows; pane 0 (conv A) is mounted-but-hidden.
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-0')).toBeHidden()

    // Tapping tab 0 swaps the visible conversation to pane 0; pane 1 hides.
    await byTestId(page, 'pane-tab-0').click()
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-1')).toBeHidden()
  })
})
