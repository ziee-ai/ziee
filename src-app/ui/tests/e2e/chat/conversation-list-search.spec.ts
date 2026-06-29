import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * ChatHistoryPage (/chats) portals the ConversationList search box into a
 * different target depending on layout width (getSearchBoxContainer):
 *   - WIDE  (!isNarrow): the search input lives inline in the header.
 *   - NARROW (isNarrow): a toggle button reveals/hides it in the page body.
 * ConversationList only mounts when there are conversations, so we seed one.
 */
async function seedConversation(page: any, apiURL: string) {
  const token = await getAdminToken(apiURL)
  const res = await page.request.post(`${apiURL}/api/conversations`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { title: `history-${Date.now()}` },
  })
  expect(res.ok()).toBe(true)
}

const SEARCH = 'Search conversations...'

test.describe('Chat history - conversation list search', () => {
  test('wide layout shows the search input inline in the header', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await seedConversation(page, apiURL)

    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(`${baseURL}/chats`)

    // Header portal target → the search input is visible without any toggle.
    await expect(page.getByPlaceholder(SEARCH)).toBeVisible({ timeout: 30000 })
  })

  test('narrow layout toggles the search input in the body', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await seedConversation(page, apiURL)

    await page.setViewportSize({ width: 460, height: 900 })
    await page.goto(`${baseURL}/chats`)

    // Narrow + closed → the search input is hidden; a toggle button is shown.
    const openBtn = page.getByRole('button', { name: 'Open search' })
    await expect(openBtn).toBeVisible({ timeout: 30000 })
    await expect(page.getByPlaceholder(SEARCH)).toHaveCount(0)

    // Open → the search input appears in the body.
    await openBtn.click()
    await expect(page.getByPlaceholder(SEARCH)).toBeVisible()

    // Close → it hides again.
    await page.getByRole('button', { name: 'Hide search' }).click()
    await expect(page.getByPlaceholder(SEARCH)).toHaveCount(0)
  })
})
