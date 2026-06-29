import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — ConversationList search in the NARROW layout (ChatHistoryPage.tsx:65-83,
 * 118-131). On a narrow page the inline header search is hidden; a header
 * "Open search" toggle reveals a body-portaled search box (the
 * getSearchBoxContainer → bodySearchRef path). This toggle was untested.
 */

test.describe('Chat history — narrow-layout search toggle', () => {
  test('the "Open search" toggle reveals the body search box when narrow', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    // A conversation so ConversationList (which owns the portaled search box)
    // actually renders.
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Narrow Search Conv' },
    })
    expect(res.ok()).toBeTruthy()

    // Narrow the viewport so the page element is ≤640px (minSize.sm → isNarrow).
    await page.setViewportSize({ width: 480, height: 900 })
    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('domcontentloaded')

    // The narrow-only "Open search" toggle is present; the search box is hidden.
    const toggle = page.getByRole('button', { name: 'Open search' })
    await expect(toggle).toBeVisible({ timeout: 30000 })
    await expect(page.getByPlaceholder('Search conversations...')).toHaveCount(0)

    // Click it → the body search box appears (toggle flips to pressed).
    await toggle.click()
    await expect(page.getByPlaceholder('Search conversations...')).toBeVisible({
      timeout: 10000,
    })
    await expect(
      page.getByRole('button', { name: 'Hide search' }),
    ).toHaveAttribute('aria-pressed', 'true')
  })
})
