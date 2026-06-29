import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — ConversationList "Load More" pagination (ConversationList.tsx:165-173,
 * 250-253). The list pages 20 at a time (ChatHistory.store limit=20); when more
 * exist a "Load More" button fetches the next page. Untested before.
 */

test.describe('Chat — conversation list Load More', () => {
  test('Load More fetches the next page of conversations', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed 25 conversations (> the page size of 20) so a second page exists.
    for (let i = 0; i < 25; i++) {
      const res = await page.request.post(`${apiURL}/api/conversations`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { title: `LoadMore Conv ${String(i).padStart(2, '0')}` },
      })
      expect(res.ok()).toBeTruthy()
    }

    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('domcontentloaded')

    // First page shows 20 of 25 with a Load More button.
    await expect(page.getByText(/Showing 20 of 25 conversations/)).toBeVisible({
      timeout: 30000,
    })
    const loadMore = page.getByRole('button', { name: 'Load More' })
    await expect(loadMore).toBeVisible()

    // Click → the next page loads; all 25 now shown and the button disappears.
    await loadMore.click()
    await expect(page.getByText(/Showing 25 of 25 conversations/)).toBeVisible({
      timeout: 15000,
    })
    await expect(page.getByRole('button', { name: 'Load More' })).toHaveCount(0)
  })
})
